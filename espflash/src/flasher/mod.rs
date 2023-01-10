//! Write a binary application to a target device
//!
//! The [Flasher] struct abstracts over various operations for writing a binary
//! application to a target device. It additionally provides some operations to
//! read information from the target device.

use std::{borrow::Cow, str::FromStr, thread::sleep};

use bytemuck::{Pod, Zeroable, __core::time::Duration};
use esp_idf_part::PartitionTable;
use log::{debug, info, warn};
use serialport::UsbPortInfo;
use strum::{Display, EnumIter, EnumVariantNames};

use self::stubs::FlashStub;
use crate::{
    command::{Command, CommandType},
    connection::Connection,
    elf::{ElfFirmwareImage, FirmwareImage, RomSegment},
    error::{ConnectionError, Error, FlashDetectError, ResultExt},
    image_format::ImageFormatKind,
    interface::Interface,
    targets::Chip,
};

mod stubs;

pub(crate) const CHECKSUM_INIT: u8 = 0xEF;
pub(crate) const FLASH_SECTOR_SIZE: usize = 0x1000;
pub(crate) const FLASH_WRITE_SIZE: usize = 0x400;

const CHIP_DETECT_MAGIC_REG_ADDR: u32 = 0x40001000;
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(3);
const EXPECTED_STUB_HANDSHAKE: &str = "OHAI";
const FLASH_BLOCK_SIZE: usize = 0x100;
const FLASH_SECTORS_PER_BLOCK: usize = FLASH_SECTOR_SIZE / FLASH_BLOCK_SIZE;

/// Supported flash frequencies
///
/// Note that not all frequencies are supported by each target device.
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
#[derive(Debug, Default, Clone, Copy, Hash, PartialEq, Eq, Display, EnumVariantNames)]
#[repr(u8)]
pub enum FlashFrequency {
    /// 12 MHz
    #[strum(serialize = "12M")]
    Flash12M,
    /// 15 MHz
    #[strum(serialize = "15M")]
    Flash15M,
    /// 16 MHz
    #[strum(serialize = "16M")]
    Flash16M,
    /// 20 MHz
    #[strum(serialize = "20M")]
    Flash20M,
    /// 24 MHz
    #[strum(serialize = "24M")]
    Flash24M,
    /// 26 MHz
    #[strum(serialize = "26M")]
    Flash26M,
    /// 30 MHz
    #[strum(serialize = "30M")]
    Flash30M,
    /// 40 MHz
    #[default]
    #[strum(serialize = "40M")]
    Flash40M,
    /// 48 MHz
    #[strum(serialize = "48M")]
    Flash48M,
    /// 60 MHz
    #[strum(serialize = "60M")]
    Flash60M,
    /// 80 MHz
    #[strum(serialize = "80M")]
    Flash80M,
}

impl FromStr for FlashFrequency {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use FlashFrequency::*;

        match s.to_uppercase().as_str() {
            "12M" => Ok(Flash12M),
            "15M" => Ok(Flash15M),
            "16M" => Ok(Flash16M),
            "20M" => Ok(Flash20M),
            "24M" => Ok(Flash24M),
            "26M" => Ok(Flash26M),
            "30M" => Ok(Flash30M),
            "40M" => Ok(Flash40M),
            "48M" => Ok(Flash48M),
            "60M" => Ok(Flash60M),
            "80M" => Ok(Flash80M),
            _ => Err(Error::InvalidFlashFrequency(s.to_string())),
        }
    }
}

/// Supported flash modes
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
#[derive(Copy, Clone, Debug, Default, EnumVariantNames)]
#[strum(serialize_all = "lowercase")]
pub enum FlashMode {
    /// Quad I/O (4 pins used for address & data)
    Qio,
    /// Quad Output (4 pins used for data)
    Qout,
    /// Dual I/O (2 pins used for address & data)
    #[default]
    Dio,
    /// Dual Output (2 pins used for data)
    Dout,
}

