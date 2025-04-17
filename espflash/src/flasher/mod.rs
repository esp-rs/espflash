//! Write a binary application to a target device
//!
//! The [Flasher] struct abstracts over various operations for writing a binary
//! application to a target device. It additionally provides some operations to
//! read information from the target device.

#[cfg(feature = "serialport")]
use std::{borrow::Cow, io::Write, path::PathBuf, thread::sleep, time::Duration};
use std::{collections::HashMap, fmt, fs, path::Path, str::FromStr};

use esp_idf_part::PartitionTable;
#[cfg(feature = "serialport")]
use log::{debug, info, warn};
#[cfg(feature = "serialport")]
use md5::{Digest, Md5};
#[cfg(feature = "serialport")]
use object::{Endianness, read::elf::ElfFile32 as ElfFile};
use serde::{Deserialize, Serialize};
#[cfg(feature = "serialport")]
use serialport::UsbPortInfo;
use strum::{Display, EnumIter, IntoEnumIterator, VariantNames};

#[cfg(feature = "serialport")]
pub(crate) use self::stubs::{FLASH_SECTOR_SIZE, FLASH_WRITE_SIZE};
#[cfg(feature = "serialport")]
pub use crate::targets::flash_target::ProgressCallbacks;
use crate::{
    Error,
    targets::{Chip, XtalFrequency},
};
#[cfg(feature = "serialport")]
use crate::{
    connection::{
        Connection,
        Port,
        command::{Command, CommandType},
        reset::{ResetAfterOperation, ResetBeforeOperation},
    },
    error::{ConnectionError, ResultExt as _},
    flasher::stubs::{
        CHIP_DETECT_MAGIC_REG_ADDR,
        DEFAULT_TIMEOUT,
        EXPECTED_STUB_HANDSHAKE,
        FlashStub,
    },
    image_format::{Segment, ram_segments, rom_segments},
};

#[cfg(feature = "serialport")]
pub(crate) mod stubs;

/// List of SPI parameters to try while detecting flash size
#[cfg(feature = "serialport")]
pub(crate) const TRY_SPI_PARAMS: [SpiAttachParams; 2] =
    [SpiAttachParams::default(), SpiAttachParams::esp32_pico_d4()];

/// Security Info Response containing
#[derive(Debug)]
pub struct SecurityInfo {
    /// 32 bits flags
    pub flags: u32,
    /// 1 byte flash_crypt_cnt
    pub flash_crypt_cnt: u8,
    /// 7 bytes key purposes
    pub key_purposes: [u8; 7],
    /// 32-bit word chip id
    pub chip_id: Option<u32>,
    /// 32-bit word eco version
    pub eco_version: Option<u32>,
}

impl SecurityInfo {
    fn security_flag_map() -> HashMap<&'static str, u32> {
        HashMap::from([
            ("SECURE_BOOT_EN", 1 << 0),
            ("SECURE_BOOT_AGGRESSIVE_REVOKE", 1 << 1),
            ("SECURE_DOWNLOAD_ENABLE", 1 << 2),
            ("SECURE_BOOT_KEY_REVOKE0", 1 << 3),
            ("SECURE_BOOT_KEY_REVOKE1", 1 << 4),
            ("SECURE_BOOT_KEY_REVOKE2", 1 << 5),
            ("SOFT_DIS_JTAG", 1 << 6),
            ("HARD_DIS_JTAG", 1 << 7),
            ("DIS_USB", 1 << 8),
            ("DIS_DOWNLOAD_DCACHE", 1 << 9),
            ("DIS_DOWNLOAD_ICACHE", 1 << 10),
        ])
    }

    fn security_flag_status(&self, flag_name: &str) -> bool {
        if let Some(&flag) = Self::security_flag_map().get(flag_name) {
            (self.flags & flag) != 0
        } else {
            false
        }
    }
}

impl TryFrom<&[u8]> for SecurityInfo {
    type Error = Error;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        let esp32s2 = bytes.len() == 12;

        if bytes.len() < 12 {
            return Err(Error::InvalidResponse(format!(
                "expected response of at least 12 bytes, received {} bytes",
                bytes.len()
            )));
        }

