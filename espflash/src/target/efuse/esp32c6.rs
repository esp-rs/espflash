//! eFuse field definitions for the esp32c6
//!
//! This file was automatically generated, please do not edit it manually!
//!
//! Generated: 2025-12-08 14:48
//! Version:   df46b69f0ed3913114ba53d3a0b2b843

#![allow(unused)]

use super::{EfuseBlock, EfuseField};

/// All eFuse blocks available on this device.
pub(crate) const BLOCKS: &[EfuseBlock] = &[
    EfuseBlock {
        index: 0u8,
        length: 6u8,
        read_address: 0x600b082cu32,
        write_address: 0x600b0800u32,
    },
    EfuseBlock {
        index: 1u8,
        length: 6u8,
        read_address: 0x600b0844u32,
        write_address: 0x600b0800u32,
    },
    EfuseBlock {
        index: 2u8,
        length: 8u8,
        read_address: 0x600b085cu32,
        write_address: 0x600b0800u32,
    },
    EfuseBlock {
        index: 3u8,
        length: 8u8,
        read_address: 0x600b087cu32,
        write_address: 0x600b0800u32,
    },
    EfuseBlock {
        index: 4u8,
        length: 8u8,
        read_address: 0x600b089cu32,
        write_address: 0x600b0800u32,
    },
    EfuseBlock {
        index: 5u8,
        length: 8u8,
        read_address: 0x600b08bcu32,
        write_address: 0x600b0800u32,
    },
    EfuseBlock {
        index: 6u8,
        length: 8u8,
        read_address: 0x600b08dcu32,
        write_address: 0x600b0800u32,
    },
    EfuseBlock {
        index: 7u8,
        length: 8u8,
        read_address: 0x600b08fcu32,
        write_address: 0x600b0800u32,
    },
    EfuseBlock {
        index: 8u8,
        length: 8u8,
        read_address: 0x600b091cu32,
        write_address: 0x600b0800u32,
    },
    EfuseBlock {
        index: 9u8,
        length: 8u8,
        read_address: 0x600b093cu32,
        write_address: 0x600b0800u32,
    },
    EfuseBlock {
        index: 10u8,
        length: 8u8,
        read_address: 0x600b095cu32,
        write_address: 0x600b0800u32,
    },
];