impl FromStr for FlashMode {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mode = match s.to_lowercase().as_str() {
            "qio" => FlashMode::Qio,
            "qout" => FlashMode::Qout,
            "dio" => FlashMode::Dio,
            "dout" => FlashMode::Dout,
            _ => return Err(Error::InvalidFlashMode(s.to_string())),
        };

        Ok(mode)
    }
}

/// Supported flash sizes
///
/// Note that not all sizes are supported by each target device.
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Display, EnumVariantNames, EnumIter)]
#[repr(u8)]
pub enum FlashSize {
    /// 256 KB
    #[strum(serialize = "256K")]
    Flash256Kb = 0x12,
    /// 512 KB
    #[strum(serialize = "512K")]
    Flash512Kb = 0x13,
    /// 1 MB
    #[strum(serialize = "1M")]
    Flash1Mb = 0x14,
    /// 2 MB
    #[strum(serialize = "2M")]
    Flash2Mb = 0x15,
    /// 4 MB
    #[default]
    #[strum(serialize = "4M")]
    Flash4Mb = 0x16,
    /// 8 MB
    #[strum(serialize = "8M")]
    Flash8Mb = 0x17,
    /// 16 MB
    #[strum(serialize = "16M")]
    Flash16Mb = 0x18,
    /// 32 MB
    #[strum(serialize = "32M")]
    Flash32Mb = 0x19,
    /// 64 MB
    #[strum(serialize = "64M")]
    Flash64Mb = 0x1a,
    /// 128 MB
    #[strum(serialize = "128M")]
    Flash128Mb = 0x21,
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
            0x21 => Ok(FlashSize::Flash128Mb),
            _ => Err(Error::UnsupportedFlash(FlashDetectError::from(value))),
        }
    }

    /// Returns the flash size in bytes
    pub fn size(self) -> u32 {
        match self {
            FlashSize::Flash256Kb => 0x0040000,
            FlashSize::Flash512Kb => 0x0080000,
            FlashSize::Flash1Mb => 0x0100000,
            FlashSize::Flash2Mb => 0x0200000,
            FlashSize::Flash4Mb => 0x0400000,
            FlashSize::Flash8Mb => 0x0800000,
            FlashSize::Flash16Mb => 0x1000000,
            FlashSize::Flash32Mb => 0x2000000,
            FlashSize::Flash64Mb => 0x4000000,
            FlashSize::Flash128Mb => 0x8000000,
        }
    }
}

impl FromStr for FlashSize {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use strum::{IntoEnumIterator, VariantNames};
        let upper = s.to_uppercase();
        FlashSize::VARIANTS
            .iter()
            .copied()
            .zip(FlashSize::iter())
            .find(|(name, _)| *name == upper)
            .map(|(_, variant)| variant)
            .ok_or_else(|| Error::InvalidFlashSize(s.to_string()))
    }
}

/// Parameters for attaching to a target devices SPI flash
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

    pub fn encode(self, stub: bool) -> Vec<u8> {
        let packed = ((self.hd as u32) << 24)
            | ((self.cs as u32) << 18)
            | ((self.d as u32) << 12)
            | ((self.q as u32) << 6)
            | (self.clk as u32);

        let mut encoded: Vec<u8> = packed.to_le_bytes().to_vec();

        if !stub {
            encoded.append(&mut vec![0u8; 4]);
        }

        encoded
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

/// Information about the connected device
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    /// The chip being used
    pub chip: Chip,
    /// The revision of the chip
    pub revision: Option<(u32, u32)>,
    /// The crystal frequency of the chip
    pub crystal_frequency: u32,
    /// The total available flash size
    pub flash_size: FlashSize,
    /// Device features
    pub features: Vec<String>,
    /// MAC address
    pub mac_address: String,
}

/// Connect to and flash a target device
pub struct Flasher {
    /// Connection for flash operations
    connection: Connection,
    /// Chip ID
    chip: Chip,
    /// Flash size, loaded from SPI flash
    flash_size: FlashSize,
    /// Configuration for SPI attached flash (0 to use fused values)
    spi_params: SpiAttachParams,
    /// Indicate RAM stub loader is in use
    use_stub: bool,
}

