use std::{borrow::Cow, thread::sleep};

use bytemuck::{Pod, Zeroable, __core::time::Duration};
use serial::{BaudRate, SystemPort};
use strum_macros::Display;

use crate::{
    chip::Chip,
    command::{Command, CommandType},
    connection::Connection,
    elf::{FirmwareImage, RomSegment},
    error::{ConnectionError, FlashDetectError, ResultExt, RomError, RomErrorKind},
    Error, PartitionTable,
};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(3);
pub(crate) const FLASH_SECTOR_SIZE: usize = 0x1000;
const FLASH_BLOCK_SIZE: usize = 0x100;
const FLASH_SECTORS_PER_BLOCK: usize = FLASH_SECTOR_SIZE / FLASH_BLOCK_SIZE;
pub(crate) const FLASH_WRITE_SIZE: usize = 0x400;

// register used for chip detect
const CHIP_DETECT_MAGIC_REG_ADDR: u32 = 0x40001000;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Display)]
#[allow(dead_code)]
#[repr(u8)]
pub enum FlashSize {
    #[strum(serialize = "256KB")]
    Flash256Kb = 0x12,
    #[strum(serialize = "512KB")]
    Flash512Kb = 0x13,
    #[strum(serialize = "1MB")]
    Flash1Mb = 0x14,
    #[strum(serialize = "2MB")]
    Flash2Mb = 0x15,
    #[strum(serialize = "4MB")]
    Flash4Mb = 0x16,
    #[strum(serialize = "8MB")]
    Flash8Mb = 0x17,
    #[strum(serialize = "16MB")]
    Flash16Mb = 0x18,
    #[strum(serialize = "32MB")]
    Flash32Mb = 0x19,
    #[strum(serialize = "64MB")]
    Flash64Mb = 0x1a,
    FlashRetry = 0xFF, // used to hint that alternate detection should be tried
}

impl FlashSize {
    fn from(value: u8) -> Result<FlashSize, Error> {
        match value {
            0x12 => Ok(FlashSize::Flash256Kb),
            0x13 => Ok(FlashSize::Flash512Kb),
            0x14 => Ok(FlashSize::Flash1Mb),
            0x15 => Ok(FlashSize::Flash2Mb),
            0x16 => Ok(FlashSize::Flash4Mb),
            0x17 => Ok(FlashSize::Flash8Mb),
            0x18 => Ok(FlashSize::Flash16Mb),
            0x19 => Ok(FlashSize::Flash32Mb),
            0x1a => Ok(FlashSize::Flash64Mb),
            0xFF => Ok(FlashSize::FlashRetry),
            _ => Err(Error::UnsupportedFlash(FlashDetectError::from(value))),
        }
    }
}

#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct SpiAttachParams {
    clk: u8,
    q: u8,
    d: u8,
    hd: u8,
    cs: u8,
}

impl SpiAttachParams {
    pub const fn default() -> Self {
        SpiAttachParams {
            clk: 0,
            q: 0,
            d: 0,
            hd: 0,
            cs: 0,
        }
    }

    pub const fn esp32_pico_d4() -> Self {
        SpiAttachParams {
            clk: 6,
            q: 17,
            d: 8,
            hd: 11,
            cs: 16,
        }
    }

    pub fn encode(self) -> Vec<u8> {
        let packed = ((self.hd as u32) << 24)
            | ((self.cs as u32) << 18)
            | ((self.d as u32) << 12)
            | ((self.q as u32) << 6)
            | (self.clk as u32);
        if packed == 0 {
            vec![0; 5]
        } else {
            packed.to_le_bytes().to_vec()
        }
    }
}

/// List of spi params to try while detecting flash size
const TRY_SPI_PARAMS: [SpiAttachParams; 2] =
    [SpiAttachParams::default(), SpiAttachParams::esp32_pico_d4()];

