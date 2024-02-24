//! Write a binary application to a target device
//!
//! The [Flasher] struct abstracts over various operations for writing a binary
//! application to a target device. It additionally provides some operations to
//! read information from the target device.

use std::{borrow::Cow, fs, io::Write, path::PathBuf, thread::sleep, time::Duration};

use log::{debug, info, warn};
use md5::{Digest, Md5};
#[cfg(feature = "serialport")]
use serialport::UsbPortInfo;

use self::stubs::FlashStub;
#[cfg(feature = "serialport")]
use crate::connection::{
    reset::{ResetAfterOperation, ResetBeforeOperation},
    Connection, Port,
};
use crate::{
    command::{Command, CommandType},
    elf::{ElfFirmwareImage, FirmwareImage, RomSegment},
    error::{ConnectionError, Error, ResultExt},
    flash_data::{DeviceInfo, FlashData, FlashSize, SpiAttachParams, SpiSetParams},
    progress::ProgressCallbacks,
    targets::{Chip, XtalFrequency},
};

mod stubs;

const CHIP_DETECT_MAGIC_REG_ADDR: u32 = 0x40001000;
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(3);
const EXPECTED_STUB_HANDSHAKE: &str = "OHAI";

/// List of SPI parameters to try while detecting flash size
const TRY_SPI_PARAMS: [SpiAttachParams; 2] =
    [SpiAttachParams::default(), SpiAttachParams::esp32_pico_d4()];

#[cfg(feature = "serialport")]
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
    /// Indicate verifying flash contents after flashing
    verify: bool,
    /// Indicate skipping of already flashed regions
    skip: bool,
}

#[cfg(feature = "serialport")]
impl Flasher {
    pub fn connect(
        serial: Port,
        port_info: UsbPortInfo,
        speed: Option<u32>,
        use_stub: bool,
        verify: bool,
        skip: bool,
        chip: Option<Chip>,
        after_operation: ResetAfterOperation,
        before_operation: ResetBeforeOperation,
    ) -> Result<Self, Error> {
        // Establish a connection to the device using the default baud rate of 115,200
        // and timeout of 3 seconds.
        let mut connection = Connection::new(serial, port_info, after_operation, before_operation);
        connection.begin()?;
        connection.set_timeout(DEFAULT_TIMEOUT)?;

        let detected_chip = if before_operation != ResetBeforeOperation::NoResetNoSync {
            // Detect which chip we are connected to.
            let magic = connection.read_reg(CHIP_DETECT_MAGIC_REG_ADDR)?;
            let detected_chip = Chip::from_magic(magic)?;
            if let Some(chip) = chip {
                if chip != detected_chip {
                    return Err(Error::ChipMismatch(
                        chip.to_string(),
                        detected_chip.to_string(),
                    ));
                }
            }
            detected_chip
        } else if before_operation == ResetBeforeOperation::NoResetNoSync && chip.is_some() {
            chip.unwrap()
        } else {
            return Err(Error::ChipNotProvided);
        };

        let mut flasher = Flasher {
            connection,
            chip: detected_chip,
            flash_size: FlashSize::_4Mb,
            spi_params: SpiAttachParams::default(),
            use_stub,
            verify,
            skip,
        };

        if before_operation == ResetBeforeOperation::NoResetNoSync {
            return Ok(flasher);
        }

        // Load flash stub if enabled
        if use_stub {
            info!("Using flash stub");
            flasher.load_stub()?;
        }

        flasher.spi_autodetect()?;

        // Now that we have established a connection and detected the chip and flash
        // size, we can set the baud rate of the connection to the configured value.
        if let Some(baud) = speed {
            if baud > 115_200 {
                warn!("Setting baud rate higher than 115,200 can cause issues");
                flasher.change_baud(baud)?;
            }
        }

        Ok(flasher)
    }

    pub fn set_flash_size(&mut self, flash_size: FlashSize) {
        self.flash_size = flash_size;
    }

