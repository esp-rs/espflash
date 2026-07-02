//! eFuse field definitions for the esp32s31
//!
//! This file was automatically generated, please do not edit it manually!
//!
//! For source, see espefuse/efuse_defs/esp32s31.yaml in the esptool repository.

#![allow(unused)]

use super::{EfuseBlock, EfuseField};

/// All eFuse blocks available on this device.
pub(crate) const BLOCKS: &[EfuseBlock] = &[
    EfuseBlock {
        index: 0u8,
        length: 9u8,
        read_address: 0x2071502cu32,
        write_address: 0x20715000u32,
    },
    EfuseBlock {
        index: 1u8,
        length: 6u8,
        read_address: 0x20715050u32,
        write_address: 0x20715000u32,
    },
    EfuseBlock {
        index: 2u8,
        length: 8u8,
        read_address: 0x20715068u32,
        write_address: 0x20715000u32,
    },
    EfuseBlock {
        index: 3u8,
        length: 8u8,
        read_address: 0x20715088u32,
        write_address: 0x20715000u32,
    },
    EfuseBlock {
        index: 4u8,
        length: 8u8,
        read_address: 0x207150a8u32,
        write_address: 0x20715000u32,
    },
    EfuseBlock {
        index: 5u8,
        length: 8u8,
        read_address: 0x207150c8u32,
        write_address: 0x20715000u32,
    },
    EfuseBlock {
        index: 6u8,
        length: 8u8,
        read_address: 0x207150e8u32,
        write_address: 0x20715000u32,
    },
    EfuseBlock {
        index: 7u8,
        length: 8u8,
        read_address: 0x20715108u32,
        write_address: 0x20715000u32,
    },
    EfuseBlock {
        index: 8u8,
        length: 8u8,
        read_address: 0x20715128u32,
        write_address: 0x20715000u32,
    },
    EfuseBlock {
        index: 9u8,
        length: 8u8,
        read_address: 0x20715148u32,
        write_address: 0x20715000u32,
    },
];

/// Defined eFuse registers and commands
pub(crate) mod defines {
    use super::super::EfuseBlockErrors;

    pub(crate) const BLOCK_ERRORS: &[EfuseBlockErrors] = &[
        // BLOCK0
        EfuseBlockErrors {
            err_num_reg: 0x20715168u32,
            err_num_mask: None,
            err_num_offset: None,
            fail_bit_reg: 0x20715168u32,
            fail_bit_offset: None,
        },
        // BLOCK1 (MAC_SPI_8M_0)
        EfuseBlockErrors {
            err_num_reg: 0x20715188u32,
            err_num_mask: Some(0x7u32),
            err_num_offset: Some(0x0u32),
            fail_bit_reg: 0x20715188u32,
            fail_bit_offset: Some(0x3u32),
        },
        // BLOCK2 (BLOCK_SYS_DATA)
        EfuseBlockErrors {
            err_num_reg: 0x20715188u32,
            err_num_mask: Some(0x7u32),
            err_num_offset: Some(0x4u32),
            fail_bit_reg: 0x20715188u32,
            fail_bit_offset: Some(0x7u32),
        },
        // BLOCK3 (BLOCK_USR_DATA)
        EfuseBlockErrors {
            err_num_reg: 0x20715188u32,
            err_num_mask: Some(0x7u32),
            err_num_offset: Some(0x8u32),
            fail_bit_reg: 0x20715188u32,
            fail_bit_offset: Some(0xbu32),
        },
        // BLOCK4 (BLOCK_KEY0)
        EfuseBlockErrors {
            err_num_reg: 0x20715188u32,
            err_num_mask: Some(0x7u32),
            err_num_offset: Some(0xcu32),
            fail_bit_reg: 0x20715188u32,
            fail_bit_offset: Some(0xfu32),
        },
        // BLOCK5 (BLOCK_KEY1)
        EfuseBlockErrors {
            err_num_reg: 0x20715188u32,
            err_num_mask: Some(0x7u32),
            err_num_offset: Some(0x10u32),
            fail_bit_reg: 0x20715188u32,
            fail_bit_offset: Some(0x13u32),
        },
        // BLOCK6 (BLOCK_KEY2)
        EfuseBlockErrors {
            err_num_reg: 0x20715188u32,
            err_num_mask: Some(0x7u32),
            err_num_offset: Some(0x14u32),
            fail_bit_reg: 0x20715188u32,
            fail_bit_offset: Some(0x17u32),
        },
        // BLOCK7 (BLOCK_KEY3)
        EfuseBlockErrors {
            err_num_reg: 0x20715188u32,
            err_num_mask: Some(0x7u32),
            err_num_offset: Some(0x18u32),
            fail_bit_reg: 0x20715188u32,
            fail_bit_offset: Some(0x1bu32),
        },
        // BLOCK8 (BLOCK_KEY4)
        EfuseBlockErrors {
            err_num_reg: 0x20715188u32,
            err_num_mask: Some(0x7u32),
            err_num_offset: Some(0x1cu32),
            fail_bit_reg: 0x20715188u32,
            fail_bit_offset: Some(0x1fu32),
        },
        // BLOCK9 (BLOCK_SYS_DATA2)
        EfuseBlockErrors {
            err_num_reg: 0x2071518cu32,
            err_num_mask: Some(0x7u32),
            err_num_offset: Some(0x4u32),
            fail_bit_reg: 0x2071518cu32,
            fail_bit_offset: Some(0x7u32),
        },
    ];

