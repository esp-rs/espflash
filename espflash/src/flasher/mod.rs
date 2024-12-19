//! Write a binary application to a target device
//!
//! The [Flasher] struct abstracts over various operations for writing a binary
//! application to a target device. It additionally provides some operations to
//! read information from the target device.

use std::{fs, path::Path, str::FromStr};

#[cfg(feature = "serialport")]
use std::{borrow::Cow, io::Write, path::PathBuf, thread::sleep, time::Duration};

use esp_idf_part::PartitionTable;

#[cfg(feature = "serialport")]
use log::{debug, info, warn};
#[cfg(feature = "serialport")]
use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};
#[cfg(feature = "serialport")]
use serialport::UsbPortInfo;
use strum::IntoEnumIterator;
use strum::{Display, EnumIter, VariantNames};

use crate::{
    error::Error,
    targets::{Chip, XtalFrequency},
};

#[cfg(feature = "serialport")]
use crate::{
    command::{Command, CommandType},
    connection::{
        reset::{ResetAfterOperation, ResetBeforeOperation},
        Connection, Port,
    },
    elf::{ElfFirmwareImage, FirmwareImage, RomSegment},
    error::{ConnectionError, ResultExt},
    flasher::stubs::{
        FlashStub, CHIP_DETECT_MAGIC_REG_ADDR, DEFAULT_TIMEOUT, EXPECTED_STUB_HANDSHAKE,
    },
};

#[cfg(feature = "serialport")]
pub use crate::targets::flash_target::ProgressCallbacks;

#[cfg(feature = "serialport")]
pub(crate) use stubs::{FLASH_SECTOR_SIZE, FLASH_WRITE_SIZE};

#[cfg(feature = "serialport")]
pub(crate) mod stubs;

/// Supported flash frequencies
///
/// Note that not all frequencies are supported by each target device.
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
#[derive(
    Debug, Default, Clone, Copy, Hash, PartialEq, Eq, Display, VariantNames, Serialize, Deserialize,
)]
#[non_exhaustive]
#[repr(u8)]
pub enum FlashFrequency {
    /// 12 MHz
    #[serde(rename = "12MHz")]
    _12Mhz,
    /// 15 MHz
    #[serde(rename = "15MHz")]
    _15Mhz,
    /// 16 MHz
    #[serde(rename = "16MHz")]
    _16Mhz,
    /// 20 MHz
    #[serde(rename = "20MHz")]
    _20Mhz,
    /// 24 MHz
    #[serde(rename = "24MHz")]
    _24Mhz,
    /// 26 MHz
    #[serde(rename = "26MHz")]
    _26Mhz,
    /// 30 MHz
    #[serde(rename = "30MHz")]
    _30Mhz,
    /// 40 MHz
    #[serde(rename = "40MHz")]
    #[default]
    _40Mhz,
    /// 48 MHz
    #[serde(rename = "48MHz")]
    _48Mhz,
    /// 60 MHz
    #[serde(rename = "60MHz")]
    _60Mhz,
    /// 80 MHz
    #[serde(rename = "80MHz")]
    _80Mhz,
}

impl FlashFrequency {
    /// Encodes flash frequency into the format used by the bootloader.
    pub fn encode_flash_frequency(self: FlashFrequency, chip: Chip) -> Result<u8, Error> {
        let encodings = chip.into_target().flash_frequency_encodings();
        if let Some(&f) = encodings.get(&self) {
            Ok(f)
        } else {
            Err(Error::UnsupportedFlashFrequency {
                chip,
                frequency: self,
            })
        }
    }
}

/// Supported flash modes
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
#[derive(Copy, Clone, Debug, Default, VariantNames, Serialize, Deserialize)]
#[non_exhaustive]
#[strum(serialize_all = "lowercase")]
#[serde(rename_all = "lowercase")]
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

/// Supported flash sizes
///
/// Note that not all sizes are supported by each target device.
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    PartialEq,
    Display,
    VariantNames,
    EnumIter,
    Serialize,
    Deserialize,
)]
#[non_exhaustive]
#[repr(u8)]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
#[doc(alias("esp_image_flash_size_t"))]
pub enum FlashSize {
    /// 256 KB
    #[serde(rename = "256KB")]
    _256Kb,
    /// 512 KB
    #[serde(rename = "512KB")]
    _512Kb,
    /// 1 MB
    #[serde(rename = "1MB")]
    _1Mb,
    /// 2 MB
    #[serde(rename = "2MB")]
    _2Mb,
    /// 4 MB
    #[default]
    #[serde(rename = "4MB")]
    _4Mb,
    /// 8 MB
    #[serde(rename = "8MB")]
    _8Mb,
    /// 16 MB
    #[serde(rename = "16MB")]
    _16Mb,
    /// 32 MB
    #[serde(rename = "32MB")]
    _32Mb,
    /// 64 MB
    #[serde(rename = "64MB")]
    _64Mb,
    /// 128 MB
    #[serde(rename = "128MB")]
    _128Mb,
    /// 256 MB
    #[serde(rename = "256MB")]
    _256Mb,
}