/// Defined eFuse registers and commands
pub(crate) mod defines {
    use super::super::EfuseBlockErrors;
    pub(crate) const BLOCK_ERRORS: &[EfuseBlockErrors] = &[
        EfuseBlockErrors {
            err_num_reg: 0x600b097cu32,
            err_num_mask: None,
            err_num_offset: None,
            fail_bit_reg: 0x600b097cu32,
            fail_bit_offset: None,
        },
        EfuseBlockErrors {
            err_num_reg: 0x600b09c0u32,
            err_num_mask: Some(0x7u32),
            err_num_offset: Some(0x0u32),
            fail_bit_reg: 0x600b09c0u32,
            fail_bit_offset: Some(0x3u32),
        },
        EfuseBlockErrors {
            err_num_reg: 0x600b09c0u32,
            err_num_mask: Some(0x7u32),
            err_num_offset: Some(0x4u32),
            fail_bit_reg: 0x600b09c0u32,
            fail_bit_offset: Some(0x7u32),
        },
        EfuseBlockErrors {
            err_num_reg: 0x600b09c0u32,
            err_num_mask: Some(0x7u32),
            err_num_offset: Some(0x8u32),
            fail_bit_reg: 0x600b09c0u32,
            fail_bit_offset: Some(0xbu32),
        },
        EfuseBlockErrors {
            err_num_reg: 0x600b09c0u32,
            err_num_mask: Some(0x7u32),
            err_num_offset: Some(0xcu32),
            fail_bit_reg: 0x600b09c0u32,
            fail_bit_offset: Some(0xfu32),
        },
        EfuseBlockErrors {
            err_num_reg: 0x600b09c0u32,
            err_num_mask: Some(0x7u32),
            err_num_offset: Some(0x10u32),
            fail_bit_reg: 0x600b09c0u32,
            fail_bit_offset: Some(0x13u32),
        },
        EfuseBlockErrors {
            err_num_reg: 0x600b09c0u32,
            err_num_mask: Some(0x7u32),
            err_num_offset: Some(0x14u32),
            fail_bit_reg: 0x600b09c0u32,
            fail_bit_offset: Some(0x17u32),
        },
        EfuseBlockErrors {
            err_num_reg: 0x600b09c0u32,
            err_num_mask: Some(0x7u32),
            err_num_offset: Some(0x18u32),
            fail_bit_reg: 0x600b09c0u32,
            fail_bit_offset: Some(0x1bu32),
        },
        EfuseBlockErrors {
            err_num_reg: 0x600b09c0u32,
            err_num_mask: Some(0x7u32),
            err_num_offset: Some(0x1cu32),
            fail_bit_reg: 0x600b09c0u32,
            fail_bit_offset: Some(0x1fu32),
        },
        EfuseBlockErrors {
            err_num_reg: 0x600b09c4u32,
            err_num_mask: Some(0x7u32),
            err_num_offset: Some(0x0u32),
            fail_bit_reg: 0x600b09c4u32,
            fail_bit_offset: Some(0x3u32),
        },
        EfuseBlockErrors {
            err_num_reg: 0x600b09c4u32,
            err_num_mask: Some(0x7u32),
            err_num_offset: Some(0x4u32),
            fail_bit_reg: 0x600b09c4u32,
            fail_bit_offset: Some(0x7u32),
        },
    ];
    pub(crate) const EFUSE_WR_TIM_CONF1_REG: u32 = 0x600b09f0;
    pub(crate) const EFUSE_WR_TIM_CONF2_REG: u32 = 0x600b09f4;
    pub(crate) const EFUSE_RD_REPEAT_ERR0_REG: u32 = 0x600b097c;
    pub(crate) const EFUSE_DAC_CLK_DIV_S: u32 = 0x0;
    pub(crate) const EFUSE_STATUS_REG: u32 = 0x600b09d0;
    pub(crate) const EFUSE_WRITE_OP_CODE: u32 = 0x5a5a;
    pub(crate) const CODING_SCHEME_NONE: u32 = 0x0;
    pub(crate) const EFUSE_PWR_OFF_NUM_S: u32 = 0x0;
    pub(crate) const EFUSE_PWR_ON_NUM_S: u32 = 0x8;
    pub(crate) const EFUSE_RD_REPEAT_ERR1_REG: u32 = 0x600b0980;
    pub(crate) const CODING_SCHEME_NONE_RECOVERY: u32 = 0x3;
    pub(crate) const EFUSE_RD_RS_ERR0_REG: u32 = 0x600b09c0;
    pub(crate) const EFUSE_READ_CMD: u32 = 0x1;
    pub(crate) const EFUSE_RD_REPEAT_ERR3_REG: u32 = 0x600b0988;
    pub(crate) const EFUSE_CLK_REG: u32 = 0x600b09c8;
    pub(crate) const EFUSE_CMD_REG: u32 = 0x600b09d4;
    pub(crate) const EFUSE_RD_REPEAT_ERR2_REG: u32 = 0x600b0984;
    pub(crate) const EFUSE_DAC_NUM_M: u32 = 0x1fe00;
    pub(crate) const CODING_SCHEME_34: u32 = 0x1;
    pub(crate) const EFUSE_DATE_REG: u32 = 0x600b09fc;
    pub(crate) const EFUSE_RD_RS_ERR1_REG: u32 = 0x600b09c4;
    pub(crate) const CODING_SCHEME_RS: u32 = 0x4;
    pub(crate) const EFUSE_CONF_REG: u32 = 0x600b09cc;
    pub(crate) const EFUSE_READ_OP_CODE: u32 = 0x5aa5;
    pub(crate) const EFUSE_PGM_CMD_MASK: u32 = 0x3;
    pub(crate) const EFUSE_PGM_CMD: u32 = 0x2;
    pub(crate) const EFUSE_DAC_CONF_REG: u32 = 0x600b09e8;
    pub(crate) const EFUSE_RD_TIM_CONF_REG: u32 = 0x600b09ec;
    pub(crate) const CODING_SCHEME_REPEAT: u32 = 0x2;
    pub(crate) const EFUSE_DAC_CLK_DIV_M: u32 = 0xff;
    pub(crate) const EFUSE_CHECK_VALUE0_REG: u32 = 0x600b0820;
    pub(crate) const EFUSE_MEM_SIZE: u32 = 0x200;
    pub(crate) const EFUSE_RD_REPEAT_ERR4_REG: u32 = 0x600b098c;
    pub(crate) const EFUSE_PWR_ON_NUM_M: u32 = 0xffff00;
    pub(crate) const EFUSE_PGM_DATA0_REG: u32 = 0x600b0800;
    pub(crate) const EFUSE_PWR_OFF_NUM_M: u32 = 0xffff;
    pub(crate) const EFUSE_DAC_NUM_S: u32 = 0x9;
}