    pub(crate) const EFUSE_MEM_SIZE: u32 = 0x0200;
    pub(crate) const EFUSE_PGM_DATA0_REG: u32 = 0x20715000;
    pub(crate) const EFUSE_CHECK_VALUE0_REG: u32 = 0x20715020;
    pub(crate) const EFUSE_CLK_REG: u32 = 0x207151c8;
    pub(crate) const EFUSE_CONF_REG: u32 = 0x207151cc;
    pub(crate) const EFUSE_STATUS_REG: u32 = 0x207151d4;
    pub(crate) const EFUSE_CMD_REG: u32 = 0x207151d8;
    pub(crate) const EFUSE_RD_RS_ERR0_REG: u32 = 0x20715188;
    pub(crate) const EFUSE_RD_RS_ERR1_REG: u32 = 0x2071518c;
    pub(crate) const EFUSE_RD_REPEAT_ERR0_REG: u32 = 0x20715168;
    pub(crate) const EFUSE_RD_REPEAT_ERR1_REG: u32 = 0x2071516c;
    pub(crate) const EFUSE_RD_REPEAT_ERR2_REG: u32 = 0x20715170;
    pub(crate) const EFUSE_RD_REPEAT_ERR3_REG: u32 = 0x20715174;
    pub(crate) const EFUSE_RD_REPEAT_ERR4_REG: u32 = 0x20715178;
    pub(crate) const EFUSE_RD_REPEAT_ERR5_REG: u32 = 0x2071517c;
    pub(crate) const EFUSE_RD_REPEAT_ERR6_REG: u32 = 0x20715180;
    pub(crate) const EFUSE_RD_REPEAT_ERR7_REG: u32 = 0x20715184;
    pub(crate) const EFUSE_DAC_CONF_REG: u32 = 0x207151ec;
    pub(crate) const EFUSE_RD_TIM_CONF_REG: u32 = 0x207151f0;
    pub(crate) const EFUSE_WR_TIM_CONF1_REG: u32 = 0x207151f4;
    pub(crate) const EFUSE_WR_TIM_CONF2_REG: u32 = 0x207151f8;
    pub(crate) const EFUSE_DATE_REG: u32 = 0x20715190;
    pub(crate) const EFUSE_WRITE_OP_CODE: u32 = 0x5a5a;
    pub(crate) const EFUSE_READ_OP_CODE: u32 = 0x5aa5;
    pub(crate) const EFUSE_PGM_CMD_MASK: u32 = 0x3;
    pub(crate) const EFUSE_PGM_CMD: u32 = 0x2;
    pub(crate) const EFUSE_READ_CMD: u32 = 0x1;
    pub(crate) const EFUSE_PWR_OFF_NUM_S: u32 = 0x0;
    pub(crate) const EFUSE_PWR_OFF_NUM_M: u32 = 0xffff;
    pub(crate) const EFUSE_PWR_ON_NUM_S: u32 = 0x8;
    pub(crate) const EFUSE_PWR_ON_NUM_M: u32 = 0xffff00;
    pub(crate) const EFUSE_DAC_CLK_DIV_S: u32 = 0x0;
    pub(crate) const EFUSE_DAC_CLK_DIV_M: u32 = 0xff;
    pub(crate) const EFUSE_DAC_NUM_S: u32 = 0x9;
    pub(crate) const EFUSE_DAC_NUM_M: u32 = 0x1fe00;
    pub(crate) const CODING_SCHEME_NONE: u32 = 0x0;
    pub(crate) const CODING_SCHEME_34: u32 = 0x1;
    pub(crate) const CODING_SCHEME_REPEAT: u32 = 0x2;
    pub(crate) const CODING_SCHEME_NONE_RECOVERY: u32 = 0x3;
    pub(crate) const CODING_SCHEME_RS: u32 = 0x4;
}