#[derive(Zeroable, Pod, Copy, Clone, Debug)]
#[repr(C)]
struct BlockParams {
    size: u32,
    sequence: u32,
    dummy1: u32,
    dummy2: u32,
}

#[derive(Zeroable, Pod, Copy, Clone, Debug)]
#[repr(C)]
struct BeginParams {
    size: u32,
    blocks: u32,
    block_size: u32,
    offset: u32,
    encrypted: u32,
}

#[derive(Zeroable, Pod, Copy, Clone)]
#[repr(C)]
struct EntryParams {
    no_entry: u32,
    entry: u32,
}

pub struct Flasher {
    connection: Connection,
    chip: Chip,
    flash_size: FlashSize,
    spi_params: SpiAttachParams,
}

impl Flasher {
    pub fn connect(serial: SystemPort, speed: Option<BaudRate>) -> Result<Self, Error> {
        let mut flasher = Flasher {
            connection: Connection::new(serial), // default baud is always 115200
            chip: Chip::Esp8266,                 // dummy, set properly later
            flash_size: FlashSize::Flash4Mb,
            spi_params: SpiAttachParams::default(), // may be set when trying to attach to flash
        };
        flasher.start_connection()?;
        flasher.connection.set_timeout(DEFAULT_TIMEOUT)?;
        flasher.chip_detect()?;
        flasher.spi_autodetect()?;

        if let Some(b) = speed {
            match flasher.chip {
                Chip::Esp8266 => (), /* Not available */
                _ => {
                    if b.speed() > BaudRate::Baud115200.speed() {
                        println!("WARN setting baud rate higher than 115200 can cause issues.");
                        flasher.change_baud(b)?;
                    }
                }
            }
        }

        Ok(flasher)
    }

    fn spi_autodetect(&mut self) -> Result<(), Error> {
        // loop over all available spi params until we find one that successfully reads
        // the flash size
        for spi_params in TRY_SPI_PARAMS.iter().copied() {
            self.enable_flash(spi_params)?;
            if self.flash_detect()? {
                // flash detect successful, save these spi params
                self.spi_params = spi_params;
                return Ok(());
            }
        }

        // none of the spi parameters were successful
        Err(Error::FlashConnect)
    }

    fn chip_detect(&mut self) -> Result<(), Error> {
        let magic = self.connection.read_reg(CHIP_DETECT_MAGIC_REG_ADDR)?;
        let chip = Chip::from_magic(magic)?;

        self.chip = chip;
        Ok(())
    }

    fn flash_detect(&mut self) -> Result<bool, Error> {
        let flash_id = self.spi_command(CommandType::FlashDetect, &[], 24)?;
        let size_id = flash_id >> 16;

        self.flash_size = match FlashSize::from(size_id as u8) {
            Ok(size) => size,
            Err(_) => {
                eprintln!(
                    "Warning: could not detect flash size (FlashID=0x{:02X}, SizeID=0x{:02X}), defaulting to 4MB\n",
                    flash_id,
                    size_id
                );
                FlashSize::Flash4Mb
            }
        };

        Ok(self.flash_size != FlashSize::FlashRetry)
    }

    fn sync(&mut self) -> Result<(), Error> {
        self.connection
            .with_timeout(CommandType::Sync.timeout(), |connection| {
                connection.write_command(Command::Sync)?;

                for _ in 0..100 {
                    match connection.read_response()? {
                        Some(response) if response.return_op == CommandType::Sync as u8 => {
                            if response.status == 1 {
                                let _error = connection.flush();
                                return Err(Error::RomError(RomError::new(
                                    CommandType::Sync,
                                    RomErrorKind::from(response.error),
                                )));
                            } else {
                                break;
                            }
                        }
                        _ => continue,
                    }
                }

                Ok(())
            })?;
        for _ in 0..700 {
            match self.connection.read_response()? {
                Some(_) => break,
                _ => continue,
            }
        }
        Ok(())
    }

