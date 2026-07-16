//! eFuse field definitions for the esp32s31
//!
//! This file was automatically generated, please do not edit it manually!
//!
//! Generated: 2026-07-15 12:48
//! Version:   2ec5c3c5bd07b0b4e073b91d75592d8f

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
        EfuseBlockErrors {
            err_num_reg: 0x20715168u32,
            err_num_mask: None,
            err_num_offset: None,
            fail_bit_reg: 0x20715168u32,
            fail_bit_offset: None,
        },
        EfuseBlockErrors {
            err_num_reg: 0x20715188u32,
            err_num_mask: Some(0x7u32),
            err_num_offset: Some(0x0u32),
            fail_bit_reg: 0x20715188u32,
            fail_bit_offset: Some(0x3u32),
        },
        EfuseBlockErrors {
            err_num_reg: 0x20715188u32,
            err_num_mask: Some(0x7u32),
            err_num_offset: Some(0x4u32),
            fail_bit_reg: 0x20715188u32,
            fail_bit_offset: Some(0x7u32),
        },
        EfuseBlockErrors {
            err_num_reg: 0x20715188u32,
            err_num_mask: Some(0x7u32),
            err_num_offset: Some(0x8u32),
            fail_bit_reg: 0x20715188u32,
            fail_bit_offset: Some(0xbu32),
        },
        EfuseBlockErrors {
            err_num_reg: 0x20715188u32,
            err_num_mask: Some(0x7u32),
            err_num_offset: Some(0xcu32),
            fail_bit_reg: 0x20715188u32,
            fail_bit_offset: Some(0xfu32),
        },
        EfuseBlockErrors {
            err_num_reg: 0x20715188u32,
            err_num_mask: Some(0x7u32),
            err_num_offset: Some(0x10u32),
            fail_bit_reg: 0x20715188u32,
            fail_bit_offset: Some(0x13u32),
        },
        EfuseBlockErrors {
            err_num_reg: 0x20715188u32,
            err_num_mask: Some(0x7u32),
            err_num_offset: Some(0x14u32),
            fail_bit_reg: 0x20715188u32,
            fail_bit_offset: Some(0x17u32),
        },
        EfuseBlockErrors {
            err_num_reg: 0x20715188u32,
            err_num_mask: Some(0x7u32),
            err_num_offset: Some(0x18u32),
            fail_bit_reg: 0x20715188u32,
            fail_bit_offset: Some(0x1bu32),
        },
        EfuseBlockErrors {
            err_num_reg: 0x20715188u32,
            err_num_mask: Some(0x7u32),
            err_num_offset: Some(0x1cu32),
            fail_bit_reg: 0x20715188u32,
            fail_bit_offset: Some(0x1fu32),
        },
        EfuseBlockErrors {
            err_num_reg: 0x2071518cu32,
            err_num_mask: Some(0x7u32),
            err_num_offset: Some(0x4u32),
            fail_bit_reg: 0x2071518cu32,
            fail_bit_offset: Some(0x7u32),
        },
    ];
    pub(crate) const EFUSE_PGM_DATA0_REG: u32 = 0x20715000;
    pub(crate) const EFUSE_PGM_CMD: u32 = 0x2;
    pub(crate) const EFUSE_RD_REPEAT_ERR2_REG: u32 = 0x20715170;
    pub(crate) const EFUSE_RD_REPEAT_ERR6_REG: u32 = 0x20715180;
    pub(crate) const EFUSE_MEM_SIZE: u32 = 0x200;
    pub(crate) const EFUSE_RD_REPEAT_ERR1_REG: u32 = 0x2071516c;
    pub(crate) const EFUSE_RD_RS_ERR1_REG: u32 = 0x2071518c;
    pub(crate) const EFUSE_CMD_REG: u32 = 0x207151d8;
    pub(crate) const EFUSE_PWR_OFF_NUM_M: u32 = 0xffff;
    pub(crate) const EFUSE_DAC_CONF_REG: u32 = 0x207151ec;
    pub(crate) const EFUSE_PWR_OFF_NUM_S: u32 = 0x0;
    pub(crate) const EFUSE_DAC_CLK_DIV_S: u32 = 0x0;
    pub(crate) const EFUSE_RD_REPEAT_ERR5_REG: u32 = 0x2071517c;
    pub(crate) const EFUSE_CHECK_VALUE0_REG: u32 = 0x20715020;
    pub(crate) const EFUSE_RD_RS_ERR0_REG: u32 = 0x20715188;
    pub(crate) const EFUSE_CLK_REG: u32 = 0x207151c8;
    pub(crate) const EFUSE_DAC_NUM_S: u32 = 0x9;
    pub(crate) const EFUSE_RD_TIM_CONF_REG: u32 = 0x207151f0;
    pub(crate) const CODING_SCHEME_NONE: u32 = 0x0;
    pub(crate) const EFUSE_WR_TIM_CONF1_REG: u32 = 0x207151f4;
    pub(crate) const EFUSE_DAC_NUM_M: u32 = 0x1fe00;
    pub(crate) const CODING_SCHEME_REPEAT: u32 = 0x2;
    pub(crate) const EFUSE_PWR_ON_NUM_M: u32 = 0xffff00;
    pub(crate) const EFUSE_RD_REPEAT_ERR4_REG: u32 = 0x20715178;
    pub(crate) const CODING_SCHEME_RS: u32 = 0x4;
    pub(crate) const EFUSE_READ_OP_CODE: u32 = 0x5aa5;
    pub(crate) const EFUSE_DAC_CLK_DIV_M: u32 = 0xff;
    pub(crate) const CODING_SCHEME_34: u32 = 0x1;
    pub(crate) const EFUSE_DATE_REG: u32 = 0x20715190;
    pub(crate) const EFUSE_RD_REPEAT_ERR0_REG: u32 = 0x20715168;
    pub(crate) const EFUSE_RD_REPEAT_ERR3_REG: u32 = 0x20715174;
    pub(crate) const EFUSE_RD_REPEAT_ERR7_REG: u32 = 0x20715184;
    pub(crate) const EFUSE_WR_TIM_CONF2_REG: u32 = 0x207151f8;
    pub(crate) const EFUSE_PWR_ON_NUM_S: u32 = 0x8;
    pub(crate) const CODING_SCHEME_NONE_RECOVERY: u32 = 0x3;
    pub(crate) const EFUSE_WRITE_OP_CODE: u32 = 0x5a5a;
    pub(crate) const EFUSE_READ_CMD: u32 = 0x1;
    pub(crate) const EFUSE_STATUS_REG: u32 = 0x207151d4;
    pub(crate) const EFUSE_CONF_REG: u32 = 0x207151cc;
    pub(crate) const EFUSE_PGM_CMD_MASK: u32 = 0x3;
}