/// Disable programming of individual eFuses
pub const WR_DIS: EfuseField = EfuseField::new(0, 0, 0, 32);
/// Disable reading from BLOCK4-9
pub const RD_DIS: EfuseField = EfuseField::new(0, 1, 32, 7);
/// Disable USB JTAG function
pub const DIS_USB_JTAG: EfuseField = EfuseField::new(0, 1, 42, 1);
/// Disable USB-Serial-JTAG
pub const DIS_USB_SERIAL_JTAG: EfuseField = EfuseField::new(0, 1, 43, 1);
/// Disable force download mode
pub const DIS_FORCE_DOWNLOAD: EfuseField = EfuseField::new(0, 1, 44, 1);
/// Disable SPI0 during boot_mode_download
pub const SPI_DOWNLOAD_MSPI_DIS: EfuseField = EfuseField::new(0, 1, 45, 1);
/// Disable TWAI
pub const DIS_TWAI: EfuseField = EfuseField::new(0, 1, 46, 1);
/// Enable JTAG selection via strapping pin
pub const JTAG_SEL_ENABLE: EfuseField = EfuseField::new(0, 1, 47, 1);
/// Soft-disable JTAG
pub const SOFT_DIS_JTAG: EfuseField = EfuseField::new(0, 1, 48, 3);
/// Permanently disable JTAG
pub const DIS_PAD_JTAG: EfuseField = EfuseField::new(0, 1, 51, 1);
/// Disable manual flash encryption
pub const DIS_DOWNLOAD_MANUAL_ENCRYPT: EfuseField = EfuseField::new(0, 1, 52, 1);
/// Disable Wi-Fi 6
pub const DIS_WIFI6: EfuseField = EfuseField::new(0, 1, 54, 1);
/// HUK generation state
pub const HUK_GEN_STATE: EfuseField = EfuseField::new(0, 1, 55, 5);
/// KM random number switch cycle control
pub const KM_RND_SWITCH_CYCLE: EfuseField = EfuseField::new(0, 2, 64, 1);
/// Key purpose for Key0
pub const KEY_PURPOSE_0: EfuseField = EfuseField::new(0, 2, 65, 5);
/// Key purpose for Key1
pub const KEY_PURPOSE_1: EfuseField = EfuseField::new(0, 2, 70, 5);
/// Key purpose for Key2
pub const KEY_PURPOSE_2: EfuseField = EfuseField::new(0, 2, 75, 5);
/// Key purpose for Key3
pub const KEY_PURPOSE_3: EfuseField = EfuseField::new(0, 2, 80, 5);
/// Key purpose for Key4
pub const KEY_PURPOSE_4: EfuseField = EfuseField::new(0, 2, 85, 5);
/// Enable secure boot
pub const SECURE_BOOT_EN: EfuseField = EfuseField::new(0, 3, 96, 1);
/// Enable aggressive revocation for secure boot
pub const SECURE_BOOT_AGGRESSIVE_REVOKE: EfuseField = EfuseField::new(0, 3, 97, 1);
/// Flash power-up waiting time
pub const FLASH_TPUW: EfuseField = EfuseField::new(0, 3, 100, 4);
/// Disable download mode
pub const DIS_DOWNLOAD_MODE: EfuseField = EfuseField::new(0, 3, 104, 1);
/// Disable direct boot mode
pub const DIS_DIRECT_BOOT: EfuseField = EfuseField::new(0, 3, 105, 1);
/// Disable USB print from ROM
pub const DIS_USB_SERIAL_JTAG_ROM_PRINT: EfuseField = EfuseField::new(0, 3, 106, 1);
/// Disable USB download mode
pub const DIS_USB_SERIAL_JTAG_DOWNLOAD_MODE: EfuseField = EfuseField::new(0, 3, 107, 1);
/// Enable security download
pub const ENABLE_SECURITY_DOWNLOAD: EfuseField = EfuseField::new(0, 3, 108, 1);
/// UART print control
pub const UART_PRINT_CONTROL: EfuseField = EfuseField::new(0, 3, 109, 2);
/// Enables flash encryption counter
pub const SPI_BOOT_CRYPT_CNT: EfuseField = EfuseField::new(0, 3, 111, 3);
/// Secure version for anti-rollback
pub const SECURE_VERSION: EfuseField = EfuseField::new(0, 3, 114, 16);
/// Revoke secure boot key 0
pub const SECURE_BOOT_KEY_REVOKE0: EfuseField = EfuseField::new(0, 4, 128, 1);
/// Revoke secure boot key 1
pub const SECURE_BOOT_KEY_REVOKE1: EfuseField = EfuseField::new(0, 4, 129, 1);
/// Revoke secure boot key 2
pub const SECURE_BOOT_KEY_REVOKE2: EfuseField = EfuseField::new(0, 4, 130, 1);
/// MAC address (bits 0–31)
pub const MAC0: EfuseField = EfuseField::new(1, 0, 0, 32);
/// MAC address (bits 32–47)
pub const MAC1: EfuseField = EfuseField::new(1, 1, 32, 16);
/// Minor chip version
pub const WAFER_VERSION_MINOR: EfuseField = EfuseField::new(1, 3, 114, 4);
/// Major chip version
pub const WAFER_VERSION_MAJOR: EfuseField = EfuseField::new(1, 3, 118, 2);
/// Disables check of wafer version major
pub const DISABLE_WAFER_VERSION_MAJOR: EfuseField = EfuseField::new(1, 3, 120, 1);
/// Disables check of blk version major
pub const DISABLE_BLK_VERSION_MAJOR: EfuseField = EfuseField::new(1, 3, 121, 1);
/// BLK_VERSION_MINOR of BLOCK2
pub const BLK_VERSION_MINOR: EfuseField = EfuseField::new(1, 3, 122, 3);
/// BLK_VERSION_MAJOR of BLOCK2
pub const BLK_VERSION_MAJOR: EfuseField = EfuseField::new(1, 3, 125, 2);
/// PSRAM capacity
pub const PSRAM_CAP: EfuseField = EfuseField::new(1, 3, 127, 3);
/// Max ambient temperature
pub const TEMP: EfuseField = EfuseField::new(1, 4, 130, 2);
/// PSRAM vendor
pub const PSRAM_VENDOR: EfuseField = EfuseField::new(1, 4, 132, 2);
/// Package version
pub const PKG_VERSION: EfuseField = EfuseField::new(1, 4, 134, 2);
/// Optional unique 128-bit ID
pub const OPTIONAL_UNIQUE_ID: EfuseField = EfuseField::new(2, 0, 0, 128);
/// User data
pub const BLOCK_USR_DATA: EfuseField = EfuseField::new(3, 0, 0, 256);
/// Custom MAC
pub const CUSTOM_MAC: EfuseField = EfuseField::new(3, 6, 200, 48);
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
/// System data part 2 (reserved)
pub const BLOCK_SYS_DATA2: EfuseField = EfuseField::new(9, 0, 0, 256);