    pub fn disable_watchdog(&mut self) -> Result<(), Error> {
        let mut target = self
            .chip
            .flash_target(self.spi_params, self.use_stub, false, false);
        target.begin(&mut self.connection).flashing()?;
        Ok(())
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
                &mut None,
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
                &mut None,
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

                let spi_set_params = SpiSetParams::default(self.flash_size.size());
                self.connection.with_timeout(
                    CommandType::SpiSetParams.timeout(),
                    |connection| {
                        connection.command(Command::SpiSetParams {
                            spi_params: spi_set_params,
                        })
                    },
                )?;

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

        let flash_size = match FlashSize::from_detected(size_id) {
            Ok(size) => size,
            Err(_) => {
                warn!(
                    "Could not detect flash size (FlashID=0x{:02X}, SizeID=0x{:02X}), defaulting to 4MB",
                    flash_id,
                    size_id
                );
                FlashSize::default()
            }
        };

        Ok(Some(flash_size))
    }

    fn enable_flash(&mut self, spi_params: SpiAttachParams) -> Result<(), Error> {
        self.connection
            .with_timeout(CommandType::SpiAttach.timeout(), |connection| {
                connection.command(if self.use_stub {
                    Command::SpiAttachStub { spi_params }
                } else {
                    Command::SpiAttach { spi_params }
                })
            })?;

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

        let revision = Some(target.chip_revision(self.connection())?);
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

    /// Load an ELF image to RAM and execute it
    ///
    /// Note that this will not touch the flash on the device
    pub fn load_elf_to_ram(
        &mut self,
        elf_data: &[u8],
        mut progress: Option<&mut dyn ProgressCallbacks>,
    ) -> Result<(), Error> {
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
            target
                .write_segment(&mut self.connection, segment.into(), &mut progress)
                .flashing()?;
        }

        target.finish(&mut self.connection, true).flashing()
    }