    fn start_connection(&mut self) -> Result<(), Error> {
        self.connection.reset_to_flash()?;
        for _ in 0..10 {
            self.connection.flush()?;
            if self.sync().is_ok() {
                return Ok(());
            }
        }
        Err(Error::Connection(ConnectionError::ConnectionFailed))
    }

    fn enable_flash(&mut self, spi_params: SpiAttachParams) -> Result<(), Error> {
        match self.chip {
            Chip::Esp8266 => {
                self.connection.command(Command::FlashBegin {
                    supports_encryption: false,
                    offset: 0,
                    block_size: FLASH_WRITE_SIZE as u32,
                    size: 0,
                    blocks: 0,
                })?;
            }
            _ => {
                self.connection
                    .with_timeout(CommandType::SpiAttach.timeout(), |connection| {
                        connection.command(Command::SpiAttach { spi_params })
                    })?;
            }
        }
        Ok(())
    }

    fn spi_command(
        &mut self,
        command: CommandType,
        data: &[u8],
        read_bits: u32,
    ) -> Result<u32, Error> {
        assert!(read_bits < 32);
        assert!(data.len() < 64);

        let spi_registers = self.chip.spi_registers();

        let old_spi_usr = self.connection.read_reg(spi_registers.usr())?;
        let old_spi_usr2 = self.connection.read_reg(spi_registers.usr2())?;

        let mut flags = 1 << 31;
        if !data.is_empty() {
            flags |= 1 << 27;
        }
        if read_bits > 0 {
            flags |= 1 << 28;
        }

        self.connection
            .write_reg(spi_registers.usr(), flags, None)?;
        self.connection
            .write_reg(spi_registers.usr2(), 7 << 28 | command as u32, None)?;

        if let (Some(mosi_data_length), Some(miso_data_length)) =
            (spi_registers.mosi_length(), spi_registers.miso_length())
        {
            if !data.is_empty() {
                self.connection
                    .write_reg(mosi_data_length, data.len() as u32 * 8 - 1, None)?;
            }
            if read_bits > 0 {
                self.connection
                    .write_reg(miso_data_length, read_bits - 1, None)?;
            }
        } else {
            let mosi_mask = if data.is_empty() {
                0
            } else {
                data.len() as u32 * 8 - 1
            };
            let miso_mask = if read_bits == 0 { 0 } else { read_bits - 1 };
            self.connection.write_reg(
                spi_registers.usr1(),
                miso_mask << 8 | mosi_mask << 17,
                None,
            )?;
        }

        if data.is_empty() {
            self.connection.write_reg(spi_registers.w0(), 0, None)?;
        } else {
            for (i, bytes) in data.chunks(4).enumerate() {
                let mut data_bytes = [0; 4];
                data_bytes[0..bytes.len()].copy_from_slice(bytes);
                let data = u32::from_le_bytes(data_bytes);
                self.connection
                    .write_reg(spi_registers.w0() + i as u32, data, None)?;
            }
        }

        self.connection
            .write_reg(spi_registers.cmd(), 1 << 18, None)?;

        let mut i = 0;
        loop {
            sleep(Duration::from_millis(1));
            if self.connection.read_reg(spi_registers.usr())? & (1 << 18) == 0 {
                break;
            }
            i += 1;
            if i > 10 {
                return Err(Error::Connection(ConnectionError::Timeout(command.into())));
            }
        }

        let result = self.connection.read_reg(spi_registers.w0())?;
        self.connection
            .write_reg(spi_registers.usr(), old_spi_usr, None)?;
        self.connection
            .write_reg(spi_registers.usr2(), old_spi_usr2, None)?;

        Ok(result)
    }

    /// The active serial connection being used by the flasher
    pub fn connection(&mut self) -> &mut Connection {
        &mut self.connection
    }

    /// The chip type that the flasher is connected to
    pub fn chip(&self) -> Chip {
        self.chip
    }