impl FlashSize {
    /// Encodes flash size into the format used by the bootloader.
    ///
    /// ## Values:
    ///
    /// * https://docs.espressif.com/projects/esptool/en/latest/esp32s3/advanced-topics/firmware-image-format.html#file-header
    pub const fn encode_flash_size(self: FlashSize) -> Result<u8, Error> {
        use FlashSize::*;

        let encoded = match self {
            _1Mb => 0,
            _2Mb => 1,
            _4Mb => 2,
            _8Mb => 3,
            _16Mb => 4,
            _32Mb => 5,
            _64Mb => 6,
            _128Mb => 7,
            _256Mb => 8,
            _ => return Err(Error::UnsupportedFlash(self as u8)),
        };

        Ok(encoded)
    }

    /// Create a [FlashSize] from an [u8]
    ///
    /// [source](https://github.com/espressif/esptool/blob/f4d2510e2c897621884f433ef3f191e8fc5ff184/esptool/cmds.py#L42)
    pub const fn from_detected(value: u8) -> Result<FlashSize, Error> {
        match value {
            0x12 | 0x32 => Ok(FlashSize::_256Kb),
            0x13 | 0x33 => Ok(FlashSize::_512Kb),
            0x14 | 0x34 => Ok(FlashSize::_1Mb),
            0x15 | 0x35 => Ok(FlashSize::_2Mb),
            0x16 | 0x36 => Ok(FlashSize::_4Mb),
            0x17 | 0x37 => Ok(FlashSize::_8Mb),
            0x18 | 0x38 => Ok(FlashSize::_16Mb),
            0x19 | 0x39 => Ok(FlashSize::_32Mb),
            0x20 | 0x1A | 0x3A => Ok(FlashSize::_64Mb),
            0x21 | 0x1B => Ok(FlashSize::_128Mb),
            0x22 | 0x1C => Ok(FlashSize::_256Mb),
            _ => Err(Error::UnsupportedFlash(value)),
        }
    }

    /// Returns the flash size in bytes
    pub const fn size(self) -> u32 {
        match self {
            FlashSize::_256Kb => 0x0040000,
            FlashSize::_512Kb => 0x0080000,
            FlashSize::_1Mb => 0x0100000,
            FlashSize::_2Mb => 0x0200000,
            FlashSize::_4Mb => 0x0400000,
            FlashSize::_8Mb => 0x0800000,
            FlashSize::_16Mb => 0x1000000,
            FlashSize::_32Mb => 0x2000000,
            FlashSize::_64Mb => 0x4000000,
            FlashSize::_128Mb => 0x8000000,
            FlashSize::_256Mb => 0x10000000,
        }
    }
}

impl FromStr for FlashSize {
    type Err = Error;
    /// Create a [FlashSize] from a string
    fn from_str(s: &str) -> Result<Self, Self::Err> {
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

/// Flash settings to use when flashing a device
#[derive(Copy, Clone, Debug, Serialize, Deserialize, Default)]
#[non_exhaustive]
pub struct FlashSettings {
    pub mode: Option<FlashMode>,
    pub size: Option<FlashSize>,
    #[serde(rename = "frequency")]
    pub freq: Option<FlashFrequency>,
}

impl FlashSettings {
    pub const fn default() -> Self {
        FlashSettings {
            mode: None,
            size: None,
            freq: None,
        }
    }

    pub fn new(
        mode: Option<FlashMode>,
        size: Option<FlashSize>,
        freq: Option<FlashFrequency>,
    ) -> Self {
        FlashSettings { mode, size, freq }
    }
}

/// Builder interface to create [`FlashData`] objects.
pub struct FlashDataBuilder<'a> {
    bootloader_path: Option<&'a Path>,
    partition_table_path: Option<&'a Path>,
    partition_table_offset: Option<u32>,
    target_app_partition: Option<String>,
    flash_settings: FlashSettings,
    min_chip_rev: u16,
}

impl Default for FlashDataBuilder<'_> {
    fn default() -> Self {
        Self {
            bootloader_path: Default::default(),
            partition_table_path: Default::default(),
            partition_table_offset: Default::default(),
            target_app_partition: Default::default(),
            flash_settings: FlashSettings::default(),
            min_chip_rev: Default::default(),
        }
    }
}

impl<'a> FlashDataBuilder<'a> {
    /// Creates a new [`FlashDataBuilder`] object.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the bootloader path.
    pub fn with_bootloader(mut self, bootloader_path: &'a Path) -> Self {
        self.bootloader_path = Some(bootloader_path);
        self
    }

    /// Sets the partition table path.
    pub fn with_partition_table(mut self, partition_table_path: &'a Path) -> Self {
        self.partition_table_path = Some(partition_table_path);
        self
    }

    /// Sets the partition table offset.
    pub fn with_partition_table_offset(mut self, partition_table_offset: u32) -> Self {
        self.partition_table_offset = Some(partition_table_offset);
        self
    }