/// Disable programming of individual eFuses
pub const WR_DIS: EfuseField = EfuseField::new(0, 0, 0, 32);
/// Disable reading from BlOCK4-10
pub const RD_DIS: EfuseField = EfuseField::new(0, 1, 32, 7);
/// Represents whether pad of uart and sdio is swapped or not. 1: swapped. 0:
/// not swapped
pub const SWAP_UART_SDIO_EN: EfuseField = EfuseField::new(0, 1, 39, 1);
/// Represents whether icache is disabled or enabled. 1: disabled. 0: enabled
pub const DIS_ICACHE: EfuseField = EfuseField::new(0, 1, 40, 1);
/// Represents whether the function of usb switch to jtag is disabled or
/// enabled. 1: disabled. 0: enabled
pub const DIS_USB_JTAG: EfuseField = EfuseField::new(0, 1, 41, 1);
/// Represents whether icache is disabled or enabled in Download mode. 1:
/// disabled. 0: enabled
pub const DIS_DOWNLOAD_ICACHE: EfuseField = EfuseField::new(0, 1, 42, 1);
/// Represents whether USB-Serial-JTAG is disabled or enabled. 1: disabled. 0:
/// enabled
pub const DIS_USB_SERIAL_JTAG: EfuseField = EfuseField::new(0, 1, 43, 1);
/// Represents whether the function that forces chip into download mode is
/// disabled or enabled. 1: disabled. 0: enabled
pub const DIS_FORCE_DOWNLOAD: EfuseField = EfuseField::new(0, 1, 44, 1);
/// Represents whether SPI0 controller during boot_mode_download is disabled or
/// enabled. 1: disabled. 0: enabled
pub const SPI_DOWNLOAD_MSPI_DIS: EfuseField = EfuseField::new(0, 1, 45, 1);
/// Represents whether TWAI function is disabled or enabled. 1: disabled. 0:
/// enabled
pub const DIS_TWAI: EfuseField = EfuseField::new(0, 1, 46, 1);
/// Represents whether the selection between usb_to_jtag and pad_to_jtag through
/// strapping gpio15 when both EFUSE_DIS_PAD_JTAG and EFUSE_DIS_USB_JTAG are
/// equal to 0 is enabled or disabled. 1: enabled. 0: disabled
pub const JTAG_SEL_ENABLE: EfuseField = EfuseField::new(0, 1, 47, 1);
/// Represents whether JTAG is disabled in soft way. Odd number: disabled. Even
/// number: enabled
pub const SOFT_DIS_JTAG: EfuseField = EfuseField::new(0, 1, 48, 3);
/// Represents whether JTAG is disabled in the hard way(permanently). 1:
/// disabled. 0: enabled
pub const DIS_PAD_JTAG: EfuseField = EfuseField::new(0, 1, 51, 1);
/// Represents whether flash encrypt function is disabled or enabled(except in
/// SPI boot mode). 1: disabled. 0: enabled
pub const DIS_DOWNLOAD_MANUAL_ENCRYPT: EfuseField = EfuseField::new(0, 1, 52, 1);
/// Represents the single-end input threshold vrefh; 1.76 V to 2 V with step of
/// 80 mV
pub const USB_DREFH: EfuseField = EfuseField::new(0, 1, 53, 2);
/// Represents the single-end input threshold vrefl; 1.76 V to 2 V with step of
/// 80 mV
pub const USB_DREFL: EfuseField = EfuseField::new(0, 1, 55, 2);
/// Represents whether the D+ and D- pins is exchanged. 1: exchanged. 0: not
/// exchanged
pub const USB_EXCHG_PINS: EfuseField = EfuseField::new(0, 1, 57, 1);
/// Represents whether vdd spi pin is functioned as gpio. 1: functioned. 0: not
/// functioned
pub const VDD_SPI_AS_GPIO: EfuseField = EfuseField::new(0, 1, 58, 1);
/// Reserved
pub const RPT4_RESERVED0_2: EfuseField = EfuseField::new(0, 1, 59, 2);
/// Reserved
pub const RPT4_RESERVED0_1: EfuseField = EfuseField::new(0, 1, 61, 1);
/// Reserved
pub const RPT4_RESERVED0_0: EfuseField = EfuseField::new(0, 1, 62, 2);
/// Reserved
pub const RPT4_RESERVED1_0: EfuseField = EfuseField::new(0, 2, 64, 16);
/// Represents whether RTC watchdog timeout threshold is selected at startup. 1:
/// selected. 0: not selected
pub const WDT_DELAY_SEL: EfuseField = EfuseField::new(0, 2, 80, 2);
/// Enables flash encryption when 1 or 3 bits are set and disables otherwise
pub const SPI_BOOT_CRYPT_CNT: EfuseField = EfuseField::new(0, 2, 82, 3);
/// Revoke 1st secure boot key
pub const SECURE_BOOT_KEY_REVOKE0: EfuseField = EfuseField::new(0, 2, 85, 1);
/// Revoke 2nd secure boot key
pub const SECURE_BOOT_KEY_REVOKE1: EfuseField = EfuseField::new(0, 2, 86, 1);
/// Revoke 3rd secure boot key
pub const SECURE_BOOT_KEY_REVOKE2: EfuseField = EfuseField::new(0, 2, 87, 1);
/// Represents the purpose of Key0
pub const KEY_PURPOSE_0: EfuseField = EfuseField::new(0, 2, 88, 4);
/// Represents the purpose of Key1
pub const KEY_PURPOSE_1: EfuseField = EfuseField::new(0, 2, 92, 4);
/// Represents the purpose of Key2
pub const KEY_PURPOSE_2: EfuseField = EfuseField::new(0, 3, 96, 4);
/// Represents the purpose of Key3
pub const KEY_PURPOSE_3: EfuseField = EfuseField::new(0, 3, 100, 4);
/// Represents the purpose of Key4
pub const KEY_PURPOSE_4: EfuseField = EfuseField::new(0, 3, 104, 4);
/// Represents the purpose of Key5
pub const KEY_PURPOSE_5: EfuseField = EfuseField::new(0, 3, 108, 4);
/// Represents the spa secure level by configuring the clock random divide mode
pub const SEC_DPA_LEVEL: EfuseField = EfuseField::new(0, 3, 112, 2);
/// Represents whether anti-dpa attack is enabled. 1:enabled. 0: disabled
pub const CRYPT_DPA_ENABLE: EfuseField = EfuseField::new(0, 3, 114, 1);
/// Reserved
pub const RPT4_RESERVED2_1: EfuseField = EfuseField::new(0, 3, 115, 1);
/// Represents whether secure boot is enabled or disabled. 1: enabled. 0:
/// disabled
pub const SECURE_BOOT_EN: EfuseField = EfuseField::new(0, 3, 116, 1);
/// Represents whether revoking aggressive secure boot is enabled or disabled.
/// 1: enabled. 0: disabled
pub const SECURE_BOOT_AGGRESSIVE_REVOKE: EfuseField = EfuseField::new(0, 3, 117, 1);
/// Reserved
pub const RPT4_RESERVED2_0: EfuseField = EfuseField::new(0, 3, 118, 6);
/// Represents the flash waiting time after power-up; in unit of ms. When the
/// value less than 15; the waiting time is the programmed value. Otherwise; the
/// waiting time is 2 times the programmed value
pub const FLASH_TPUW: EfuseField = EfuseField::new(0, 3, 124, 4);
/// Represents whether Download mode is disabled or enabled. 1: disabled. 0:
/// enabled
pub const DIS_DOWNLOAD_MODE: EfuseField = EfuseField::new(0, 4, 128, 1);
/// Represents whether direct boot mode is disabled or enabled. 1: disabled. 0:
/// enabled
pub const DIS_DIRECT_BOOT: EfuseField = EfuseField::new(0, 4, 129, 1);
/// Represents whether print from USB-Serial-JTAG is disabled or enabled. 1:
/// disabled. 0: enabled
pub const DIS_USB_SERIAL_JTAG_ROM_PRINT: EfuseField = EfuseField::new(0, 4, 130, 1);
/// Reserved
pub const RPT4_RESERVED3_5: EfuseField = EfuseField::new(0, 4, 131, 1);
/// Represents whether the USB-Serial-JTAG download function is disabled or
/// enabled. 1: disabled. 0: enabled
pub const DIS_USB_SERIAL_JTAG_DOWNLOAD_MODE: EfuseField = EfuseField::new(0, 4, 132, 1);
/// Represents whether security download is enabled or disabled. 1: enabled. 0:
/// disabled
pub const ENABLE_SECURITY_DOWNLOAD: EfuseField = EfuseField::new(0, 4, 133, 1);
/// Set the default UARTboot message output mode
pub const UART_PRINT_CONTROL: EfuseField = EfuseField::new(0, 4, 134, 2);
/// Reserved
pub const RPT4_RESERVED3_4: EfuseField = EfuseField::new(0, 4, 136, 1);
/// Reserved
pub const RPT4_RESERVED3_3: EfuseField = EfuseField::new(0, 4, 137, 1);
/// Reserved
pub const RPT4_RESERVED3_2: EfuseField = EfuseField::new(0, 4, 138, 2);
/// Reserved
pub const RPT4_RESERVED3_1: EfuseField = EfuseField::new(0, 4, 140, 1);
/// Represents whether ROM code is forced to send a resume command during SPI
/// boot. 1: forced. 0:not forced
pub const FORCE_SEND_RESUME: EfuseField = EfuseField::new(0, 4, 141, 1);
/// Represents the version used by ESP-IDF anti-rollback feature
pub const SECURE_VERSION: EfuseField = EfuseField::new(0, 4, 142, 16);
/// Represents whether FAST VERIFY ON WAKE is disabled or enabled when Secure
/// Boot is enabled. 1: disabled. 0: enabled
pub const SECURE_BOOT_DISABLE_FAST_WAKE: EfuseField = EfuseField::new(0, 4, 158, 1);
/// Reserved
pub const RPT4_RESERVED3_0: EfuseField = EfuseField::new(0, 4, 159, 1);
/// Disables check of wafer version major
pub const DISABLE_WAFER_VERSION_MAJOR: EfuseField = EfuseField::new(0, 5, 160, 1);
/// Disables check of blk version major
pub const DISABLE_BLK_VERSION_MAJOR: EfuseField = EfuseField::new(0, 5, 161, 1);
/// reserved
pub const RESERVED_0_162: EfuseField = EfuseField::new(0, 5, 162, 22);
/// Reserved
pub const RPT4_RESERVED4_0: EfuseField = EfuseField::new(0, 5, 184, 8);
/// MAC address
pub const MAC0: EfuseField = EfuseField::new(1, 0, 0, 32);
/// MAC address
pub const MAC1: EfuseField = EfuseField::new(1, 1, 32, 16);
/// Stores the extended bits of MAC address
pub const MAC_EXT: EfuseField = EfuseField::new(1, 1, 48, 16);
/// Stores the active hp dbias
pub const ACTIVE_HP_DBIAS: EfuseField = EfuseField::new(1, 2, 64, 5);
/// Stores the active lp dbias
pub const ACTIVE_LP_DBIAS: EfuseField = EfuseField::new(1, 2, 69, 5);
/// Stores the lslp hp dbg
pub const LSLP_HP_DBG: EfuseField = EfuseField::new(1, 2, 74, 2);
/// Stores the lslp hp dbias
pub const LSLP_HP_DBIAS: EfuseField = EfuseField::new(1, 2, 76, 4);
/// Stores the dslp lp dbg
pub const DSLP_LP_DBG: EfuseField = EfuseField::new(1, 2, 80, 3);
/// Stores the dslp lp dbias
pub const DSLP_LP_DBIAS: EfuseField = EfuseField::new(1, 2, 83, 4);
/// Stores the hp and lp dbias vol gap
pub const DBIAS_VOL_GAP: EfuseField = EfuseField::new(1, 2, 87, 5);
/// Stores the first part of SPI_PAD_CONF
pub const SPI_PAD_CONF_1: EfuseField = EfuseField::new(1, 2, 92, 4);
/// Stores the second part of SPI_PAD_CONF
pub const SPI_PAD_CONF_2: EfuseField = EfuseField::new(1, 3, 96, 18);
///
pub const WAFER_VERSION_MINOR: EfuseField = EfuseField::new(1, 3, 114, 4);
///
pub const WAFER_VERSION_MAJOR: EfuseField = EfuseField::new(1, 3, 118, 2);
/// Package version
pub const PKG_VERSION: EfuseField = EfuseField::new(1, 3, 120, 3);
/// BLK_VERSION_MINOR of BLOCK2
pub const BLK_VERSION_MINOR: EfuseField = EfuseField::new(1, 3, 123, 3);
/// BLK_VERSION_MAJOR of BLOCK2
pub const BLK_VERSION_MAJOR: EfuseField = EfuseField::new(1, 3, 126, 2);
///
pub const FLASH_CAP: EfuseField = EfuseField::new(1, 4, 128, 3);
///
pub const FLASH_TEMP: EfuseField = EfuseField::new(1, 4, 131, 2);
///
pub const FLASH_VENDOR: EfuseField = EfuseField::new(1, 4, 133, 3);
/// reserved
pub const RESERVED_1_136: EfuseField = EfuseField::new(1, 4, 136, 24);
/// Stores the second 32 bits of the zeroth part of system data
pub const SYS_DATA_PART0_2: EfuseField = EfuseField::new(1, 5, 160, 32);
/// Optional unique 128-bit ID
pub const OPTIONAL_UNIQUE_ID: EfuseField = EfuseField::new(2, 0, 0, 128);
/// Temperature calibration data
pub const TEMP_CALIB: EfuseField = EfuseField::new(2, 4, 128, 9);
/// ADC OCode
pub const OCODE: EfuseField = EfuseField::new(2, 4, 137, 8);
/// ADC1 init code at atten0
pub const ADC1_INIT_CODE_ATTEN0: EfuseField = EfuseField::new(2, 4, 145, 10);
/// ADC1 init code at atten1
pub const ADC1_INIT_CODE_ATTEN1: EfuseField = EfuseField::new(2, 4, 155, 10);
/// ADC1 init code at atten2
pub const ADC1_INIT_CODE_ATTEN2: EfuseField = EfuseField::new(2, 5, 165, 10);
/// ADC1 init code at atten3
pub const ADC1_INIT_CODE_ATTEN3: EfuseField = EfuseField::new(2, 5, 175, 10);
/// ADC1 calibration voltage at atten0
pub const ADC1_CAL_VOL_ATTEN0: EfuseField = EfuseField::new(2, 5, 185, 10);
/// ADC1 calibration voltage at atten1
pub const ADC1_CAL_VOL_ATTEN1: EfuseField = EfuseField::new(2, 6, 195, 10);
/// ADC1 calibration voltage at atten2
pub const ADC1_CAL_VOL_ATTEN2: EfuseField = EfuseField::new(2, 6, 205, 10);
/// ADC1 calibration voltage at atten3
pub const ADC1_CAL_VOL_ATTEN3: EfuseField = EfuseField::new(2, 6, 215, 10);
/// ADC1 init code at atten0 ch0
pub const ADC1_INIT_CODE_ATTEN0_CH0: EfuseField = EfuseField::new(2, 7, 225, 4);
/// ADC1 init code at atten0 ch1
pub const ADC1_INIT_CODE_ATTEN0_CH1: EfuseField = EfuseField::new(2, 7, 229, 4);
/// ADC1 init code at atten0 ch2
pub const ADC1_INIT_CODE_ATTEN0_CH2: EfuseField = EfuseField::new(2, 7, 233, 4);
/// ADC1 init code at atten0 ch3
pub const ADC1_INIT_CODE_ATTEN0_CH3: EfuseField = EfuseField::new(2, 7, 237, 4);
/// ADC1 init code at atten0 ch4
pub const ADC1_INIT_CODE_ATTEN0_CH4: EfuseField = EfuseField::new(2, 7, 241, 4);
/// ADC1 init code at atten0 ch5
pub const ADC1_INIT_CODE_ATTEN0_CH5: EfuseField = EfuseField::new(2, 7, 245, 4);
/// ADC1 init code at atten0 ch6
pub const ADC1_INIT_CODE_ATTEN0_CH6: EfuseField = EfuseField::new(2, 7, 249, 4);
/// reserved
pub const RESERVED_2_253: EfuseField = EfuseField::new(2, 7, 253, 3);
/// User data
pub const BLOCK_USR_DATA: EfuseField = EfuseField::new(3, 0, 0, 192);
/// reserved
pub const RESERVED_3_192: EfuseField = EfuseField::new(3, 6, 192, 8);
/// Custom MAC
pub const CUSTOM_MAC: EfuseField = EfuseField::new(3, 6, 200, 48);
/// reserved
pub const RESERVED_3_248: EfuseField = EfuseField::new(3, 7, 248, 8);
/// Key0 or user data
pub const BLOCK_KEY0: EfuseField = EfuseField::new(4, 0, 0, 256);
/// Key1 or user data
pub const BLOCK_KEY1: EfuseField = EfuseField::new(5, 0, 0, 256);
/// Key2 or user data
pub const BLOCK_KEY2: EfuseField = EfuseField::new(6, 0, 0, 256);
/// Key3 or user data
pub const BLOCK_KEY3: EfuseField = EfuseField::new(7, 0, 0, 256);
/// Key4 or user data
pub const BLOCK_KEY4: EfuseField = EfuseField::new(8, 0, 0, 256);
/// Key5 or user data
pub const BLOCK_KEY5: EfuseField = EfuseField::new(9, 0, 0, 256);
/// System data part 2 (reserved)
pub const BLOCK_SYS_DATA2: EfuseField = EfuseField::new(10, 0, 0, 256);