impl Flasher {
    pub fn connect(
        serial: Interface,
        port_info: UsbPortInfo,
        speed: Option<u32>,
        use_stub: bool,
    ) -> Result<Self, Error> {
        // Establish a connection to the device using the default baud rate of 115,200
        // and timeout of 3 seconds.
        let mut connection = Connection::new(serial, port_info);
        connection.begin()?;
        connection.set_timeout(DEFAULT_TIMEOUT)?;

        // Detect which chip we are connected to.
        let magic = connection.read_reg(CHIP_DETECT_MAGIC_REG_ADDR)?;
        let chip = Chip::from_magic(magic)?;

        let mut flasher = Flasher {
            connection,
            chip,
            flash_size: FlashSize::Flash4Mb,
            spi_params: SpiAttachParams::default(),
            use_stub,
        };

        // Load flash stub if enabled
        if use_stub {
            info!("Using flash stub");
            flasher.load_stub()?;
        }

        flasher.spi_autodetect()?;

        // Now that we have established a connection and detected the chip and flash
        // size, we can set the baud rate of the connection to the configured value.
        if let Some(baud) = speed {
            match flasher.chip {
                Chip::Esp8266 => (), // Not available
                _ => {
                    if baud > 115_200 {
                        warn!("Setting baud rate higher than 115,200 can cause issues");
                        flasher.change_baud(baud)?;
                    }
                }
            }
        }

        Ok(flasher)
    }

    /// Load flash stub
    fn load_stub(&mut self) -> Result<(), Error> {
        debug!("Loading flash stub for chip: {:?}", self.chip);

        // Load flash stub
        let stub = FlashStub::get(self.chip);

        let mut ram_target = self.chip.ram_target(
            Some(stub.entry()),
            self.chip
                .into_target()
                .max_ram_block_size(&mut self.connection)?,
        );
        ram_target.begin(&mut self.connection).flashing()?;

        let (text_addr, text) = stub.text();
        debug!("Write {} byte stub text", text.len());

        ram_target
            .write_segment(
                &mut self.connection,
                RomSegment {
                    addr: text_addr,
                    data: Cow::Borrowed(&text),
                },
                None,
            )
            .flashing()?;

        let (data_addr, data) = stub.data();
        debug!("Write {} byte stub data", data.len());

        ram_target
            .write_segment(
                &mut self.connection,
                RomSegment {
                    addr: data_addr,
                    data: Cow::Borrowed(&data),
                },
                None,
            )
            .flashing()?;

        debug!("Finish stub write");
        ram_target.finish(&mut self.connection, true).flashing()?;

        debug!("Stub written!");

        match self.connection.read(EXPECTED_STUB_HANDSHAKE.len())? {
            Some(resp) if resp == EXPECTED_STUB_HANDSHAKE.as_bytes() => Ok(()),
            _ => Err(Error::Connection(ConnectionError::InvalidStubHandshake)),
        }?;

        // Re-detect chip to check stub is up
        let magic = self.connection.read_reg(CHIP_DETECT_MAGIC_REG_ADDR)?;
        let chip = Chip::from_magic(magic)?;
        debug!("Re-detected chip: {:?}", chip);

        Ok(())
    }

    fn spi_autodetect(&mut self) -> Result<(), Error> {
        // Loop over all available SPI parameters until we find one that successfully
        // reads the flash size.
        for spi_params in TRY_SPI_PARAMS.iter().copied() {
            debug!("Attempting flash enable with: {:?}", spi_params);

            // Send `SpiAttach` to enable flash, in some instances this command
            // may fail while the flash connection succeeds
            if let Err(_e) = self.enable_flash(spi_params) {
                debug!("Flash enable failed");
            }

            if let Some(flash_size) = self.flash_detect()? {
                debug!("Flash detect OK!");

                // Flash detection was successful, so save the flash size and SPI parameters and
                // return.
                self.flash_size = flash_size;
                self.spi_params = spi_params;

                return Ok(());
            }

            debug!("Flash detect failed");
        }

        debug!("SPI flash autodetection failed");

        // None of the SPI parameters were successful.
        Err(Error::FlashConnect)
    }