    /// Sets the label of the target app partition.
    pub fn with_target_app_partition(mut self, target_app_partition: String) -> Self {
        self.target_app_partition = Some(target_app_partition);
        self
    }

    /// Sets the flash settings.
    pub fn with_flash_settings(mut self, flash_settings: FlashSettings) -> Self {
        self.flash_settings = flash_settings;
        self
    }

    /// Sets the minimum chip revision.
    pub fn with_min_chip_rev(mut self, min_chip_rev: u16) -> Self {
        self.min_chip_rev = min_chip_rev;
        self
    }

    /// Builds a [`FlashData`] object.
    pub fn build(self) -> Result<FlashData, Error> {
        FlashData::new(
            self.bootloader_path,
            self.partition_table_path,
            self.partition_table_offset,
            self.target_app_partition,
            self.flash_settings,
            self.min_chip_rev,
        )
    }
}

/// Flash data and configuration
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct FlashData {
    pub bootloader: Option<Vec<u8>>,
    pub partition_table: Option<PartitionTable>,
    pub partition_table_offset: Option<u32>,
    pub target_app_partition: Option<String>,
    pub flash_settings: FlashSettings,
    pub min_chip_rev: u16,
}

impl FlashData {
    pub fn new(
        bootloader: Option<&Path>,
        partition_table: Option<&Path>,
        partition_table_offset: Option<u32>,
        target_app_partition: Option<String>,
        flash_settings: FlashSettings,
        min_chip_rev: u16,
    ) -> Result<Self, Error> {
        // If the '--bootloader' option is provided, load the binary file at the
        // specified path.
        let bootloader = if let Some(path) = bootloader {
            let data = fs::canonicalize(path)
                .and_then(fs::read)
                .map_err(|e| Error::FileOpenError(path.display().to_string(), e))?;

            Some(data)
        } else {
            None
        };

        // If the '-T' option is provided, load the partition table from
        // the CSV or binary file at the specified path.
        let partition_table = match partition_table {
            Some(path) => Some(parse_partition_table(path)?),
            None => None,
        };

        Ok(FlashData {
            bootloader,
            partition_table,
            partition_table_offset,
            target_app_partition,
            flash_settings,
            min_chip_rev,
        })
    }
}

/// Parameters of the attached SPI flash chip (sizes, etc).
///
/// See https://github.com/espressif/esptool/blob/da31d9d7a1bb496995f8e30a6be259689948e43e/esptool.py#L655
#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct SpiSetParams {
    /// Flash chip ID
    fl_id: u32,
    /// Total size in bytes
    total_size: u32,
    /// Block size
    block_size: u32,
    /// Sector size
    sector_size: u32,
    /// Page size
    page_size: u32,
    /// Status mask
    status_mask: u32,
}

impl SpiSetParams {
    pub const fn default(size: u32) -> Self {
        SpiSetParams {
            fl_id: 0,
            total_size: size,
            block_size: 64 * 1024,
            sector_size: 4 * 1024,
            page_size: 256,
            status_mask: 0xFFFF,
        }
    }

    /// Encode the parameters into a byte array
    pub fn encode(&self) -> Vec<u8> {
        let mut encoded: Vec<u8> = Vec::new();
        encoded.extend_from_slice(&self.fl_id.to_le_bytes());
        encoded.extend_from_slice(&self.total_size.to_le_bytes());
        encoded.extend_from_slice(&self.block_size.to_le_bytes());
        encoded.extend_from_slice(&self.sector_size.to_le_bytes());
        encoded.extend_from_slice(&self.page_size.to_le_bytes());
        encoded.extend_from_slice(&self.status_mask.to_le_bytes());
        encoded
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

    // Default SPI parameters for ESP32-PICO-D4
    pub const fn esp32_pico_d4() -> Self {
        SpiAttachParams {
            clk: 6,
            q: 17,
            d: 8,
            hd: 11,
            cs: 16,
        }
    }

    /// Encode the parameters into a byte array
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

/// Information about the connected device
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    /// The chip being used
    pub chip: Chip,
    /// The revision of the chip
    pub revision: Option<(u32, u32)>,
    /// The crystal frequency of the chip
    pub crystal_frequency: XtalFrequency,
    /// The total available flash size
    pub flash_size: FlashSize,
    /// Device features
    pub features: Vec<String>,
    /// MAC address
    pub mac_address: String,
}

/// Parse a [PartitionTable] from the provided path
pub fn parse_partition_table(path: &Path) -> Result<PartitionTable, Error> {
    let data = fs::read(path).map_err(|e| Error::FileOpenError(path.display().to_string(), e))?;

    Ok(PartitionTable::try_from(data)?)
}

#[cfg(feature = "serialport")]
/// List of SPI parameters to try while detecting flash size
pub(crate) const TRY_SPI_PARAMS: [SpiAttachParams; 2] =
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
    #[allow(clippy::too_many_arguments)]
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
