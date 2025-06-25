//! eFuse field definitions for the esp32p4
//!
//! This file was automatically generated, please do not edit it manually!
//!
//! Generated: 2025-06-25 11:06
//! Version:   f7765f0ac3faf4b54f8c1f064307522c

#![allow(unused)]

use super::EfuseField;

/// Total size in bytes of each block
pub(crate) const BLOCK_SIZES: &[u32] = &[24, 24, 32, 32, 32, 32, 32, 32, 32, 32, 32];

/// Disable programming of individual eFuses
pub const WR_DIS: EfuseField = EfuseField::new(0, 0, 0, 32);
/// Disable reading from BlOCK4-10
pub const RD_DIS: EfuseField = EfuseField::new(0, 1, 32, 7);
/// Enable usb device exchange pins of D+ and D-
pub const USB_DEVICE_EXCHG_PINS: EfuseField = EfuseField::new(0, 1, 39, 1);
/// Enable usb otg11 exchange pins of D+ and D-
pub const USB_OTG11_EXCHG_PINS: EfuseField = EfuseField::new(0, 1, 40, 1);
/// Represents whether the function of usb switch to jtag is disabled or
/// enabled. 1: disabled. 0: enabled
pub const DIS_USB_JTAG: EfuseField = EfuseField::new(0, 1, 41, 1);
/// Represents whether power glitch function is enabled. 1: enabled. 0: disabled
pub const POWERGLITCH_EN: EfuseField = EfuseField::new(0, 1, 42, 1);
/// Represents whether USB-Serial-JTAG is disabled or enabled. 1: disabled. 0:
/// enabled
pub const DIS_USB_SERIAL_JTAG: EfuseField = EfuseField::new(0, 1, 43, 1);
/// Represents whether the function that forces chip into download mode is
/// disabled or enabled. 1: disabled. 0: enabled
pub const DIS_FORCE_DOWNLOAD: EfuseField = EfuseField::new(0, 1, 44, 1);
/// Set this bit to disable accessing MSPI flash/MSPI ram by SYS AXI matrix
/// during boot_mode_download
pub const SPI_DOWNLOAD_MSPI_DIS: EfuseField = EfuseField::new(0, 1, 45, 1);
/// Represents whether TWAI function is disabled or enabled. 1: disabled. 0:
/// enabled
pub const DIS_TWAI: EfuseField = EfuseField::new(0, 1, 46, 1);
/// Represents whether the selection between usb_to_jtag and pad_to_jtag through
/// strapping gpio34 when both EFUSE_DIS_PAD_JTAG and EFUSE_DIS_USB_JTAG are
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
/// USB intphy of usb device signle-end input high threshold; 1.76V to 2V. Step
/// by 80mV
pub const USB_DEVICE_DREFH: EfuseField = EfuseField::new(0, 1, 53, 2);
/// USB intphy of usb otg11 signle-end input high threshold; 1.76V to 2V. Step
/// by 80mV
pub const USB_OTG11_DREFH: EfuseField = EfuseField::new(0, 1, 55, 2);
/// TBD
pub const USB_PHY_SEL: EfuseField = EfuseField::new(0, 1, 57, 1);
/// Set this bit to control validation of HUK generate mode. Odd of 1 is
/// invalid; even of 1 is valid
pub const KM_HUK_GEN_STATE: EfuseField = EfuseField::new(0, 1, 58, 9);
/// Set bits to control key manager random number switch cycle. 0: control by
/// register. 1: 8 km clk cycles. 2: 16 km cycles. 3: 32 km cycles
pub const KM_RND_SWITCH_CYCLE: EfuseField = EfuseField::new(0, 2, 67, 2);
/// Set each bit to control whether corresponding key can only be deployed once.
/// 1 is true; 0 is false. Bit0: ecdsa. Bit1: xts. Bit2: hmac. Bit3: ds
pub const KM_DEPLOY_ONLY_ONCE: EfuseField = EfuseField::new(0, 2, 69, 4);
/// Set each bit to control whether corresponding key must come from key
/// manager.. 1 is true; 0 is false. Bit0: ecdsa. Bit1: xts. Bit2: hmac. Bit3:
/// ds
pub const FORCE_USE_KEY_MANAGER_KEY: EfuseField = EfuseField::new(0, 2, 73, 4);
/// Set this bit to disable software written init key; and force use
/// efuse_init_key
pub const FORCE_DISABLE_SW_INIT_KEY: EfuseField = EfuseField::new(0, 2, 77, 1);
/// Set this bit to configure flash encryption use xts-128 key; else use xts-256
/// key
pub const XTS_KEY_LENGTH_256: EfuseField = EfuseField::new(0, 2, 78, 1);
/// Reserved; it was created by set_missed_fields_in_regs func
pub const RESERVE_0_79: EfuseField = EfuseField::new(0, 2, 79, 1);
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
/// Represents whether hardware random number k is forced used in ESDCA. 1:
/// force used. 0: not force used
pub const ECDSA_ENABLE_SOFT_K: EfuseField = EfuseField::new(0, 3, 114, 1);
/// Represents whether anti-dpa attack is enabled. 1:enabled. 0: disabled
pub const CRYPT_DPA_ENABLE: EfuseField = EfuseField::new(0, 3, 115, 1);
/// Represents whether secure boot is enabled or disabled. 1: enabled. 0:
/// disabled
pub const SECURE_BOOT_EN: EfuseField = EfuseField::new(0, 3, 116, 1);
/// Represents whether revoking aggressive secure boot is enabled or disabled.
/// 1: enabled. 0: disabled
pub const SECURE_BOOT_AGGRESSIVE_REVOKE: EfuseField = EfuseField::new(0, 3, 117, 1);
/// Reserved; it was created by set_missed_fields_in_regs func
pub const RESERVE_0_118: EfuseField = EfuseField::new(0, 3, 118, 1);
/// The type of interfaced flash. 0: four data lines; 1: eight data lines
pub const FLASH_TYPE: EfuseField = EfuseField::new(0, 3, 119, 1);
/// Set flash page size
pub const FLASH_PAGE_SIZE: EfuseField = EfuseField::new(0, 3, 120, 2);
/// Set this bit to enable ecc for flash boot
pub const FLASH_ECC_EN: EfuseField = EfuseField::new(0, 3, 122, 1);
/// Set this bit to disable download via USB-OTG
pub const DIS_USB_OTG_DOWNLOAD_MODE: EfuseField = EfuseField::new(0, 3, 123, 1);
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
/// TBD
pub const LOCK_KM_KEY: EfuseField = EfuseField::new(0, 4, 131, 1);
/// Represents whether the USB-Serial-JTAG download function is disabled or
/// enabled. 1: disabled. 0: enabled
pub const DIS_USB_SERIAL_JTAG_DOWNLOAD_MODE: EfuseField = EfuseField::new(0, 4, 132, 1);
/// Represents whether security download is enabled or disabled. 1: enabled. 0:
/// disabled
pub const ENABLE_SECURITY_DOWNLOAD: EfuseField = EfuseField::new(0, 4, 133, 1);
/// Represents the type of UART printing. 00: force enable printing. 01: enable
/// printing when GPIO8 is reset at low level. 10: enable printing when GPIO8 is
/// reset at high level. 11: force disable printing
pub const UART_PRINT_CONTROL: EfuseField = EfuseField::new(0, 4, 134, 2);
/// Represents whether ROM code is forced to send a resume command during SPI
/// boot. 1: forced. 0:not forced
pub const FORCE_SEND_RESUME: EfuseField = EfuseField::new(0, 4, 136, 1);
/// Represents the version used by ESP-IDF anti-rollback feature
pub const SECURE_VERSION: EfuseField = EfuseField::new(0, 4, 137, 16);
/// Represents whether FAST VERIFY ON WAKE is disabled or enabled when Secure
/// Boot is enabled. 1: disabled. 0: enabled
pub const SECURE_BOOT_DISABLE_FAST_WAKE: EfuseField = EfuseField::new(0, 4, 153, 1);
/// Represents whether the hysteresis function of corresponding PAD is enabled.
/// 1: enabled. 0:disabled
pub const HYS_EN_PAD: EfuseField = EfuseField::new(0, 4, 154, 1);
/// Set the dcdc voltage default
pub const DCDC_VSET: EfuseField = EfuseField::new(0, 4, 155, 5);
/// TBD
pub const PXA0_TIEH_SEL_0: EfuseField = EfuseField::new(0, 5, 160, 2);
/// TBD
pub const PXA0_TIEH_SEL_1: EfuseField = EfuseField::new(0, 5, 162, 2);
/// TBD
pub const PXA0_TIEH_SEL_2: EfuseField = EfuseField::new(0, 5, 164, 2);
/// TBD
pub const PXA0_TIEH_SEL_3: EfuseField = EfuseField::new(0, 5, 166, 2);
/// TBD
pub const KM_DISABLE_DEPLOY_MODE: EfuseField = EfuseField::new(0, 5, 168, 4);
/// Represents the usb device single-end input low threshold; 0.8 V to 1.04 V
/// with step of 80 mV
pub const USB_DEVICE_DREFL: EfuseField = EfuseField::new(0, 5, 172, 2);
/// Represents the usb otg11 single-end input low threshold; 0.8 V to 1.04 V
/// with step of 80 mV
pub const USB_OTG11_DREFL: EfuseField = EfuseField::new(0, 5, 174, 2);
/// Reserved; it was created by set_missed_fields_in_regs func
pub const RESERVE_0_176: EfuseField = EfuseField::new(0, 5, 176, 2);
/// HP system power source select. 0:LDO. 1: DCDC
pub const HP_PWR_SRC_SEL: EfuseField = EfuseField::new(0, 5, 178, 1);
/// Select dcdc vset use efuse_dcdc_vset
pub const DCDC_VSET_EN: EfuseField = EfuseField::new(0, 5, 179, 1);
/// Set this bit to disable watch dog
pub const DIS_WDT: EfuseField = EfuseField::new(0, 5, 180, 1);
/// Set this bit to disable super-watchdog
pub const DIS_SWD: EfuseField = EfuseField::new(0, 5, 181, 1);
/// Reserved; it was created by set_missed_fields_in_regs func
pub const RESERVE_0_182: EfuseField = EfuseField::new(0, 5, 182, 10);
/// MAC address
pub const MAC0: EfuseField = EfuseField::new(1, 0, 0, 32);
/// MAC address
pub const MAC1: EfuseField = EfuseField::new(1, 1, 32, 16);
/// Stores the extended bits of MAC address
pub const RESERVED_1_16: EfuseField = EfuseField::new(1, 1, 48, 16);
/// Minor chip version
pub const WAFER_VERSION_MINOR: EfuseField = EfuseField::new(1, 2, 64, 4);
/// Major chip version (lower 2 bits)
pub const WAFER_VERSION_MAJOR_LO: EfuseField = EfuseField::new(1, 2, 68, 2);
/// Disables check of wafer version major
pub const DISABLE_WAFER_VERSION_MAJOR: EfuseField = EfuseField::new(1, 2, 70, 1);
/// Disables check of blk version major
pub const DISABLE_BLK_VERSION_MAJOR: EfuseField = EfuseField::new(1, 2, 71, 1);
/// BLK_VERSION_MINOR of BLOCK2
pub const BLK_VERSION_MINOR: EfuseField = EfuseField::new(1, 2, 72, 3);
/// BLK_VERSION_MAJOR of BLOCK2
pub const BLK_VERSION_MAJOR: EfuseField = EfuseField::new(1, 2, 75, 2);
/// PSRAM capacity
pub const PSRAM_CAP: EfuseField = EfuseField::new(1, 2, 77, 3);
/// Operating temperature of the ESP chip
pub const TEMP: EfuseField = EfuseField::new(1, 2, 80, 2);
/// PSRAM vendor
pub const PSRAM_VENDOR: EfuseField = EfuseField::new(1, 2, 82, 2);
/// Package version
pub const PKG_VERSION: EfuseField = EfuseField::new(1, 2, 84, 3);
/// Major chip version (MSB)
pub const WAFER_VERSION_MAJOR_HI: EfuseField = EfuseField::new(1, 2, 87, 1);
/// Output VO1 parameter
pub const LDO_VO1_DREF: EfuseField = EfuseField::new(1, 2, 88, 4);
/// Output VO2 parameter
pub const LDO_VO2_DREF: EfuseField = EfuseField::new(1, 2, 92, 4);
/// Output VO1 parameter
pub const LDO_VO1_MUL: EfuseField = EfuseField::new(1, 3, 96, 3);
/// Output VO2 parameter
pub const LDO_VO2_MUL: EfuseField = EfuseField::new(1, 3, 99, 3);
/// Output VO3 calibration parameter
pub const LDO_VO3_K: EfuseField = EfuseField::new(1, 3, 102, 8);
/// Output VO3 calibration parameter
pub const LDO_VO3_VOS: EfuseField = EfuseField::new(1, 3, 110, 6);
/// Output VO3 calibration parameter
pub const LDO_VO3_C: EfuseField = EfuseField::new(1, 3, 116, 6);
/// Output VO4 calibration parameter
pub const LDO_VO4_K: EfuseField = EfuseField::new(1, 3, 122, 8);
/// Output VO4 calibration parameter
pub const LDO_VO4_VOS: EfuseField = EfuseField::new(1, 4, 130, 6);
/// Output VO4 calibration parameter
pub const LDO_VO4_C: EfuseField = EfuseField::new(1, 4, 136, 6);
/// reserved
pub const RESERVED_1_142: EfuseField = EfuseField::new(1, 4, 142, 2);
/// Active HP DBIAS of fixed voltage
pub const ACTIVE_HP_DBIAS: EfuseField = EfuseField::new(1, 4, 144, 4);
/// Active LP DBIAS of fixed voltage
pub const ACTIVE_LP_DBIAS: EfuseField = EfuseField::new(1, 4, 148, 4);
/// LSLP HP DBIAS of fixed voltage
pub const LSLP_HP_DBIAS: EfuseField = EfuseField::new(1, 4, 152, 4);
/// DSLP BDG of fixed voltage
pub const DSLP_DBG: EfuseField = EfuseField::new(1, 4, 156, 4);
/// DSLP LP DBIAS of fixed voltage
pub const DSLP_LP_DBIAS: EfuseField = EfuseField::new(1, 5, 160, 5);
/// DBIAS gap between LP and DCDC
pub const LP_DCDC_DBIAS_VOL_GAP: EfuseField = EfuseField::new(1, 5, 165, 5);
/// reserved
pub const RESERVED_1_170: EfuseField = EfuseField::new(1, 5, 170, 22);
/// Optional unique 128-bit ID
pub const OPTIONAL_UNIQUE_ID: EfuseField = EfuseField::new(2, 0, 0, 128);
/// Average initcode of ADC1 atten0
pub const ADC1_AVE_INITCODE_ATTEN0: EfuseField = EfuseField::new(2, 4, 128, 10);
/// Average initcode of ADC1 atten1
pub const ADC1_AVE_INITCODE_ATTEN1: EfuseField = EfuseField::new(2, 4, 138, 10);
/// Average initcode of ADC1 atten2
pub const ADC1_AVE_INITCODE_ATTEN2: EfuseField = EfuseField::new(2, 4, 148, 10);
/// Average initcode of ADC1 atten3
pub const ADC1_AVE_INITCODE_ATTEN3: EfuseField = EfuseField::new(2, 4, 158, 10);
/// Average initcode of ADC2 atten0
pub const ADC2_AVE_INITCODE_ATTEN0: EfuseField = EfuseField::new(2, 5, 168, 10);
/// Average initcode of ADC2 atten1
pub const ADC2_AVE_INITCODE_ATTEN1: EfuseField = EfuseField::new(2, 5, 178, 10);
/// Average initcode of ADC2 atten2
pub const ADC2_AVE_INITCODE_ATTEN2: EfuseField = EfuseField::new(2, 5, 188, 10);
/// Average initcode of ADC2 atten3
pub const ADC2_AVE_INITCODE_ATTEN3: EfuseField = EfuseField::new(2, 6, 198, 10);
/// HI_DOUT of ADC1 atten0
pub const ADC1_HI_DOUT_ATTEN0: EfuseField = EfuseField::new(2, 6, 208, 10);
/// HI_DOUT of ADC1 atten1
pub const ADC1_HI_DOUT_ATTEN1: EfuseField = EfuseField::new(2, 6, 218, 10);
/// HI_DOUT of ADC1 atten2
pub const ADC1_HI_DOUT_ATTEN2: EfuseField = EfuseField::new(2, 7, 228, 10);
/// HI_DOUT of ADC1 atten3
pub const ADC1_HI_DOUT_ATTEN3: EfuseField = EfuseField::new(2, 7, 238, 10);
/// reserved
pub const RESERVED_2_248: EfuseField = EfuseField::new(2, 7, 248, 8);
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
/// HI_DOUT of ADC2 atten0
pub const ADC2_HI_DOUT_ATTEN0: EfuseField = EfuseField::new(10, 0, 0, 10);
/// HI_DOUT of ADC2 atten1
pub const ADC2_HI_DOUT_ATTEN1: EfuseField = EfuseField::new(10, 0, 10, 10);
/// HI_DOUT of ADC2 atten2
pub const ADC2_HI_DOUT_ATTEN2: EfuseField = EfuseField::new(10, 0, 20, 10);
/// HI_DOUT of ADC2 atten3
pub const ADC2_HI_DOUT_ATTEN3: EfuseField = EfuseField::new(10, 0, 30, 10);
/// Gap between ADC1_ch0 and average initcode
pub const ADC1_CH0_ATTEN0_INITCODE_DIFF: EfuseField = EfuseField::new(10, 1, 40, 4);
/// Gap between ADC1_ch1 and average initcode
pub const ADC1_CH1_ATTEN0_INITCODE_DIFF: EfuseField = EfuseField::new(10, 1, 44, 4);
/// Gap between ADC1_ch2 and average initcode
pub const ADC1_CH2_ATTEN0_INITCODE_DIFF: EfuseField = EfuseField::new(10, 1, 48, 4);
/// Gap between ADC1_ch3 and average initcode
pub const ADC1_CH3_ATTEN0_INITCODE_DIFF: EfuseField = EfuseField::new(10, 1, 52, 4);
/// Gap between ADC1_ch4 and average initcode
pub const ADC1_CH4_ATTEN0_INITCODE_DIFF: EfuseField = EfuseField::new(10, 1, 56, 4);
/// Gap between ADC1_ch5 and average initcode
pub const ADC1_CH5_ATTEN0_INITCODE_DIFF: EfuseField = EfuseField::new(10, 1, 60, 4);
/// Gap between ADC1_ch6 and average initcode
pub const ADC1_CH6_ATTEN0_INITCODE_DIFF: EfuseField = EfuseField::new(10, 2, 64, 4);
/// Gap between ADC1_ch7 and average initcode
pub const ADC1_CH7_ATTEN0_INITCODE_DIFF: EfuseField = EfuseField::new(10, 2, 68, 4);
/// Gap between ADC2_ch0 and average initcode
pub const ADC2_CH0_ATTEN0_INITCODE_DIFF: EfuseField = EfuseField::new(10, 2, 72, 4);
/// Gap between ADC2_ch1 and average initcode
pub const ADC2_CH1_ATTEN0_INITCODE_DIFF: EfuseField = EfuseField::new(10, 2, 76, 4);
/// Gap between ADC2_ch2 and average initcode
pub const ADC2_CH2_ATTEN0_INITCODE_DIFF: EfuseField = EfuseField::new(10, 2, 80, 4);
/// Gap between ADC2_ch3 and average initcode
pub const ADC2_CH3_ATTEN0_INITCODE_DIFF: EfuseField = EfuseField::new(10, 2, 84, 4);
/// Gap between ADC2_ch4 and average initcode
pub const ADC2_CH4_ATTEN0_INITCODE_DIFF: EfuseField = EfuseField::new(10, 2, 88, 4);
/// Gap between ADC2_ch5 and average initcode
pub const ADC2_CH5_ATTEN0_INITCODE_DIFF: EfuseField = EfuseField::new(10, 2, 92, 4);
/// Temperature calibration data
pub const TEMPERATURE_SENSOR: EfuseField = EfuseField::new(10, 3, 96, 9);
/// reserved
pub const RESERVED_10_105: EfuseField = EfuseField::new(10, 3, 105, 23);
/// Stores the $nth 32 bits of the 2nd part of system data
pub const SYS_DATA_PART2_4: EfuseField = EfuseField::new(10, 4, 128, 32);
/// Stores the $nth 32 bits of the 2nd part of system data
pub const SYS_DATA_PART2_5: EfuseField = EfuseField::new(10, 5, 160, 32);
/// Stores the $nth 32 bits of the 2nd part of system data
pub const SYS_DATA_PART2_6: EfuseField = EfuseField::new(10, 6, 192, 32);
/// Stores the $nth 32 bits of the 2nd part of system data
pub const SYS_DATA_PART2_7: EfuseField = EfuseField::new(10, 7, 224, 32);
