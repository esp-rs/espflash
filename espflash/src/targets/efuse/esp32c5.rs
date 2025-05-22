//! This file was automatically generated, please do not edit it manually!
//!
//! Generated: 2025-05-19 11:59
//! Version:   287a0ed4951aba84b9571a5f31000275

#![allow(unused)]

use super::EfuseField;

/// Total size in bytes of each block
pub(crate) const BLOCK_SIZES: &[u32] = &[24, 24, 32, 32, 32, 32, 32, 32, 32, 32, 32];

/// Disable programming of individual eFuses
pub(crate) const WR_DIS: EfuseField = EfuseField::new(0, 0, 0, 32);
/// Disable reading from BlOCK4-10
pub(crate) const RD_DIS: EfuseField = EfuseField::new(0, 1, 32, 7);
/// Reserved; it was created by set_missed_fields_in_regs func
pub(crate) const RESERVE_0_39: EfuseField = EfuseField::new(0, 1, 39, 1);
/// Represents whether icache is disabled or enabled.\\ 1: disabled\\ 0:
/// enabled\\
pub(crate) const DIS_ICACHE: EfuseField = EfuseField::new(0, 1, 40, 1);
/// Represents whether the function of usb switch to jtag is disabled or
/// enabled.\\ 1: disabled\\ 0: enabled\\
pub(crate) const DIS_USB_JTAG: EfuseField = EfuseField::new(0, 1, 41, 1);
/// Reserved; it was created by set_missed_fields_in_regs func
pub(crate) const RESERVE_0_42: EfuseField = EfuseField::new(0, 1, 42, 1);
/// Represents whether USB-Serial-JTAG is disabled or enabled.\\ 1: disabled\\
/// 0: enabled\\
pub(crate) const DIS_USB_SERIAL_JTAG: EfuseField = EfuseField::new(0, 1, 43, 1);
/// Represents whether the function that forces chip into download mode is
/// disabled or enabled.\\ 1: disabled\\ 0: enabled\\
pub(crate) const DIS_FORCE_DOWNLOAD: EfuseField = EfuseField::new(0, 1, 44, 1);
/// Represents whether SPI0 controller during boot_mode_download is disabled or
/// enabled.\\ 1: disabled\\ 0: enabled\\
pub(crate) const SPI_DOWNLOAD_MSPI_DIS: EfuseField = EfuseField::new(0, 1, 45, 1);
/// Represents whether TWAI function is disabled or enabled.\\ 1: disabled\\ 0:
/// enabled\\
pub(crate) const DIS_TWAI: EfuseField = EfuseField::new(0, 1, 46, 1);
/// Represents whether the selection between usb_to_jtag and pad_to_jtag through
/// strapping gpio15 when both EFUSE_DIS_PAD_JTAG and EFUSE_DIS_USB_JTAG are
/// equal to 0 is enabled or disabled.\\ 1: enabled\\ 0: disabled\\
pub(crate) const JTAG_SEL_ENABLE: EfuseField = EfuseField::new(0, 1, 47, 1);
/// Represents whether JTAG is disabled in soft way.\\ Odd number: disabled\\
/// Even number: enabled\\
pub(crate) const SOFT_DIS_JTAG: EfuseField = EfuseField::new(0, 1, 48, 3);
/// Represents whether JTAG is disabled in the hard way(permanently).\\ 1:
/// disabled\\ 0: enabled\\
pub(crate) const DIS_PAD_JTAG: EfuseField = EfuseField::new(0, 1, 51, 1);
/// Represents whether flash encrypt function is disabled or enabled(except in
/// SPI boot mode).\\ 1: disabled\\ 0: enabled\\
pub(crate) const DIS_DOWNLOAD_MANUAL_ENCRYPT: EfuseField = EfuseField::new(0, 1, 52, 1);
/// Represents the single-end input threshold vrefh; 1.76 V to 2 V with step of
/// 80 mV
pub(crate) const USB_DREFH: EfuseField = EfuseField::new(0, 1, 53, 2);
/// Represents the single-end input threshold vrefl; 1.76 V to 2 V with step of
/// 80 mV
pub(crate) const USB_DREFL: EfuseField = EfuseField::new(0, 1, 55, 2);
/// Represents whether the D+ and D- pins is exchanged.\\ 1: exchanged\\ 0: not
/// exchanged\\
pub(crate) const USB_EXCHG_PINS: EfuseField = EfuseField::new(0, 1, 57, 1);
/// Represents whether vdd spi pin is functioned as gpio.\\ 1: functioned\\ 0:
/// not functioned\\
pub(crate) const VDD_SPI_AS_GPIO: EfuseField = EfuseField::new(0, 1, 58, 1);
/// Reserved; it was created by set_missed_fields_in_regs func
pub(crate) const RESERVE_0_59: EfuseField = EfuseField::new(0, 1, 59, 5);
/// Represents whether the deploy mode of key manager is disable or not. \\ 1:
/// disabled \\ 0: enabled.\\
pub(crate) const KM_DISABLE_DEPLOY_MODE: EfuseField = EfuseField::new(0, 2, 64, 4);
/// Set the bits to control key manager random number switch cycle. 0: control
/// by register. 1: 8 km clk cycles. 2: 16 km cycles. 3: 32 km cycles
pub(crate) const KM_RND_SWITCH_CYCLE: EfuseField = EfuseField::new(0, 2, 68, 2);
/// Set each bit to control whether corresponding key can only be deployed once.
/// 1 is true; 0 is false. bit 0: ecsda; bit 1: xts; bit2: hmac; bit3: ds
pub(crate) const KM_DEPLOY_ONLY_ONCE: EfuseField = EfuseField::new(0, 2, 70, 4);
/// Set each bit to control whether corresponding key must come from key
/// manager. 1 is true; 0 is false. bit 0: ecsda; bit 1: xts; bit2: hmac; bit3:
/// ds
pub(crate) const FORCE_USE_KEY_MANAGER_KEY: EfuseField = EfuseField::new(0, 2, 74, 4);
/// Set this bit to disable software written init key; and force use
/// efuse_init_key
pub(crate) const FORCE_DISABLE_SW_INIT_KEY: EfuseField = EfuseField::new(0, 2, 78, 1);
/// Reserved; it was created by set_missed_fields_in_regs func
pub(crate) const RESERVE_0_79: EfuseField = EfuseField::new(0, 2, 79, 1);
/// Represents the threshold level of the RTC watchdog STG0 timeout.\\ 0:
/// Original threshold configuration value of STG0 *2 \\1: Original threshold
/// configuration value of STG0 *4 \\2: Original threshold configuration value
/// of STG0 *8 \\3: Original threshold configuration value of STG0 *16 \\
pub(crate) const WDT_DELAY_SEL: EfuseField = EfuseField::new(0, 2, 80, 2);
/// Enables flash encryption when 1 or 3 bits are set and disables otherwise
pub(crate) const SPI_BOOT_CRYPT_CNT: EfuseField = EfuseField::new(0, 2, 82, 3);
/// Revoke 1st secure boot key
pub(crate) const SECURE_BOOT_KEY_REVOKE0: EfuseField = EfuseField::new(0, 2, 85, 1);
/// Revoke 2nd secure boot key
pub(crate) const SECURE_BOOT_KEY_REVOKE1: EfuseField = EfuseField::new(0, 2, 86, 1);
/// Revoke 3rd secure boot key
pub(crate) const SECURE_BOOT_KEY_REVOKE2: EfuseField = EfuseField::new(0, 2, 87, 1);
/// Represents the purpose of Key0
pub(crate) const KEY_PURPOSE_0: EfuseField = EfuseField::new(0, 2, 88, 4);
/// Represents the purpose of Key1
pub(crate) const KEY_PURPOSE_1: EfuseField = EfuseField::new(0, 2, 92, 4);
/// Represents the purpose of Key2
pub(crate) const KEY_PURPOSE_2: EfuseField = EfuseField::new(0, 3, 96, 4);
/// Represents the purpose of Key3
pub(crate) const KEY_PURPOSE_3: EfuseField = EfuseField::new(0, 3, 100, 4);
/// Represents the purpose of Key4
pub(crate) const KEY_PURPOSE_4: EfuseField = EfuseField::new(0, 3, 104, 4);
/// Represents the purpose of Key5
pub(crate) const KEY_PURPOSE_5: EfuseField = EfuseField::new(0, 3, 108, 4);
/// Represents the spa secure level by configuring the clock random divide mode
pub(crate) const SEC_DPA_LEVEL: EfuseField = EfuseField::new(0, 3, 112, 2);
/// Reserved; it was created by set_missed_fields_in_regs func
pub(crate) const RESERVE_0_114: EfuseField = EfuseField::new(0, 3, 114, 2);
/// Represents whether secure boot is enabled or disabled.\\ 1: enabled\\ 0:
/// disabled\\
pub(crate) const SECURE_BOOT_EN: EfuseField = EfuseField::new(0, 3, 116, 1);
/// Represents whether revoking aggressive secure boot is enabled or disabled.\\
/// 1: enabled.\\ 0: disabled\\
pub(crate) const SECURE_BOOT_AGGRESSIVE_REVOKE: EfuseField = EfuseField::new(0, 3, 117, 1);
/// Reserved; it was created by set_missed_fields_in_regs func
pub(crate) const RESERVE_0_118: EfuseField = EfuseField::new(0, 3, 118, 5);
/// Set this bitto configure flash encryption use xts-128 key. else use xts-256
/// key
pub(crate) const KM_XTS_KEY_LENGTH_256: EfuseField = EfuseField::new(0, 3, 123, 1);
/// Represents the flash waiting time after power-up; in unit of ms. When the
/// value less than 15; the waiting time is the programmed value. Otherwise; the
/// waiting time is 2 times the programmed value
pub(crate) const FLASH_TPUW: EfuseField = EfuseField::new(0, 3, 124, 4);
/// Represents whether Download mode is disabled or enabled.\\ 1: disabled\\ 0:
/// enabled\\
pub(crate) const DIS_DOWNLOAD_MODE: EfuseField = EfuseField::new(0, 4, 128, 1);
/// Represents whether direct boot mode is disabled or enabled.\\ 1: disabled\\
/// 0: enabled\\
pub(crate) const DIS_DIRECT_BOOT: EfuseField = EfuseField::new(0, 4, 129, 1);
/// Represents whether print from USB-Serial-JTAG is disabled or enabled.\\ 1:
/// disabled\\ 0: enabled\\
pub(crate) const DIS_USB_SERIAL_JTAG_ROM_PRINT: EfuseField = EfuseField::new(0, 4, 130, 1);
/// Represetns whether to lock the efuse xts key.\\ 1. Lock\\ 0: Unlock\\
pub(crate) const LOCK_KM_KEY: EfuseField = EfuseField::new(0, 4, 131, 1);
/// Represents whether the USB-Serial-JTAG download function is disabled or
/// enabled.\\ 1: Disable\\ 0: Enable\\
pub(crate) const DIS_USB_SERIAL_JTAG_DOWNLOAD_MODE: EfuseField = EfuseField::new(0, 4, 132, 1);
/// Represents whether security download is enabled or disabled.\\ 1: enabled\\
/// 0: disabled\\
pub(crate) const ENABLE_SECURITY_DOWNLOAD: EfuseField = EfuseField::new(0, 4, 133, 1);
/// Set the default UARTboot message output mode
pub(crate) const UART_PRINT_CONTROL: EfuseField = EfuseField::new(0, 4, 134, 2);
/// Represents whether ROM code is forced to send a resume command during SPI
/// boot.\\ 1: forced\\ 0:not forced\\
pub(crate) const FORCE_SEND_RESUME: EfuseField = EfuseField::new(0, 4, 136, 1);
/// Represents the version used by ESP-IDF anti-rollback feature
pub(crate) const SECURE_VERSION: EfuseField = EfuseField::new(0, 4, 137, 16);
/// Represents whether FAST VERIFY ON WAKE is disabled or enabled when Secure
/// Boot is enabled.\\ 1: disabled\\ 0: enabled\\
pub(crate) const SECURE_BOOT_DISABLE_FAST_WAKE: EfuseField = EfuseField::new(0, 4, 153, 1);
/// Represents whether the hysteresis function of corresponding PAD is
/// enabled.\\ 1: enabled\\ 0:disabled\\
pub(crate) const HYS_EN_PAD: EfuseField = EfuseField::new(0, 4, 154, 1);
/// Represents the pseudo round level of xts-aes anti-dpa attack.\\ 3: High.\\
/// 2: Moderate 1. Low\\ 0: Disabled\\
pub(crate) const XTS_DPA_PSEUDO_LEVEL: EfuseField = EfuseField::new(0, 4, 155, 2);
/// Represents whether xts-aes anti-dpa attack clock is enabled.\\ 1. Enable.\\
/// 0: Disable.\\
pub(crate) const XTS_DPA_CLK_ENABLE: EfuseField = EfuseField::new(0, 4, 157, 1);
/// Reserved; it was created by set_missed_fields_in_regs func
pub(crate) const RESERVE_0_158: EfuseField = EfuseField::new(0, 4, 158, 2);
/// Set the bits to control validation of HUK generate mode.\\ Odd of 1 is
/// invalid.\\ Even of 1 is valid.\\
pub(crate) const HUK_GEN_STATE: EfuseField = EfuseField::new(0, 5, 160, 9);
/// Represents whether XTAL frequency is 48MHz or not. If not; 40MHz XTAL will
/// be used. If this field contains Odd number bit 1: Enable 48MHz XTAL\ Even
/// number bit 1: Enable 40MHz XTAL
pub(crate) const XTAL_48M_SEL: EfuseField = EfuseField::new(0, 5, 169, 3);
/// Specify the XTAL frequency selection is decided by eFuse or
/// strapping-PAD-state. 1: eFuse\\ 0: strapping-PAD-state
pub(crate) const XTAL_48M_SEL_MODE: EfuseField = EfuseField::new(0, 5, 172, 1);
/// Represents whether to disable P192 curve in ECDSA.\\ 1: Disabled.\\ 0: Not
/// disable
pub(crate) const ECDSA_DISABLE_P192: EfuseField = EfuseField::new(0, 5, 173, 1);
/// Represents whether to force ecc to use const-time calculation mode. \\ 1:
/// Enable. \\ 0: Disable
pub(crate) const ECC_FORCE_CONST_TIME: EfuseField = EfuseField::new(0, 5, 174, 1);
/// Reserved; it was created by set_missed_fields_in_regs func
pub(crate) const RESERVE_0_175: EfuseField = EfuseField::new(0, 5, 175, 17);
/// MAC address
pub(crate) const MAC0: EfuseField = EfuseField::new(1, 0, 0, 32);
/// MAC address
pub(crate) const MAC1: EfuseField = EfuseField::new(1, 1, 32, 16);
/// Represents the extended bits of MAC address
pub(crate) const MAC_EXT: EfuseField = EfuseField::new(1, 1, 48, 16);
/// Minor chip version
pub(crate) const WAFER_VERSION_MINOR: EfuseField = EfuseField::new(1, 2, 64, 4);
/// Minor chip version
pub(crate) const WAFER_VERSION_MAJOR: EfuseField = EfuseField::new(1, 2, 68, 2);
/// Disables check of wafer version major
pub(crate) const DISABLE_WAFER_VERSION_MAJOR: EfuseField = EfuseField::new(1, 2, 70, 1);
/// Disables check of blk version major
pub(crate) const DISABLE_BLK_VERSION_MAJOR: EfuseField = EfuseField::new(1, 2, 71, 1);
/// BLK_VERSION_MINOR of BLOCK2
pub(crate) const BLK_VERSION_MINOR: EfuseField = EfuseField::new(1, 2, 72, 3);
/// BLK_VERSION_MAJOR of BLOCK2
pub(crate) const BLK_VERSION_MAJOR: EfuseField = EfuseField::new(1, 2, 75, 2);
/// Flash capacity
pub(crate) const FLASH_CAP: EfuseField = EfuseField::new(1, 2, 77, 3);
/// Flash vendor
pub(crate) const FLASH_VENDOR: EfuseField = EfuseField::new(1, 2, 80, 3);
/// Psram capacity
pub(crate) const PSRAM_CAP: EfuseField = EfuseField::new(1, 2, 83, 3);
/// Psram vendor
pub(crate) const PSRAM_VENDOR: EfuseField = EfuseField::new(1, 2, 86, 2);
/// Temp (die embedded inside)
pub(crate) const TEMP: EfuseField = EfuseField::new(1, 2, 88, 2);
/// Package version
pub(crate) const PKG_VERSION: EfuseField = EfuseField::new(1, 2, 90, 3);
/// PADC CAL PA trim version
pub(crate) const PA_TRIM_VERSION: EfuseField = EfuseField::new(1, 2, 93, 3);
/// PADC CAL N bias
pub(crate) const TRIM_N_BIAS: EfuseField = EfuseField::new(1, 3, 96, 5);
/// PADC CAL P bias
pub(crate) const TRIM_P_BIAS: EfuseField = EfuseField::new(1, 3, 101, 5);
/// Active HP DBIAS of fixed voltage
pub(crate) const ACTIVE_HP_DBIAS: EfuseField = EfuseField::new(1, 3, 106, 4);
/// Active LP DBIAS of fixed voltage
pub(crate) const ACTIVE_LP_DBIAS: EfuseField = EfuseField::new(1, 3, 110, 4);
/// LSLP HP DBG of fixed voltage
pub(crate) const LSLP_HP_DBG: EfuseField = EfuseField::new(1, 3, 114, 2);
/// LSLP HP DBIAS of fixed voltage
pub(crate) const LSLP_HP_DBIAS: EfuseField = EfuseField::new(1, 3, 116, 4);
/// DSLP LP DBG of fixed voltage
pub(crate) const DSLP_LP_DBG: EfuseField = EfuseField::new(1, 3, 120, 4);
/// DSLP LP DBIAS of fixed voltage
pub(crate) const DSLP_LP_DBIAS: EfuseField = EfuseField::new(1, 3, 124, 5);
/// DBIAS gap between LP and HP
pub(crate) const LP_HP_DBIAS_VOL_GAP: EfuseField = EfuseField::new(1, 4, 129, 5);
/// reserved
pub(crate) const RESERVED_1_134: EfuseField = EfuseField::new(1, 4, 134, 26);
/// Represents the second 32-bit of zeroth part of system data
pub(crate) const SYS_DATA_PART0_2: EfuseField = EfuseField::new(1, 5, 160, 32);
/// Optional unique 128-bit ID
pub(crate) const OPTIONAL_UNIQUE_ID: EfuseField = EfuseField::new(2, 0, 0, 128);
/// Temperature calibration data
pub(crate) const TEMPERATURE_SENSOR: EfuseField = EfuseField::new(2, 4, 128, 9);
/// ADC OCode
pub(crate) const OCODE: EfuseField = EfuseField::new(2, 4, 137, 8);
/// Average initcode of ADC1 atten0
pub(crate) const ADC1_AVE_INITCODE_ATTEN0: EfuseField = EfuseField::new(2, 4, 145, 10);
/// Average initcode of ADC1 atten0
pub(crate) const ADC1_AVE_INITCODE_ATTEN1: EfuseField = EfuseField::new(2, 4, 155, 10);
/// Average initcode of ADC1 atten0
pub(crate) const ADC1_AVE_INITCODE_ATTEN2: EfuseField = EfuseField::new(2, 5, 165, 10);
/// Average initcode of ADC1 atten0
pub(crate) const ADC1_AVE_INITCODE_ATTEN3: EfuseField = EfuseField::new(2, 5, 175, 10);
/// HI DOUT of ADC1 atten0
pub(crate) const ADC1_HI_DOUT_ATTEN0: EfuseField = EfuseField::new(2, 5, 185, 10);
/// HI DOUT of ADC1 atten1
pub(crate) const ADC1_HI_DOUT_ATTEN1: EfuseField = EfuseField::new(2, 6, 195, 10);
/// HI DOUT of ADC1 atten2
pub(crate) const ADC1_HI_DOUT_ATTEN2: EfuseField = EfuseField::new(2, 6, 205, 10);
/// HI DOUT of ADC1 atten3
pub(crate) const ADC1_HI_DOUT_ATTEN3: EfuseField = EfuseField::new(2, 6, 215, 10);
/// Gap between ADC1 CH0 and average initcode
pub(crate) const ADC1_CH0_ATTEN0_INITCODE_DIFF: EfuseField = EfuseField::new(2, 7, 225, 4);
/// Gap between ADC1 CH1 and average initcode
pub(crate) const ADC1_CH1_ATTEN0_INITCODE_DIFF: EfuseField = EfuseField::new(2, 7, 229, 4);
/// Gap between ADC1 CH2 and average initcode
pub(crate) const ADC1_CH2_ATTEN0_INITCODE_DIFF: EfuseField = EfuseField::new(2, 7, 233, 4);
/// Gap between ADC1 CH3 and average initcode
pub(crate) const ADC1_CH3_ATTEN0_INITCODE_DIFF: EfuseField = EfuseField::new(2, 7, 237, 4);
/// Gap between ADC1 CH4 and average initcode
pub(crate) const ADC1_CH4_ATTEN0_INITCODE_DIFF: EfuseField = EfuseField::new(2, 7, 241, 4);
/// Gap between ADC1 CH5 and average initcode
pub(crate) const ADC1_CH5_ATTEN0_INITCODE_DIFF: EfuseField = EfuseField::new(2, 7, 245, 4);
/// reserved
pub(crate) const RESERVED_2_249: EfuseField = EfuseField::new(2, 7, 249, 7);
/// User data
pub(crate) const BLOCK_USR_DATA: EfuseField = EfuseField::new(3, 0, 0, 192);
/// reserved
pub(crate) const RESERVED_3_192: EfuseField = EfuseField::new(3, 6, 192, 8);
/// Custom MAC
pub(crate) const CUSTOM_MAC: EfuseField = EfuseField::new(3, 6, 200, 48);
/// reserved
pub(crate) const RESERVED_3_248: EfuseField = EfuseField::new(3, 7, 248, 8);
/// Key0 or user data
pub(crate) const BLOCK_KEY0: EfuseField = EfuseField::new(4, 0, 0, 256);
/// Key1 or user data
pub(crate) const BLOCK_KEY1: EfuseField = EfuseField::new(5, 0, 0, 256);
/// Key2 or user data
pub(crate) const BLOCK_KEY2: EfuseField = EfuseField::new(6, 0, 0, 256);
/// Key3 or user data
pub(crate) const BLOCK_KEY3: EfuseField = EfuseField::new(7, 0, 0, 256);
/// Key4 or user data
pub(crate) const BLOCK_KEY4: EfuseField = EfuseField::new(8, 0, 0, 256);
/// Key5 or user data
pub(crate) const BLOCK_KEY5: EfuseField = EfuseField::new(9, 0, 0, 256);
/// System data part 2 (reserved)
pub(crate) const BLOCK_SYS_DATA2: EfuseField = EfuseField::new(10, 0, 0, 256);