    fn flash_detect(&mut self) -> Result<Option<FlashSize>, Error> {
        const FLASH_RETRY: u8 = 0xFF;

        let flash_id = self.spi_command(CommandType::FlashDetect, &[], 24)?;
        let size_id = (flash_id >> 16) as u8;

        // This value indicates that an alternate detection method should be tried.
        if size_id == FLASH_RETRY {
            return Ok(None);
        }

        let flash_size = match FlashSize::from(size_id) {
            Ok(size) => size,
            Err(_) => {
                warn!(
                    "Could not detect flash size (FlashID=0x{:02X}, SizeID=0x{:02X}), defaulting to 4MB",
                    flash_id,
                    size_id
                );
                FlashSize::Flash4Mb
            }
        };

        Ok(Some(flash_size))
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
                        connection.command(if self.use_stub {
                            Command::SpiAttachStub { spi_params }
                        } else {
                            Command::SpiAttach { spi_params }
                        })
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

        let spi_registers = self.chip.into_target().spi_registers();

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

    /// Read and print any information we can about the connected device
    pub fn device_info(&mut self) -> Result<DeviceInfo, Error> {
        let chip = self.chip();
        let target = chip.into_target();

        // The ESP8266 does not have readable major/minor revision numbers, so we have
        // nothing to return if targeting it.
        let revision = if chip != Chip::Esp8266 {
            Some(target.chip_revision(self.connection())?)
        } else {
            None
        };

        let crystal_frequency = target.crystal_freq(self.connection())?;
        let features = target
            .chip_features(self.connection())?
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>();
        let mac_address = target.mac_address(self.connection())?;

        let info = DeviceInfo {
            chip,
            revision,
            crystal_frequency,
            flash_size: self.flash_size,
            features,
            mac_address,
        };

        Ok(info)
    }

    /// Load an elf image to ram and execute it
    ///
    /// Note that this will not touch the flash on the device
    pub fn load_elf_to_ram(&mut self, elf_data: &[u8]) -> Result<(), Error> {
        let image = ElfFirmwareImage::try_from(elf_data)?;
        if image.rom_segments(self.chip).next().is_some() {
            return Err(Error::ElfNotRamLoadable);
        }

        let mut target = self.chip.ram_target(
            Some(image.entry()),
            self.chip
                .into_target()
                .max_ram_block_size(&mut self.connection)?,
        );
        target.begin(&mut self.connection).flashing()?;

        for segment in image.ram_segments(self.chip) {
            // Only display progress bars when the "cli" feature is enabled.
            let progress_cb = if cfg!(feature = "cli") {
                use crate::cli::{build_progress_bar_callback, progress_bar};

                let progress = progress_bar(format!("segment 0x{:X}", segment.addr), None);
                let progress_cb = build_progress_bar_callback(progress);

                Some(progress_cb)
            } else {
                None
            };

            target
                .write_segment(&mut self.connection, segment.into(), progress_cb)
                .flashing()?;
        }

        target.finish(&mut self.connection, true).flashing()
    }

    /// Load an elf image to flash and execute it
    pub fn load_elf_to_flash_with_format(
        &mut self,
        elf_data: &[u8],
        bootloader: Option<Vec<u8>>,
        partition_table: Option<PartitionTable>,
        image_format: Option<ImageFormatKind>,
        flash_mode: Option<FlashMode>,
        flash_size: Option<FlashSize>,
        flash_freq: Option<FlashFrequency>,
    ) -> Result<(), Error> {
        let image = ElfFirmwareImage::try_from(elf_data)?;

        let mut target = self.chip.flash_target(self.spi_params, self.use_stub);
        target.begin(&mut self.connection).flashing()?;

        let image = self.chip.into_target().get_flash_image(
            &image,
            bootloader,
            partition_table,
            image_format,
            Some(
                self.chip
                    .into_target()
                    .chip_revision(&mut self.connection)?,
            ),
            flash_mode,
            flash_size.or(Some(self.flash_size)),
            flash_freq,
        )?;

        // When the "cli" feature is enabled, display the image size information.
        #[cfg(feature = "cli")]
        crate::cli::display_image_size(image.app_size(), image.part_size());

        for segment in image.flash_segments() {
            // Only display progress bars when the "cli" feature is enabled.
            let progress_cb = if cfg!(feature = "cli") {
                use crate::cli::{build_progress_bar_callback, progress_bar};

                let progress = progress_bar(format!("segment 0x{:X}", segment.addr), None);
                let progress_cb = build_progress_bar_callback(progress);

                Some(progress_cb)
            } else {
                None
            };

            target
                .write_segment(&mut self.connection, segment, progress_cb)
                .flashing()?;
        }

        target.finish(&mut self.connection, true).flashing()?;

        Ok(())
    }

    /// Load an bin image to flash at a specific address
    pub fn write_bin_to_flash(
        &mut self,
        addr: u32,
        data: &[u8],
        progress_cb: Option<Box<dyn Fn(usize, usize)>>,
    ) -> Result<(), Error> {
        let segment = RomSegment {
            addr,
            data: Cow::from(data),
        };

        let mut target = self.chip.flash_target(self.spi_params, self.use_stub);
        target.begin(&mut self.connection).flashing()?;
        target.write_segment(&mut self.connection, segment, progress_cb)?;
        target.finish(&mut self.connection, true).flashing()?;

        Ok(())
    }

    /// Load an elf image to flash and execute it
    pub fn load_elf_to_flash(
        &mut self,
        elf_data: &[u8],
        bootloader: Option<Vec<u8>>,
        partition_table: Option<PartitionTable>,
        flash_mode: Option<FlashMode>,
        flash_size: Option<FlashSize>,
        flash_freq: Option<FlashFrequency>,
    ) -> Result<(), Error> {
        self.load_elf_to_flash_with_format(
            elf_data,
            bootloader,
            partition_table,
            None,
            flash_mode,
            flash_size,
            flash_freq,
        )
    }

    pub fn change_baud(&mut self, speed: u32) -> Result<(), Error> {
        debug!("Change baud to: {}", speed);

        let prior_baud = match self.use_stub {
            true => self.connection.get_baud()?,
            false => 0,
        };

        let target = self.chip.into_target();
        let xtal_freq = target.crystal_freq(&mut self.connection)?;

        // Probably this is just a temporary solution until the next chip revision.
        //
        // The ROM code thinks it uses a 40 MHz XTAL. Recompute the baud rate in order
        // to trick the ROM code to set the correct baud rate for a 26 MHz XTAL.
        let mut new_baud = speed;
        if self.chip == Chip::Esp32c2 && !self.use_stub && xtal_freq == 26 {
            new_baud = new_baud * 40 / 26;
        }

        self.connection
            .with_timeout(CommandType::ChangeBaud.timeout(), |connection| {
                connection.command(Command::ChangeBaud {
                    new_baud,
                    prior_baud,
                })
            })?;
        self.connection.set_baud(speed)?;
        sleep(Duration::from_secs_f32(0.05));
        self.connection.flush()?;

        Ok(())
    }

    pub fn get_usb_pid(&self) -> Result<u16, Error> {
        self.connection.get_usb_pid()
    }

    pub fn erase_region(&mut self, offset: u32, size: u32) -> Result<(), Error> {
        debug!("Erasing region of 0x{:x}B at 0x{:08x}", size, offset);

        self.connection
            .with_timeout(CommandType::EraseRegion.timeout(), |connection| {
                connection.command(Command::EraseRegion { offset, size })
            })?;
        std::thread::sleep(Duration::from_secs_f32(0.05));
        self.connection.flush()?;
        Ok(())
    }

    pub fn into_interface(self) -> Interface {
        self.connection.into_interface()
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

pub(crate) fn checksum(data: &[u8], mut checksum: u8) -> u8 {
    for byte in data {
        checksum ^= *byte;
    }

    checksum
}