    /// Load an ELF image to flash and execute it
    pub fn load_elf_to_flash(
        &mut self,
        elf_data: &[u8],
        flash_data: FlashData,
        mut progress: Option<&mut dyn ProgressCallbacks>,
        xtal_freq: XtalFrequency,
    ) -> Result<(), Error> {
        let image = ElfFirmwareImage::try_from(elf_data)?;

        let mut target =
            self.chip
                .flash_target(self.spi_params, self.use_stub, self.verify, self.skip);
        target.begin(&mut self.connection).flashing()?;

        let chip_revision = Some(
            self.chip
                .into_target()
                .chip_revision(&mut self.connection)?,
        );

        let image = self.chip.into_target().get_flash_image(
            &image,
            flash_data,
            chip_revision,
            xtal_freq,
        )?;

        // When the `cli` feature is enabled, display the image size information.
        #[cfg(feature = "cli")]
        crate::cli::display_image_size(image.app_size(), image.part_size());

        for segment in image.flash_segments() {
            target
                .write_segment(&mut self.connection, segment, &mut progress)
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
        progress: Option<&mut dyn ProgressCallbacks>,
    ) -> Result<(), Error> {
        let segment = RomSegment {
            addr,
            data: Cow::from(data),
        };
        self.write_bins_to_flash(&[segment], progress)?;

        info!("Binary successfully written to flash!");

        Ok(())
    }

    /// Load multiple bin images to flash at specific addresses
    pub fn write_bins_to_flash(
        &mut self,
        segments: &[RomSegment],
        mut progress: Option<&mut dyn ProgressCallbacks>,
    ) -> Result<(), Error> {
        let mut target = self
            .chip
            .flash_target(self.spi_params, self.use_stub, false, false);
        target.begin(&mut self.connection).flashing()?;
        for segment in segments {
            target.write_segment(&mut self.connection, segment.borrow(), &mut progress)?;
        }
        target.finish(&mut self.connection, true).flashing()?;

        Ok(())
    }

    /// Get MD5 of region
    pub fn checksum_md5(&mut self, addr: u32, length: u32) -> Result<u128, Error> {
        self.connection
            .with_timeout(CommandType::FlashMd5.timeout(), |connection| {
                connection
                    .command(crate::command::Command::FlashMd5 {
                        offset: addr,
                        size: length,
                    })?
                    .try_into()
            })
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
        if self.chip == Chip::Esp32c2 && !self.use_stub && xtal_freq == XtalFrequency::_26Mhz {
            new_baud = new_baud * 40 / 26;
        }

        self.connection
            .with_timeout(CommandType::ChangeBaudrate.timeout(), |connection| {
                connection.command(Command::ChangeBaudrate {
                    new_baud,
                    prior_baud,
                })
            })?;
        self.connection.set_baud(speed)?;
        sleep(Duration::from_secs_f32(0.05));
        self.connection.flush()?;

        Ok(())
    }

    pub fn into_serial(self) -> Port {
        self.connection.into_serial()
    }

    pub fn get_usb_pid(&self) -> Result<u16, Error> {
        self.connection.get_usb_pid()
    }

    pub fn erase_region(&mut self, offset: u32, size: u32) -> Result<(), Error> {
        debug!("Erasing region of 0x{:x}B at 0x{:08x}", size, offset);

        self.connection.with_timeout(
            CommandType::EraseRegion.timeout_for_size(size),
            |connection| connection.command(Command::EraseRegion { offset, size }),
        )?;
        std::thread::sleep(Duration::from_secs_f32(0.05));
        self.connection.flush()?;
        Ok(())
    }

    pub fn erase_flash(&mut self) -> Result<(), Error> {
        debug!("Erasing the entire flash");

        self.connection
            .with_timeout(CommandType::EraseFlash.timeout(), |connection| {
                connection.command(Command::EraseFlash)
            })?;
        sleep(Duration::from_secs_f32(0.05));
        self.connection.flush()?;

        Ok(())
    }

    pub fn read_flash(
        &mut self,
        offset: u32,
        size: u32,
        block_size: u32,
        max_in_flight: u32,
        file_path: PathBuf,
    ) -> Result<(), Error> {
        debug!("Reading 0x{:x}B from 0x{:08x}", size, offset);

        let mut data = Vec::new();

        let mut file = fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(&file_path)?;

        self.connection
            .with_timeout(CommandType::ReadFlash.timeout(), |connection| {
                connection.command(Command::ReadFlash {
                    offset,
                    size,
                    block_size,
                    max_in_flight,
                })
            })?;

        while data.len() < size as usize {
            let response = self.connection.read_response()?;
            let chunk: Vec<u8> = if let Some(response) = response {
                response.value.try_into().unwrap()
            } else {
                return Err(Error::IncorrectReposnse);
            };

            data.extend_from_slice(&chunk);

            if data.len() < size as usize && chunk.len() < block_size as usize {
                return Err(Error::CorruptData(block_size as usize, chunk.len()));
            }

            self.connection.write_raw(data.len() as u32)?;
        }

        if data.len() > size as usize {
            return Err(Error::ReadMoreThanExpected);
        }

        let response = self.connection.read_response()?;
        let digest: Vec<u8> = if let Some(response) = response {
            response.value.try_into().unwrap()
        } else {
            return Err(Error::IncorrectReposnse);
        };

        if digest.len() != 16 {
            return Err(Error::IncorrectDigestLength(digest.len()));
        }

        let mut md5_hasher = Md5::new();
        md5_hasher.update(&data);
        let checksum_md5 = md5_hasher.finalize();

        if digest != checksum_md5.as_slice() {
            return Err(Error::DigestMissmatch(
                digest,
                checksum_md5.as_slice().to_vec(),
            ));
        }

        file.write_all(&data)?;

        info!(
            "Flash content successfully read and written to '{}'!",
            file_path.display()
        );

        Ok(())
    }

    pub fn verify_minimum_revision(&mut self, minimum: u16) -> Result<(), Error> {
        let (major, minor) = self.chip.into_target().chip_revision(self.connection())?;
        let revision = (major * 100 + minor) as u16;
        if revision < minimum {
            return Err(Error::UnsupportedChipRevision {
                major: minimum / 100,
                minor: minimum % 100,
                found_major: revision / 100,
                found_minor: revision % 100,
            });
        }

        Ok(())
    }
}

pub(crate) fn checksum(data: &[u8], mut checksum: u8) -> u8 {
    for byte in data {
        checksum ^= *byte;
    }

    checksum
}