/// Disable programming of individual eFuses
pub const WR_DIS: EfuseField = EfuseField::new(0, 0, 0, 32);
/// Disable reading from BlOCK4-9
pub const RD_DIS: EfuseField = EfuseField::new(0, 1, 32, 7);
/// Reserved; it was created by set_missed_fields_in_regs func
pub const RESERVE_0_39: EfuseField = EfuseField::new(0, 1, 39, 3);
/// Represents whether the function of usb switch to jtag is disabled or
/// enabled. 1: disabled 0: enabled
pub const DIS_USB_JTAG: EfuseField = EfuseField::new(0, 1, 42, 1);
/// Represents whether USB-Serial-JTAG is disabled or enabled. 1: disabled 0:
/// enabled
pub const DIS_USB_SERIAL_JTAG: EfuseField = EfuseField::new(0, 1, 43, 1);
/// Represents whether the function that forces chip into download mode is
/// disabled or enabled. 1: disabled 0: enabled
pub const DIS_FORCE_DOWNLOAD: EfuseField = EfuseField::new(0, 1, 44, 1);
/// Represents whether SPI0 controller during boot_mode_download is disabled or
/// enabled. 1: disabled 0: enabled
pub const SPI_DOWNLOAD_MSPI_DIS: EfuseField = EfuseField::new(0, 1, 45, 1);
/// Represents whether TWAI function is disabled or enabled. 1: disabled 0:
/// enabled
pub const DIS_TWAI: EfuseField = EfuseField::new(0, 1, 46, 1);
/// Represents whether the selection between usb_to_jtag and pad_to_jtag through
/// strapping gpio15 when both EFUSE_DIS_PAD_JTAG and EFUSE_DIS_USB_JTAG are
/// equal to 0 is enabled or disabled. 1: enabled 0: disabled
pub const JTAG_SEL_ENABLE: EfuseField = EfuseField::new(0, 1, 47, 1);
/// Represents whether JTAG is disabled in soft way. Odd number: disabled Even
/// number: enabled
pub const SOFT_DIS_JTAG: EfuseField = EfuseField::new(0, 1, 48, 3);
/// Represents whether JTAG is disabled in the hard way(permanently). 1:
/// disabled 0: enabled
pub const DIS_PAD_JTAG: EfuseField = EfuseField::new(0, 1, 51, 1);
/// Represents whether flash encrypt function is disabled or enabled(except in
/// SPI boot mode). 1: disabled 0: enabled
pub const DIS_DOWNLOAD_MANUAL_ENCRYPT: EfuseField = EfuseField::new(0, 1, 52, 1);
/// Reserved; it was created by set_missed_fields_in_regs func
pub const RESERVE_0_53: EfuseField = EfuseField::new(0, 1, 53, 1);
/// Represents whether the WIFI6 feature is enabled or disabled. 1: WIFI6 is
/// disabled; 0: WIFI6 is enabled
pub const DIS_WIFI6: EfuseField = EfuseField::new(0, 1, 54, 1);
/// Represents the control of validation of HUK generate mode. Odd of 1 is
/// invalid; even of 1 is valid
pub const HUK_GEN_STATE: EfuseField = EfuseField::new(0, 1, 55, 5);
/// Reserved; it was created by set_missed_fields_in_regs func
pub const RESERVE_0_60: EfuseField = EfuseField::new(0, 1, 60, 4);
/// Represents the control of key manager random number switch cycle. 0: control
/// by register. 1: 8 km clk cycles. 2: 16 km cycles. 3: 32 km cycles
pub const KM_RND_SWITCH_CYCLE: EfuseField = EfuseField::new(0, 2, 64, 1);
/// Reserved; it was created by set_missed_fields_in_regs func
pub const RESERVE_0_65: EfuseField = EfuseField::new(0, 2, 65, 1);
/// Represents whether the deploy mode of key manager is disable or not.  1:
/// disabled  0: enabled. bit 0: ecsda; bit 1: flash & spi boot srambler; bit2:
/// hmac & aes; bit3: ds & rma nonce; bit4: psram
pub const KM_DISABLE_DEPLOY_MODE: EfuseField = EfuseField::new(0, 2, 66, 5);
/// Represents whether corresponding key can only be deployed once. 1 is true; 0
/// is false.  0: ecsda 1: flash & spi boot srambler 2: hmac & aes 3: ds & rma
/// nonce 4: psram
pub const KM_DEPLOY_ONLY_ONCE: EfuseField = EfuseField::new(0, 2, 71, 5);
/// Represents whether corresponding key must come from key manager. 1 is true;
/// 0 is false. 0: ecsda 1: flash 2: reserved 3: reserved 4: psram
pub const FORCE_USE_KEY_MANAGER_KEY: EfuseField = EfuseField::new(0, 2, 76, 5);
/// Represents whether to disable software written init key; and force use
/// efuse_init_key
pub const FORCE_DISABLE_SW_INIT_KEY: EfuseField = EfuseField::new(0, 2, 81, 1);
/// Represents whether to configure flash encryption use xts-128 key. else use
/// xts-256 key.  0: 128-bit key  1: 256-bit key
pub const KM_XTS_KEY_LENGTH_256: EfuseField = EfuseField::new(0, 2, 82, 1);
/// Represents the threshold level of the RTC watchdog STG0 timeout.0: Original
/// threshold configuration value of STG0 *2 1: Original threshold configuration
/// value of STG0 *4 2: Original threshold configuration value of STG0 *8 3:
/// Original threshold configuration value of STG0 *16
pub const WDT_DELAY_SEL: EfuseField = EfuseField::new(0, 2, 83, 1);
/// Represents whether to disable all the SM crypto functions; including SM2;
/// SM3. 1: disabled 0: enabled
pub const DIS_SM_CRYPT: EfuseField = EfuseField::new(0, 2, 84, 1);
/// Enables flash encryption when 1 or 3 bits are set and disables otherwise
pub const SPI_BOOT_CRYPT_CNT: EfuseField = EfuseField::new(0, 2, 85, 3);
/// Revoke 1st secure boot key
pub const SECURE_BOOT_KEY_REVOKE0: EfuseField = EfuseField::new(0, 2, 88, 1);
/// Revoke 2nd secure boot key
pub const SECURE_BOOT_KEY_REVOKE1: EfuseField = EfuseField::new(0, 2, 89, 1);
/// Revoke 3rd secure boot key
pub const SECURE_BOOT_KEY_REVOKE2: EfuseField = EfuseField::new(0, 2, 90, 1);
/// Reserved; it was created by set_missed_fields_in_regs func
pub const RESERVE_0_91: EfuseField = EfuseField::new(0, 2, 91, 5);
/// Represents the purpose of Key0
pub const KEY_PURPOSE_0: EfuseField = EfuseField::new(0, 3, 96, 5);
/// Represents the purpose of Key1
pub const KEY_PURPOSE_1: EfuseField = EfuseField::new(0, 3, 101, 5);
/// Represents the purpose of Key2
pub const KEY_PURPOSE_2: EfuseField = EfuseField::new(0, 3, 106, 5);
/// Represents the purpose of Key3
pub const KEY_PURPOSE_3: EfuseField = EfuseField::new(0, 3, 111, 5);
/// Represents the purpose of Key4
pub const KEY_PURPOSE_4: EfuseField = EfuseField::new(0, 3, 116, 5);
/// Represents whether permanently turn on ECC const-time mode.  1: turn on 0:
/// turn off
pub const ECC_FORCE_CONST_TIME: EfuseField = EfuseField::new(0, 3, 121, 1);
/// Represents whether permanently turn off ECDSA software set KEY. 1: turn off
/// 0: turn on
pub const ECDSA_DISABLE_SOFT_K: EfuseField = EfuseField::new(0, 3, 122, 1);
/// Represents the spa secure level by configuring the clock random divide mode
pub const SEC_DPA_LEVEL: EfuseField = EfuseField::new(0, 3, 123, 2);
/// Represents whether to enable xts clock anti-dpa attack function.0: Disabled.
/// 1: Enabled
pub const XTS_DPA_CLK_ENABLE: EfuseField = EfuseField::new(0, 3, 125, 1);
/// Reserved; it was created by set_missed_fields_in_regs func
pub const RESERVE_0_126: EfuseField = EfuseField::new(0, 3, 126, 2);
/// Represents the control of the xts pseudo-round anti-dpa attack function. 0:
/// controlled by register. 1-3: the higher the value is; the more pseudo-rounds
/// are inserted to the xts-aes calculation
pub const XTS_DPA_PSEUDO_LEVEL: EfuseField = EfuseField::new(0, 4, 128, 2);
/// Represents whether secure boot is enabled or disabled. 1: enabled 0:
/// disabled
pub const SECURE_BOOT_EN: EfuseField = EfuseField::new(0, 4, 130, 1);
/// Represents whether revoking aggressive secure boot is enabled or disabled.
/// 1: enabled. 0: disabled
pub const SECURE_BOOT_AGGRESSIVE_REVOKE: EfuseField = EfuseField::new(0, 4, 131, 1);
/// Reserved; it was created by set_missed_fields_in_regs func
pub const RESERVE_0_132: EfuseField = EfuseField::new(0, 4, 132, 1);
/// flash type: 0: nor flash; 1: nand flash
pub const FLASH_TYPE: EfuseField = EfuseField::new(0, 4, 133, 1);
/// Reserved; it was created by set_missed_fields_in_regs func
pub const RESERVE_0_134: EfuseField = EfuseField::new(0, 4, 134, 3);
/// Set this bit to disable download via USB-OTG
pub const DIS_USB_OTG_DOWNLOAD_MODE: EfuseField = EfuseField::new(0, 4, 137, 1);
/// Reserved; it was created by set_missed_fields_in_regs func
pub const RESERVE_0_138: EfuseField = EfuseField::new(0, 4, 138, 2);
/// Represents the flash waiting time after power-up; in unit of ms. When the
/// value less than 15; the waiting time is the programmed value. Otherwise; the
/// waiting time is 2 times the programmed value
pub const FLASH_TPUW: EfuseField = EfuseField::new(0, 4, 140, 4);
/// Represents whether Download mode is disabled or enabled. 1: disabled 0:
/// enabled
pub const DIS_DOWNLOAD_MODE: EfuseField = EfuseField::new(0, 4, 144, 1);
/// Represents whether direct boot mode is disabled or enabled. 1: disabled 0:
/// enabled
pub const DIS_DIRECT_BOOT: EfuseField = EfuseField::new(0, 4, 145, 1);
/// Represents whether print from USB-Serial-JTAG is disabled or enabled. 1:
/// disabled 0: enabled
pub const DIS_USB_SERIAL_JTAG_ROM_PRINT: EfuseField = EfuseField::new(0, 4, 146, 1);
/// Represetns whether to lock the efuse xts key. 1. Lock 0: Unlock
pub const LOCK_KM_KEY: EfuseField = EfuseField::new(0, 4, 147, 1);
/// Represents whether the USB-Serial-JTAG download function is disabled or
/// enabled. 1: Disable 0: Enable
pub const DIS_USB_SERIAL_JTAG_DOWNLOAD_MODE: EfuseField = EfuseField::new(0, 4, 148, 1);
/// Represents whether security download is enabled or disabled. 1: enabled 0:
/// disabled
pub const ENABLE_SECURITY_DOWNLOAD: EfuseField = EfuseField::new(0, 4, 149, 1);
/// Represents the type of UART printing. 00: force enable printing 01: enable
/// printing when GPIO8 is reset at low level 10: enable printing when GPIO8 is
/// reset at high level 11: force disable printing
pub const UART_PRINT_CONTROL: EfuseField = EfuseField::new(0, 4, 150, 2);
/// Represents whether ROM code is forced to send a resume command during SPI
/// boot. 1: forced 0:not forced
pub const FORCE_SEND_RESUME: EfuseField = EfuseField::new(0, 4, 152, 1);
/// Reserved; it was created by set_missed_fields_in_regs func
pub const RESERVE_0_153: EfuseField = EfuseField::new(0, 4, 153, 7);
/// Represents the version used by ESP-IDF anti-rollback feature
pub const SECURE_VERSION: EfuseField = EfuseField::new(0, 5, 160, 16);
/// Represents whether FAST VERIFY ON WAKE is disabled or enabled when Secure
/// Boot is enabled. 1: disabled 0: enabled
pub const SECURE_BOOT_DISABLE_FAST_WAKE: EfuseField = EfuseField::new(0, 5, 176, 1);
/// Represents whether the hysteresis function of corresponding PAD is enabled.
/// 1: enabled 0:disabled
pub const HYS_EN_PAD: EfuseField = EfuseField::new(0, 5, 177, 1);
/// Reserved; it was created by set_missed_fields_in_regs func
pub const RESERVE_0_178: EfuseField = EfuseField::new(0, 5, 178, 14);
/// Reserved; it was created by set_missed_fields_in_regs func
pub const RESERVE_0_192: EfuseField = EfuseField::new(0, 6, 192, 2);
/// Select dcdc vset use efuse_dcdc_vset
pub const DCDC_VSET_EN: EfuseField = EfuseField::new(0, 6, 194, 1);
/// Set this bit to disable watch dog
pub const DIS_WDT: EfuseField = EfuseField::new(0, 6, 195, 1);
/// Set this bit to disable super-watchdog
pub const DIS_SWD: EfuseField = EfuseField::new(0, 6, 196, 1);
/// Reserved; it was created by set_missed_fields_in_regs func
pub const RESERVE_0_197: EfuseField = EfuseField::new(0, 6, 197, 6);
/// Represents whether secure boot using SHA-384 is enabled. 0: Disable 1:
/// Enable
pub const SECURE_BOOT_SHA384_EN: EfuseField = EfuseField::new(0, 6, 203, 1);
/// Represents the anti-rollback secure version of the 2nd stage bootloader used
/// by the ROM bootloader
pub const BOOTLOADER_ANTI_ROLLBACK_SECURE_VERSION: EfuseField = EfuseField::new(0, 6, 204, 4);
/// Represents whether the ani-rollback check for the 2nd stage bootloader is
/// enabled.1: Enabled0: Disabled
pub const BOOTLOADER_ANTI_ROLLBACK_EN: EfuseField = EfuseField::new(0, 6, 208, 1);
/// Represents whether the ani-rollback SECURE_VERSION will be updated from the
/// ROM bootloader.1: Enable0: Disable
pub const BOOTLOADER_ANTI_ROLLBACK_UPDATE_IN_ROM: EfuseField = EfuseField::new(0, 6, 209, 1);
/// Represents the starting flash sector (flash sector size is 0x1000) of the
/// recovery bootloader used by the ROM bootloader If the primary bootloader
/// fails. 0 and 0xFFF - this feature is disabled
pub const RECOVERY_BOOTLOADER_FLASH_SECTOR: EfuseField = EfuseField::new(0, 6, 210, 12);
/// Represents whether rma function is supported in download mode. 2'b01/2'b10:
/// enabled2'b00/2'b11: disabled
pub const RMA_ENA: EfuseField = EfuseField::new(0, 6, 222, 2);
/// Represents the number of times the RMA session has been entered
pub const RMA_SESSION_COUNTER: EfuseField = EfuseField::new(0, 7, 224, 3);
/// Represents whether random number NONCE is used in RMA and whether the KM
/// module is used to generate the NONCE. 2'bx0: No NONCE 2'b1x: Use KM generate
/// NONCE.
pub const RMA_NONCE_ENA: EfuseField = EfuseField::new(0, 7, 227, 2);
/// Represents whether HUK_info is selected as the source for calculating
/// CHIP_info in RMA.1: use HUK_info 0: use UNIQ_id
pub const RMA_CHIP_INFO_SOURCE: EfuseField = EfuseField::new(0, 7, 229, 1);
/// Represents whether disable FAST_VEF in RMA session.1: disable0: enable
pub const RMA_DISABLE_FAST_VEF: EfuseField = EfuseField::new(0, 7, 230, 1);
/// Represents whether to enable PVT power glitch monitor function.1:Enable.
/// 0:Disable
pub const PVT_0_GLITCH_EN: EfuseField = EfuseField::new(0, 7, 231, 1);
/// Use to configure glitch mode
pub const PVT_0_GLITCH_MODE: EfuseField = EfuseField::new(0, 7, 232, 2);
/// Represents whether to enable PVT power glitch monitor function.1:Enable.
/// 0:Disable
pub const PVT_1_GLITCH_EN: EfuseField = EfuseField::new(0, 7, 234, 1);
/// Use to configure glitch mode
pub const PVT_1_GLITCH_MODE: EfuseField = EfuseField::new(0, 7, 235, 2);
/// FLASH power select. 1'b1: use 3.3V1'b0: use 1.8V
pub const PMU_FLASH_POWER_SEL: EfuseField = EfuseField::new(0, 7, 237, 1);
/// FLASH power select enable signal. 1'b1 : validates EFUSE_PMU_FLASH_POWER_SEL
/// 1'b0: invalidates EFUSE_PMU_FLASH_POWER_SEL
pub const PMU_FLASH_POWER_SEL_EN: EfuseField = EfuseField::new(0, 7, 238, 1);
/// set these bit enable power glitch enable
pub const POWER_GLITCH_EN: EfuseField = EfuseField::new(0, 7, 239, 4);
/// Represents whether to enable XTS-AES shadow core countermeasure against
/// fault injection attacks.  0: Disabled 1: Enabled
pub const ENA_XTS_SHADOW: EfuseField = EfuseField::new(0, 7, 243, 1);
/// Represents whether to enable ciphertext scrambler for external memory .  0:
/// Disabled 1: Enabled
pub const ENA_SPI_BOOT_CRYPT_SCRAMBLER: EfuseField = EfuseField::new(0, 7, 244, 1);
/// Represents which Crypto peripheral is selected for re-enabling JTAG.  0: RMA
/// 1: HMAC
pub const RE_ENABLE_JTAG_SOURCE: EfuseField = EfuseField::new(0, 7, 245, 1);
/// Reserved; it was created by set_missed_fields_in_regs func
pub const RESERVE_0_246: EfuseField = EfuseField::new(0, 7, 246, 10);
/// Reserved
pub const REPEAT7_RSVD: EfuseField = EfuseField::new(0, 8, 256, 16);
/// Reserved; it was created by set_missed_fields_in_regs func
pub const RESERVE_0_272: EfuseField = EfuseField::new(0, 8, 272, 16);
/// MAC address
pub const MAC0: EfuseField = EfuseField::new(1, 0, 0, 32);
/// MAC address
pub const MAC1: EfuseField = EfuseField::new(1, 1, 32, 16);
/// Represents the extended bits of MAC address
pub const MAC_EXT: EfuseField = EfuseField::new(1, 1, 48, 16);
/// Reserved
pub const MAC_RESERVED_0: EfuseField = EfuseField::new(1, 2, 64, 14);
/// Reserved
pub const MAC_RESERVED_1: EfuseField = EfuseField::new(1, 2, 78, 18);
/// Reserved
pub const MAC_RESERVED_2: EfuseField = EfuseField::new(1, 3, 96, 18);
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
/// Psram capacity
pub const PSRAM_CAP: EfuseField = EfuseField::new(1, 3, 127, 3);
/// Maximum ambient temperature that ESP Chip can work properly
pub const TEMP: EfuseField = EfuseField::new(1, 4, 130, 2);
/// Psram vendor
pub const PSRAM_VENDOR: EfuseField = EfuseField::new(1, 4, 132, 2);
/// Package version
pub const PKG_VERSION: EfuseField = EfuseField::new(1, 4, 134, 2);
/// reserved
pub const RESERVED_1_136: EfuseField = EfuseField::new(1, 4, 136, 24);
/// Represents the third 32-bit of zeroth part of system data
pub const SYS_DATA_PART0_2: EfuseField = EfuseField::new(1, 5, 160, 32);
/// Optional unique 128-bit ID
pub const OPTIONAL_UNIQUE_ID: EfuseField = EfuseField::new(2, 0, 0, 128);
/// Represents the zeroth 32-bit of first part of system data
pub const SYS_DATA_PART1_4: EfuseField = EfuseField::new(2, 4, 128, 32);
/// Represents the zeroth 32-bit of first part of system data
pub const SYS_DATA_PART1_5: EfuseField = EfuseField::new(2, 5, 160, 32);
/// Represents the zeroth 32-bit of first part of system data
pub const SYS_DATA_PART1_6: EfuseField = EfuseField::new(2, 6, 192, 32);
/// Represents the zeroth 32-bit of first part of system data
pub const SYS_DATA_PART1_7: EfuseField = EfuseField::new(2, 7, 224, 32);
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
/// System data part 2 (reserved)
pub const BLOCK_SYS_DATA2: EfuseField = EfuseField::new(9, 0, 0, 32);
/// Represents the first 32-bit of second part of system data
pub const SYS_DATA_PART2_1: EfuseField = EfuseField::new(9, 1, 32, 32);
/// Represents the second 32-bit of second part of system data
pub const SYS_DATA_PART2_2: EfuseField = EfuseField::new(9, 2, 64, 32);
/// Represents the third 32-bit of second part of system data
pub const SYS_DATA_PART2_3: EfuseField = EfuseField::new(9, 3, 96, 32);
/// Represents the fourth 32-bit of second part of system data
pub const SYS_DATA_PART2_4: EfuseField = EfuseField::new(9, 4, 128, 32);
/// Represents the fifth 32-bit of second part of system data
pub const SYS_DATA_PART2_5: EfuseField = EfuseField::new(9, 5, 160, 32);
/// Represents whether enable usb device exchange pins of D+ and D- or not.  1:
/// enabled 0: disabled
pub const USB_DEVICE_EXCHG_PINS: EfuseField = EfuseField::new(9, 6, 192, 1);
/// Represents the single-end input threshold vrefh; 1.76 V to 2 V with step of
/// 80 mV
pub const USB_DEVICE_DREFH: EfuseField = EfuseField::new(9, 6, 193, 2);
/// Represents the usb device single-end input low threshold; 0.8 V to 1.04 V
/// with step of 80 mV
pub const USB_DEVICE_DREFL: EfuseField = EfuseField::new(9, 6, 195, 2);
/// Reserved; it was created by set_missed_fields_in_regs func
pub const RESERVE_9_197: EfuseField = EfuseField::new(9, 6, 197, 12);
/// Power glitch monitor PVT cell select
pub const PVT_0_CELL_SELECT: EfuseField = EfuseField::new(9, 6, 209, 7);
/// Power glitch monitor PVT cell select
pub const PVT_1_CELL_SELECT: EfuseField = EfuseField::new(9, 6, 216, 7);
/// Reserved; it was created by set_missed_fields_in_regs func
pub const RESERVE_9_223: EfuseField = EfuseField::new(9, 6, 223, 1);
/// Power glitch monitor threthold
pub const PVT_0_LIMIT: EfuseField = EfuseField::new(9, 7, 224, 16);
/// Power glitch monitor threthold
pub const PVT_1_LIMIT: EfuseField = EfuseField::new(9, 7, 240, 16);
