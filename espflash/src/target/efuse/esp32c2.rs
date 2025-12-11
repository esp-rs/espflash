//! eFuse field definitions for the esp32c2
//!
//! This file was automatically generated, please do not edit it manually!
//!
//! Generated: 2025-12-08 14:48
//! Version:   897499b0349a608b895d467abbcf006b

#![allow(unused)]

use super::{EfuseBlock, EfuseField};

/// All eFuse blocks available on this device.
pub(crate) const BLOCKS: &[EfuseBlock] = &[
    EfuseBlock {
        index: 0u8,
        length: 2u8,
        read_address: 0x6000882cu32,
        write_address: 0x60008800u32,
    },
    EfuseBlock {
        index: 1u8,
        length: 3u8,
        read_address: 0x60008834u32,
        write_address: 0x60008800u32,
    },
    EfuseBlock {
        index: 2u8,
        length: 8u8,
        read_address: 0x60008840u32,
        write_address: 0x60008800u32,
    },
    EfuseBlock {
        index: 3u8,
        length: 8u8,
        read_address: 0x60008860u32,
        write_address: 0x60008800u32,
    },
];

/// Defined eFuse registers and commands
pub(crate) mod defines {
    use super::super::EfuseBlockErrors;
    pub(crate) const BLOCK_ERRORS: &[EfuseBlockErrors] = &[
        EfuseBlockErrors {
            err_num_reg: 0x60008880u32,
            err_num_mask: None,
            err_num_offset: None,
            fail_bit_reg: 0x60008880u32,
            fail_bit_offset: None,
        },
        EfuseBlockErrors {
            err_num_reg: 0x60008884u32,
            err_num_mask: Some(0x7u32),
            err_num_offset: Some(0x0u32),
            fail_bit_reg: 0x60008884u32,
            fail_bit_offset: Some(0x3u32),
        },
        EfuseBlockErrors {
            err_num_reg: 0x60008884u32,
            err_num_mask: Some(0x7u32),
            err_num_offset: Some(0x4u32),
            fail_bit_reg: 0x60008884u32,
            fail_bit_offset: Some(0x7u32),
        },
        EfuseBlockErrors {
            err_num_reg: 0x60008884u32,
            err_num_mask: Some(0x7u32),
            err_num_offset: Some(0x8u32),
            fail_bit_reg: 0x60008884u32,
            fail_bit_offset: Some(0xbu32),
        },
    ];
    pub(crate) const EFUSE_PGM_CMD_MASK: u32 = 0x3;
    pub(crate) const EFUSE_WRITE_OP_CODE: u32 = 0x5a5a;
    pub(crate) const EFUSE_READ_CMD: u32 = 0x1;
    pub(crate) const EFUSE_DAC_NUM_M: u32 = 0x1fe00;
    pub(crate) const EFUSE_DAC_CONF_REG: u32 = 0x60008908;
    pub(crate) const EFUSE_MEM_SIZE: u32 = 0x200;
    pub(crate) const EFUSE_WR_TIM_CONF2_REG: u32 = 0x60008918;
    pub(crate) const CODING_SCHEME_NONE_RECOVERY: u32 = 0x3;
    pub(crate) const EFUSE_PWR_OFF_NUM_M: u32 = 0xffff;
    pub(crate) const CODING_SCHEME_RS: u32 = 0x4;
    pub(crate) const EFUSE_PGM_DATA0_REG: u32 = 0x60008800;
    pub(crate) const EFUSE_PWR_OFF_NUM_S: u32 = 0x0;
    pub(crate) const EFUSE_TPGM_INACTIVE_S: u32 = 0x8;
    pub(crate) const EFUSE_DAC_CLK_DIV_S: u32 = 0x0;
    pub(crate) const EFUSE_CLK_REG: u32 = 0x60008888;
    pub(crate) const EFUSE_PWR_ON_NUM_M: u32 = 0xffff00;
    pub(crate) const EFUSE_TPGM_INACTIVE_M: u32 = 0xff00;
    pub(crate) const EFUSE_PGM_CHECK_VALUE0_REG: u32 = 0x60008820;
    pub(crate) const EFUSE_READ_OP_CODE: u32 = 0x5aa5;
    pub(crate) const EFUSE_PWR_ON_NUM_S: u32 = 0x8;
    pub(crate) const EFUSE_RD_RS_ERR_REG: u32 = 0x60008884;
    pub(crate) const CODING_SCHEME_REPEAT: u32 = 0x2;
    pub(crate) const EFUSE_STATUS_REG: u32 = 0x60008890;
    pub(crate) const EFUSE_CONF_REG: u32 = 0x6000888c;
    pub(crate) const EFUSE_WR_TIM_CONF0_REG: u32 = 0x60008910;
    pub(crate) const EFUSE_DAC_NUM_S: u32 = 0x9;
    pub(crate) const CODING_SCHEME_34: u32 = 0x1;
    pub(crate) const EFUSE_DAC_CLK_DIV_M: u32 = 0xff;
    pub(crate) const EFUSE_PGM_CMD: u32 = 0x2;
    pub(crate) const EFUSE_WR_TIM_CONF1_REG: u32 = 0x60008914;
    pub(crate) const CODING_SCHEME_NONE: u32 = 0x0;
    pub(crate) const EFUSE_RD_REPEAT_ERR_REG: u32 = 0x60008880;
    pub(crate) const EFUSE_CMD_REG: u32 = 0x60008894;
}