    /// The flash size of the board that the flasher is connected to
    pub fn flash_size(&self) -> FlashSize {
        self.flash_size
    }

    /// Read and print any information we can about the connected board
    pub fn board_info(&mut self) -> Result<(), Error> {
        let chip = self.chip();
        let size = self.flash_size();

        let maybe_revision = chip.chip_revision(self.connection())?;
        let features = chip.chip_features(self.connection())?;
        let freq = chip.crystal_freq(self.connection())?;
        let mac = chip.mac_address(self.connection())?;

        print!("Chip type:         {}", chip);
        match maybe_revision {
            Some(revision) => println!(" (revision {})", revision),
            None => println!(),
        }
        println!("Crystal frequency: {}MHz", freq);
        println!("Flash size:        {}", size);
        println!("Features:          {}", features.join(", "));
        println!("MAC address:       {}", mac);

        Ok(())
    }

    /// Load an elf image to ram and execute it
    ///
    /// Note that this will not touch the flash on the device
    pub fn load_elf_to_ram(&mut self, elf_data: &[u8]) -> Result<(), Error> {
        let image = FirmwareImage::from_data(elf_data)?;

        let mut target = self.chip.ram_target();
        target.begin(&mut self.connection, &image).flashing()?;

        if image.rom_segments(self.chip).next().is_some() {
            return Err(Error::ElfNotRamLoadable);
        }

        for segment in image.ram_segments(self.chip) {
            target
                .write_segment(
                    &mut self.connection,
                    RomSegment {
                        addr: segment.addr,
                        data: Cow::Borrowed(segment.data()),
                    },
                )
                .flashing()?;
        }

        target.finish(&mut self.connection, true).flashing()
    }

    /// Load an elf image to flash and execute it
    pub fn load_elf_to_flash(
        &mut self,
        elf_data: &[u8],
        bootloader: Option<Vec<u8>>,
        partition_table: Option<PartitionTable>,
    ) -> Result<(), Error> {
        let mut image = FirmwareImage::from_data(elf_data)?;
        image.flash_size = self.flash_size();

        let mut target = self.chip.flash_target(self.spi_params);
        target.begin(&mut self.connection, &image).flashing()?;

        let flash_image = self
            .chip
            .get_flash_image(&image, bootloader, partition_table, None)?;

        for segment in flash_image.flash_segments() {
            target
                .write_segment(&mut self.connection, segment)
                .flashing()?;
        }

        target.finish(&mut self.connection, true).flashing()?;

        Ok(())
    }

    pub fn change_baud(&mut self, speed: BaudRate) -> Result<(), Error> {
        self.connection
            .with_timeout(CommandType::ChangeBaud.timeout(), |connection| {
                connection.command(Command::ChangeBaud {
                    speed: speed.speed() as u32,
                })
            })?;
        self.connection.set_baud(speed)?;
        std::thread::sleep(Duration::from_secs_f32(0.05));
        self.connection.flush()?;
        Ok(())
    }

    pub fn into_serial(self) -> SystemPort {
        self.connection.into_serial()
    }
}

pub(crate) fn get_erase_size(offset: usize, size: usize) -> usize {
    let sector_count = (size + FLASH_SECTOR_SIZE - 1) / FLASH_SECTOR_SIZE;
    let start_sector = offset / FLASH_SECTOR_SIZE;

    let head_sectors = usize::min(
        FLASH_SECTORS_PER_BLOCK - (start_sector % FLASH_SECTORS_PER_BLOCK),
        sector_count,
    );

    if sector_count < 2 * head_sectors {
        (sector_count + 1) / 2 * FLASH_SECTOR_SIZE
    } else {
        (sector_count - head_sectors) * FLASH_SECTOR_SIZE
    }
}

pub(crate) const CHECKSUM_INIT: u8 = 0xEF;

pub fn checksum(data: &[u8], mut checksum: u8) -> u8 {
    for byte in data {
        checksum ^= *byte;
    }

    checksum
}