        // Parse response bytes
        let flags = u32::from_le_bytes(bytes[0..4].try_into()?);
        let flash_crypt_cnt = bytes[4];
        let key_purposes: [u8; 7] = bytes[5..12].try_into()?;

        let (chip_id, eco_version) = if esp32s2 {
            (None, None) // ESP32-S2 doesn't have these values
        } else {
            if bytes.len() < 20 {
                return Err(Error::InvalidResponse(format!(
                    "expected response of at least 20 bytes, received {} bytes",
                    bytes.len()
                )));
            }
            let chip_id = u32::from_le_bytes(bytes[12..16].try_into()?);
            let eco_version = u32::from_le_bytes(bytes[16..20].try_into()?);
            (Some(chip_id), Some(eco_version))
        };

        Ok(SecurityInfo {
            flags,
            flash_crypt_cnt,
            key_purposes,
            chip_id,
            eco_version,
        })
    }
}

impl fmt::Display for SecurityInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let key_purposes_str = self
            .key_purposes
            .iter()
            .map(|b| format!("{}", b))
            .collect::<Vec<_>>()
            .join(", ");

        writeln!(f, "\nSecurity Information:")?;
        writeln!(f, "=====================")?;
        writeln!(f, "Flags: {:#010x} ({:b})", self.flags, self.flags)?;
        writeln!(f, "Key Purposes: [{}]", key_purposes_str)?;

        // Only print Chip ID if it's Some(value)
        if let Some(chip_id) = self.chip_id {
            writeln!(f, "Chip ID: {}", chip_id)?;
        }

        // Only print API Version if it's Some(value)
        if let Some(api_version) = self.eco_version {
            writeln!(f, "API Version: {}", api_version)?;
        }

        // Secure Boot
        if self.security_flag_status("SECURE_BOOT_EN") {
            writeln!(f, "Secure Boot: Enabled")?;
            if self.security_flag_status("SECURE_BOOT_AGGRESSIVE_REVOKE") {
                writeln!(f, "Secure Boot Aggressive key revocation: Enabled")?;
            }

            let revoked_keys: Vec<_> = [
                "SECURE_BOOT_KEY_REVOKE0",
                "SECURE_BOOT_KEY_REVOKE1",
                "SECURE_BOOT_KEY_REVOKE2",
            ]
            .iter()
            .enumerate()
            .filter(|(_, key)| self.security_flag_status(key))
            .map(|(i, _)| format!("Secure Boot Key{} is Revoked", i))
            .collect();

            if !revoked_keys.is_empty() {
                writeln!(
                    f,
                    "Secure Boot Key Revocation Status:\n  {}",
                    revoked_keys.join("\n  ")
                )?;
            }
        } else {
            writeln!(f, "Secure Boot: Disabled")?;
        }

        // Flash Encryption
        if self.flash_crypt_cnt.count_ones() % 2 != 0 {
            writeln!(f, "Flash Encryption: Enabled")?;
        } else {
            writeln!(f, "Flash Encryption: Disabled")?;
        }

        let crypt_cnt_str = "SPI Boot Crypt Count (SPI_BOOT_CRYPT_CNT)";
        writeln!(f, "{}: 0x{:x}", crypt_cnt_str, self.flash_crypt_cnt)?;

        // Cache Disabling
        if self.security_flag_status("DIS_DOWNLOAD_DCACHE") {
            writeln!(f, "Dcache in UART download mode: Disabled")?;
        }
        if self.security_flag_status("DIS_DOWNLOAD_ICACHE") {
            writeln!(f, "Icache in UART download mode: Disabled")?;
        }

        // JTAG Status
        if self.security_flag_status("HARD_DIS_JTAG") {
            writeln!(f, "JTAG: Permanently Disabled")?;
        } else if self.security_flag_status("SOFT_DIS_JTAG") {
            writeln!(f, "JTAG: Software Access Disabled")?;
        }

        // USB Access
        if self.security_flag_status("DIS_USB") {
            writeln!(f, "USB Access: Disabled")?;
        }

        Ok(())
    }
}

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
    /// * <https://docs.espressif.com/projects/esptool/en/latest/esp32s3/advanced-topics/firmware-image-format.html#file-header>
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
    /// [source](https://github.com/espressif/esptool/blob/f4d2510/esptool/cmds.py#L42)
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

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        FlashSize::VARIANTS
            .iter()
            .copied()
            .zip(FlashSize::iter())
            .find(|(name, _)| *name == s.to_uppercase())
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
    pub mmu_page_size: Option<u32>,
}