/// Disable programming of individual eFuses
pub const WR_DIS: EfuseField = EfuseField::new(0, 0, 0, 8);
///
pub const RESERVED_0_8: EfuseField = EfuseField::new(0, 0, 8, 24);
/// Disable reading from BlOCK3
pub const RD_DIS: EfuseField = EfuseField::new(0, 1, 32, 2);
/// RTC watchdog timeout threshold; in unit of slow clock cycle
pub const WDT_DELAY_SEL: EfuseField = EfuseField::new(0, 1, 34, 2);
/// Set this bit to disable pad jtag
pub const DIS_PAD_JTAG: EfuseField = EfuseField::new(0, 1, 36, 1);
/// The bit be set to disable icache in download mode
pub const DIS_DOWNLOAD_ICACHE: EfuseField = EfuseField::new(0, 1, 37, 1);
/// The bit be set to disable manual encryption
pub const DIS_DOWNLOAD_MANUAL_ENCRYPT: EfuseField = EfuseField::new(0, 1, 38, 1);
/// Enables flash encryption when 1 or 3 bits are set and disables otherwise
pub const SPI_BOOT_CRYPT_CNT: EfuseField = EfuseField::new(0, 1, 39, 3);
/// Flash encryption key length
pub const XTS_KEY_LENGTH_256: EfuseField = EfuseField::new(0, 1, 42, 1);
/// Set the default UARTboot message output mode
pub const UART_PRINT_CONTROL: EfuseField = EfuseField::new(0, 1, 43, 2);
/// Set this bit to force ROM code to send a resume command during SPI boot
pub const FORCE_SEND_RESUME: EfuseField = EfuseField::new(0, 1, 45, 1);
/// Set this bit to disable download mode (boot_mode\[3:0\] = 0; 1; 2; 4; 5; 6;
/// 7)
pub const DIS_DOWNLOAD_MODE: EfuseField = EfuseField::new(0, 1, 46, 1);
/// This bit set means disable direct_boot mode
pub const DIS_DIRECT_BOOT: EfuseField = EfuseField::new(0, 1, 47, 1);
/// Set this bit to enable secure UART download mode
pub const ENABLE_SECURITY_DOWNLOAD: EfuseField = EfuseField::new(0, 1, 48, 1);
/// Configures flash waiting time after power-up; in unit of ms. If the value is
/// less than 15; the waiting time is the configurable value.  Otherwise; the
/// waiting time is twice the configurable value
pub const FLASH_TPUW: EfuseField = EfuseField::new(0, 1, 49, 4);
/// The bit be set to enable secure boot
pub const SECURE_BOOT_EN: EfuseField = EfuseField::new(0, 1, 53, 1);
/// Secure version for anti-rollback
pub const SECURE_VERSION: EfuseField = EfuseField::new(0, 1, 54, 4);
/// True if MAC_CUSTOM is burned
pub const CUSTOM_MAC_USED: EfuseField = EfuseField::new(0, 1, 58, 1);
/// Disables check of wafer version major
pub const DISABLE_WAFER_VERSION_MAJOR: EfuseField = EfuseField::new(0, 1, 59, 1);
/// Disables check of blk version major
pub const DISABLE_BLK_VERSION_MAJOR: EfuseField = EfuseField::new(0, 1, 60, 1);
/// reserved
pub const RESERVED_0_61: EfuseField = EfuseField::new(0, 1, 61, 3);
/// Custom MAC address
pub const CUSTOM_MAC: EfuseField = EfuseField::new(1, 0, 0, 48);
/// reserved
pub const RESERVED_1_48: EfuseField = EfuseField::new(1, 1, 48, 16);
/// Stores the bits \[64:87\] of system data
pub const SYSTEM_DATA2: EfuseField = EfuseField::new(1, 2, 64, 24);
/// MAC address
pub const MAC0: EfuseField = EfuseField::new(2, 0, 0, 32);
/// MAC address
pub const MAC1: EfuseField = EfuseField::new(2, 1, 32, 16);
/// WAFER_VERSION_MINOR
pub const WAFER_VERSION_MINOR: EfuseField = EfuseField::new(2, 1, 48, 4);
/// WAFER_VERSION_MAJOR
pub const WAFER_VERSION_MAJOR: EfuseField = EfuseField::new(2, 1, 52, 2);
/// EFUSE_PKG_VERSION
pub const PKG_VERSION: EfuseField = EfuseField::new(2, 1, 54, 3);
/// Minor version of BLOCK2
pub const BLK_VERSION_MINOR: EfuseField = EfuseField::new(2, 1, 57, 3);
/// Major version of BLOCK2
pub const BLK_VERSION_MAJOR: EfuseField = EfuseField::new(2, 1, 60, 2);
/// OCode
pub const OCODE: EfuseField = EfuseField::new(2, 1, 62, 7);
/// Temperature calibration data
pub const TEMP_CALIB: EfuseField = EfuseField::new(2, 2, 69, 9);
/// ADC1 init code at atten0
pub const ADC1_INIT_CODE_ATTEN0: EfuseField = EfuseField::new(2, 2, 78, 8);
/// ADC1 init code at atten3
pub const ADC1_INIT_CODE_ATTEN3: EfuseField = EfuseField::new(2, 2, 86, 5);
/// ADC1 calibration voltage at atten0
pub const ADC1_CAL_VOL_ATTEN0: EfuseField = EfuseField::new(2, 2, 91, 8);
/// ADC1 calibration voltage at atten3
pub const ADC1_CAL_VOL_ATTEN3: EfuseField = EfuseField::new(2, 3, 99, 6);
/// BLOCK2 digital dbias when hvt
pub const DIG_DBIAS_HVT: EfuseField = EfuseField::new(2, 3, 105, 5);
/// BLOCK2 DIG_LDO_DBG0_DBIAS2
pub const DIG_LDO_SLP_DBIAS2: EfuseField = EfuseField::new(2, 3, 110, 7);
/// BLOCK2 DIG_LDO_DBG0_DBIAS26
pub const DIG_LDO_SLP_DBIAS26: EfuseField = EfuseField::new(2, 3, 117, 8);
/// BLOCK2 DIG_LDO_ACT_DBIAS26
pub const DIG_LDO_ACT_DBIAS26: EfuseField = EfuseField::new(2, 3, 125, 6);
/// BLOCK2 DIG_LDO_ACT_STEPD10
pub const DIG_LDO_ACT_STEPD10: EfuseField = EfuseField::new(2, 4, 131, 4);
/// BLOCK2 DIG_LDO_SLP_DBIAS13
pub const RTC_LDO_SLP_DBIAS13: EfuseField = EfuseField::new(2, 4, 135, 7);
/// BLOCK2 DIG_LDO_SLP_DBIAS29
pub const RTC_LDO_SLP_DBIAS29: EfuseField = EfuseField::new(2, 4, 142, 9);
/// BLOCK2 DIG_LDO_SLP_DBIAS31
pub const RTC_LDO_SLP_DBIAS31: EfuseField = EfuseField::new(2, 4, 151, 6);
/// BLOCK2 DIG_LDO_ACT_DBIAS31
pub const RTC_LDO_ACT_DBIAS31: EfuseField = EfuseField::new(2, 4, 157, 6);
/// BLOCK2 DIG_LDO_ACT_DBIAS13
pub const RTC_LDO_ACT_DBIAS13: EfuseField = EfuseField::new(2, 5, 163, 8);
/// reserved
pub const RESERVED_2_171: EfuseField = EfuseField::new(2, 5, 171, 21);
/// Store the bit \[86:96\] of ADC calibration data
pub const ADC_CALIBRATION_3: EfuseField = EfuseField::new(2, 6, 192, 11);
/// Store the bit \[0:20\] of block2 reserved data
pub const BLK2_RESERVED_DATA_0: EfuseField = EfuseField::new(2, 6, 203, 21);
/// Store the bit \[21:52\] of block2 reserved data
pub const BLK2_RESERVED_DATA_1: EfuseField = EfuseField::new(2, 7, 224, 32);
/// BLOCK_KEY0 - 256-bits. 256-bit key of Flash Encryption
pub const BLOCK_KEY0: EfuseField = EfuseField::new(3, 0, 0, 256);