impl FlashData {
    pub fn new(
        bootloader: Option<&Path>,
        partition_table: Option<&Path>,
        partition_table_offset: Option<u32>,
        target_app_partition: Option<String>,
        flash_settings: FlashSettings,
        min_chip_rev: u16,
        mmu_page_size: Option<u32>,
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
        let partition_table = if let Some(path) = partition_table {
            let data =
                fs::read(path).map_err(|e| Error::FileOpenError(path.display().to_string(), e))?;

            Some(PartitionTable::try_from(data)?)
        } else {
            None
        };

        Ok(FlashData {
            bootloader,
            partition_table,
            partition_table_offset,
            target_app_partition,
            flash_settings,
            min_chip_rev,
            mmu_page_size,
        })
    }
}

/// Parameters of the attached SPI flash chip (sizes, etc).
///
/// See: <https://github.com/espressif/esptool/blob/da31d9d/esptool.py#L655>
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
    pub mac_address: Option<String>,
}

/// Connect to and flash a target device
#[cfg(feature = "serialport")]
#[derive(Debug)]
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
    /// The serial port's baud rate should be 115_200 to connect. After
    /// connecting, Flasher will change the baud rate to the `speed`
    /// parameter.
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
            let detected_chip = detect_chip(&mut connection, use_stub)?;
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

        if !flasher.connection.secure_download_mode {
            // Load flash stub if enabled.
            if use_stub {
                info!("Using flash stub");
                flasher.load_stub()?;
            }
            // Flash size autodetection doesn't work in Secure Download Mode.
            flasher.spi_autodetect()?;
        } else if use_stub {
            warn!("Stub is not supported in Secure Download Mode, setting --no-stub");
            flasher.use_stub = false;
        }

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
                Segment {
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
                Segment {
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
        let chip = detect_chip(&mut self.connection, self.use_stub)?;
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

    pub fn flash_detect(&mut self) -> Result<Option<FlashSize>, Error> {
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
                    flash_id, size_id
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
            .write_reg(spi_registers.usr2(), (7 << 28) | command as u32, None)?;

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
                (miso_mask << 8) | (mosi_mask << 17),
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

        // chip_revision reads from efuse, which is not possible in Secure Download Mode
        let revision = (!self.connection.secure_download_mode)
            .then(|| target.chip_revision(self.connection()))
            .transpose()?;

        let crystal_frequency = target.crystal_freq(self.connection())?;
        let features = target
            .chip_features(self.connection())?
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>();

        let mac_address = (!self.connection.secure_download_mode)
            .then(|| target.mac_address(self.connection()))
            .transpose()?;

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
        let elf = ElfFile::parse(elf_data)?;
        if rom_segments(self.chip, &elf).next().is_some() {
            return Err(Error::ElfNotRamLoadable);
        }

        let mut target = self.chip.ram_target(
            Some(elf.elf_header().e_entry.get(Endianness::Little)),
            self.chip
                .into_target()
                .max_ram_block_size(&mut self.connection)?,
        );
        target.begin(&mut self.connection).flashing()?;

        for segment in ram_segments(self.chip, &elf) {
            target
                .write_segment(&mut self.connection, segment, &mut progress)
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
        let mut target =
            self.chip
                .flash_target(self.spi_params, self.use_stub, self.verify, self.skip);
        target.begin(&mut self.connection).flashing()?;

        let chip_revision = Some(
            self.chip
                .into_target()
                .chip_revision(&mut self.connection)?,
        );

        let image =
            self.chip
                .into_target()
                .flash_image(elf_data, flash_data, chip_revision, xtal_freq)?;

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
        let segment = Segment {
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
        segments: &[Segment<'_>],
        mut progress: Option<&mut dyn ProgressCallbacks>,
    ) -> Result<(), Error> {
        if self.connection.secure_download_mode {
            return Err(Error::UnsupportedFeature {
                chip: self.chip,
                feature: "writing binaries in Secure Download Mode currently".into(),
            });
        }

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
                    .command(Command::FlashMd5 {
                        offset: addr,
                        size: length,
                    })?
                    .try_into()
            })
    }

    /// Get security info
    pub fn security_info(&mut self) -> Result<SecurityInfo, Error> {
        security_info(&mut self.connection, self.use_stub)
    }

    pub fn change_baud(&mut self, speed: u32) -> Result<(), Error> {
        debug!("Change baud to: {}", speed);

        let prior_baud = match self.use_stub {
            true => self.connection.baud()?,
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

    pub fn usb_pid(&self) -> u16 {
        self.connection.usb_pid()
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

    pub fn read_flash_rom(
        &mut self,
        offset: u32,
        size: u32,
        block_size: u32,
        max_in_flight: u32,
        file_path: PathBuf,
    ) -> Result<(), Error> {
        // ROM read limit per command
        const BLOCK_LEN: usize = 64;

        let mut data: Vec<u8> = Vec::new();

        let mut file = fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(&file_path)?;

        let mut correct_offset = offset;

        while data.len() < size as usize {
            let block_len = std::cmp::min(BLOCK_LEN, size as usize - data.len());

            correct_offset += data.len() as u32;

            let response = self.connection.with_timeout(
                CommandType::ReadFlashSlow.timeout(),
                |connection| {
                    connection.command(Command::ReadFlashSlow {
                        offset: correct_offset,
                        size: block_len as u32,
                        block_size,
                        max_in_flight,
                    })
                },
            )?;

            let payload: Vec<u8> = response.try_into()?;

            assert!(payload.len() >= block_len);

            // command always returns 64 byte buffer,
            // regardless of how many bytes were actually read from flash
            data.append(&mut payload[..block_len].to_vec());
        }

        file.write_all(&data)?;

        info!(
            "Flash content successfully read and written to '{}'!",
            file_path.display()
        );

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
            let response = self.connection.read_flash_response()?;
            let chunk: Vec<u8> = if let Some(response) = response {
                response.value.try_into()?
            } else {
                return Err(Error::IncorrectResponse);
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

        let response = self.connection.read_flash_response()?;
        let digest: Vec<u8> = if let Some(response) = response {
            response.value.try_into()?
        } else {
            return Err(Error::IncorrectResponse);
        };

        if digest.len() != 16 {
            return Err(Error::IncorrectDigestLength(digest.len()));
        }

        let mut md5_hasher = Md5::new();
        md5_hasher.update(&data);
        let checksum_md5 = md5_hasher.finalize();

        if digest != checksum_md5.as_slice() {
            return Err(Error::DigestMismatch(
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

#[cfg(feature = "serialport")]
fn security_info(connection: &mut Connection, use_stub: bool) -> Result<SecurityInfo, Error> {
    connection.with_timeout(CommandType::GetSecurityInfo.timeout(), |connection| {
        let response = connection.command(Command::GetSecurityInfo)?;
        // Extract raw bytes and convert them into `SecurityInfo`
        if let crate::connection::CommandResponseValue::Vector(data) = response {
            // HACK: Not quite sure why there seem to be 4 extra bytes at the end of the
            //       response when the stub is not being used...
            let end = if use_stub { data.len() } else { data.len() - 4 };
            SecurityInfo::try_from(&data[..end])
        } else {
            Err(Error::InvalidResponse(
                "response was not a vector of bytes".into(),
            ))
        }
    })
}

#[cfg(feature = "serialport")]
fn detect_chip(connection: &mut Connection, use_stub: bool) -> Result<Chip, Error> {
    match security_info(connection, use_stub) {
        Ok(info) if info.chip_id.is_some() => {
            let chip_id = info.chip_id.unwrap() as u16;
            let chip = Chip::try_from(chip_id)?;

            Ok(chip)
        }
        _ => {
            let magic = connection.read_reg(CHIP_DETECT_MAGIC_REG_ADDR)?;
            let chip = Chip::from_magic(magic)?;

            Ok(chip)
        }
    }
}
