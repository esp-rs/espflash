//! eFuse field definitions for the esp32c5
//!
//! This file was automatically generated, please do not edit it manually!
//!
//! Generated: 2025-06-25 11:06
//! Version:   31c7fe3f5f4e0a55b178a57126c0aca7

#![allow(unused)]

use super::EfuseField;

/// Total size in bytes of each block
pub(crate) const BLOCK_SIZES: &[u32] = &[24, 24, 32, 32, 32, 32, 32, 32, 32, 32, 32];

/// Disable programming of individual eFuses
pub const WR_DIS: EfuseField = EfuseField::new(0, 0, 0, 32);
/// Disable reading from BlOCK4-10
pub const RD_DIS: EfuseField = EfuseField::new(0, 1, 32, 7);
/// Represents the anti-rollback secure version of the 2nd stage bootloader used
/// by the ROM bootloader (the high part of the field)
pub const BOOTLOADER_ANTI_ROLLBACK_SECURE_VERSION_HI: EfuseField = EfuseField::new(0, 1, 39, 1);
/// Represents whether cache is disabled. 1: Disabled 0: Enabled
pub const DIS_ICACHE: EfuseField = EfuseField::new(0, 1, 40, 1);
/// Represents whether the USB-to-JTAG function in USB Serial/JTAG is disabled.
/// Note that \hyperref[fielddesc:EFUSEDISUSBJTAG]{EFUSE\_DIS\_USB\_JTAG} is
/// available only when
/// \hyperref[fielddesc:EFUSEDISUSBSERIALJTAG]{EFUSE\_DIS\_USB\_SERIAL\_JTAG} is
/// configured to 0. For more information; please refer to Chapter
/// \ref{mod:bootctrl} \textit{\nameref{mod:bootctrl}}.1: Disabled0: Enabled
pub const DIS_USB_JTAG: EfuseField = EfuseField::new(0, 1, 41, 1);
/// Represents whether the ani-rollback check for the 2nd stage bootloader is
/// enabled.1: Enabled0: Disabled
pub const BOOTLOADER_ANTI_ROLLBACK_EN: EfuseField = EfuseField::new(0, 1, 42, 1);
/// Represents whether USB Serial/JTAG is disabled.1: Disabled0: Enabled
pub const DIS_USB_SERIAL_JTAG: EfuseField = EfuseField::new(0, 1, 43, 1);
/// Represents whether the function that forces chip into Download mode is
/// disabled. 1: Disabled0: Enabled
pub const DIS_FORCE_DOWNLOAD: EfuseField = EfuseField::new(0, 1, 44, 1);
/// Represents whether SPI0 controller during boot\_mode\_download is
/// disabled.0: Enabled1: Disabled
pub const SPI_DOWNLOAD_MSPI_DIS: EfuseField = EfuseField::new(0, 1, 45, 1);
/// Represents whether TWAI$^®$ function is disabled.1: Disabled0: Enabled
pub const DIS_TWAI: EfuseField = EfuseField::new(0, 1, 46, 1);
/// Represents whether the selection of a JTAG signal source through the
/// strapping pin value is enabled when all of
/// \hyperref[fielddesc:EFUSEDISPADJTAG]{EFUSE\_DIS\_PAD\_JTAG};
/// \hyperref[fielddesc:EFUSEDISUSBJTAG]{EFUSE\_DIS\_USB\_JTAG} and
/// \hyperref[fielddesc:EFUSEDISUSBSERIALJTAG]{EFUSE\_DIS\_USB\_SERIAL\_JTAG}
/// are configured to 0. For more information; please refer to Chapter
/// \ref{mod:bootctrl} \textit{\nameref{mod:bootctrl}}.1: Enabled0: Disabled
pub const JTAG_SEL_ENABLE: EfuseField = EfuseField::new(0, 1, 47, 1);
/// Represents whether PAD JTAG is disabled in the soft way. It can be restarted
/// via HMAC. Odd count of bits with a value of 1: DisabledEven count of bits
/// with a value of 1: Enabled
pub const SOFT_DIS_JTAG: EfuseField = EfuseField::new(0, 1, 48, 3);
/// Represents whether PAD JTAG is disabled in the hard way (permanently).1:
/// Disabled0: Enabled
pub const DIS_PAD_JTAG: EfuseField = EfuseField::new(0, 1, 51, 1);
/// Represents whether flash encryption is disabled (except in SPI boot mode).1:
/// Disabled0: Enabled
pub const DIS_DOWNLOAD_MANUAL_ENCRYPT: EfuseField = EfuseField::new(0, 1, 52, 1);
/// Represents the single-end input threshold vrefh; 1.76 V to 2 V with step of
/// 80 mV
pub const USB_DREFH: EfuseField = EfuseField::new(0, 1, 53, 2);
/// Represents the single-end input threshold vrefl; 1.76 V to 2 V with step of
/// 80 mV
pub const USB_DREFL: EfuseField = EfuseField::new(0, 1, 55, 2);
/// Represents whether the D+ and D- pins is exchanged.1: Exchanged0: Not
/// exchanged
pub const USB_EXCHG_PINS: EfuseField = EfuseField::new(0, 1, 57, 1);
/// Represents whether VDD SPI pin is functioned as GPIO.1: Functioned0: Not
/// functioned
pub const VDD_SPI_AS_GPIO: EfuseField = EfuseField::new(0, 1, 58, 1);
/// Represents RTC watchdog timeout threshold.0: The originally configured STG0
/// threshold × 21: The originally configured STG0 threshold × 42: The
/// originally configured STG0 threshold × 83: The originally configured STG0
/// threshold × 16
pub const WDT_DELAY_SEL: EfuseField = EfuseField::new(0, 1, 59, 2);
/// Represents the anti-rollback secure version of the 2nd stage bootloader used
/// by the ROM bootloader (the low part of the field)
pub const BOOTLOADER_ANTI_ROLLBACK_SECURE_VERSION_LO: EfuseField = EfuseField::new(0, 1, 61, 3);
/// Represents whether the new key deployment of key manager is disabled. Bit0:
/// Represents whether the new ECDSA key deployment is disabled0: Enabled1:
/// DisabledBit1: Represents whether the new XTS-AES (flash and PSRAM) key
/// deployment is disabled0: Enabled1: DisabledBit2: Represents whether the new
/// HMAC key deployment is disabled0: Enabled1: DisabledBit3: Represents whether
/// the new DS key deployment is disabled0: Enabled1: Disabled
pub const KM_DISABLE_DEPLOY_MODE: EfuseField = EfuseField::new(0, 2, 64, 4);
/// Represents the cycle at which the Key Manager switches random numbers.0:
/// Controlled by the
/// \hyperref[fielddesc:KEYMNGRNDSWITCHCYCLE]{KEYMNG\_RND\_SWITCH\_CYCLE}
/// register. For more information; please refer to Chapter \ref{mod:keymng}
/// \textit{\nameref{mod:keymng}}1: 8 Key Manager clock cycles2: 16 Key Manager
/// clock cycles3: 32 Key Manager clock cycles
pub const KM_RND_SWITCH_CYCLE: EfuseField = EfuseField::new(0, 2, 68, 2);
/// Represents whether the corresponding key can be deployed only once.Bit0:
/// Represents whether the ECDSA key can be deployed only once0: The key can be
/// deployed multiple times1: The key can be deployed only onceBit1: Represents
/// whether the XTS-AES (flash and PSRAM) key can be deployed only once0: The
/// key can be deployed multiple times1: The key can be deployed only onceBit2:
/// Represents whether the HMAC key can be deployed only once0: The key can be
/// deployed multiple times1: The key can be deployed only onceBit3: Represents
/// whether the DS key can be deployed only once0: The key can be deployed
/// multiple times1: The key can be deployed only once
pub const KM_DEPLOY_ONLY_ONCE: EfuseField = EfuseField::new(0, 2, 70, 4);
/// Represents whether the corresponding key must come from Key Manager. Bit0:
/// Represents whether the ECDSA key must come from Key Manager.0: The key does
/// not need to come from Key Manager1: The key must come from Key ManagerBit1:
/// Represents whether the XTS-AES (flash and PSRAM) key must come from Key
/// Manager.0: The key does not need to come from Key Manager1: The key must
/// come from Key ManagerBit2: Represents whether the HMAC key must come from
/// Key Manager.0: The key does not need to come from Key Manager1: The key must
/// come from Key ManagerBit3: Represents whether the DS key must come from Key
/// Manager.0: The key does not need to come from Key Manager1: The key must
/// come from Key Manager
pub const FORCE_USE_KEY_MANAGER_KEY: EfuseField = EfuseField::new(0, 2, 74, 4);
/// Represents whether to disable the use of the initialization key written by
/// software and instead force use efuse\_init\_key.0: Enable1: Disable
pub const FORCE_DISABLE_SW_INIT_KEY: EfuseField = EfuseField::new(0, 2, 78, 1);
/// Represents whether the ani-rollback SECURE_VERSION will be updated from the
/// ROM bootloader.1: Enable0: Disable
pub const BOOTLOADER_ANTI_ROLLBACK_UPDATE_IN_ROM: EfuseField = EfuseField::new(0, 2, 79, 1);
/// Enables flash encryption when 1 or 3 bits are set and disables otherwise
pub const SPI_BOOT_CRYPT_CNT: EfuseField = EfuseField::new(0, 2, 80, 3);
/// Revoke 1st secure boot key
pub const SECURE_BOOT_KEY_REVOKE0: EfuseField = EfuseField::new(0, 2, 83, 1);
/// Revoke 2nd secure boot key
pub const SECURE_BOOT_KEY_REVOKE1: EfuseField = EfuseField::new(0, 2, 84, 1);
/// Revoke 3rd secure boot key
pub const SECURE_BOOT_KEY_REVOKE2: EfuseField = EfuseField::new(0, 2, 85, 1);
/// Represents the purpose of Key0. See Table \ref{tab:efuse-key-purpose}
pub const KEY_PURPOSE_0: EfuseField = EfuseField::new(0, 2, 86, 5);
/// Represents the purpose of Key1. See Table \ref{tab:efuse-key-purpose}
pub const KEY_PURPOSE_1: EfuseField = EfuseField::new(0, 2, 91, 5);
/// Represents the purpose of Key2. See Table \ref{tab:efuse-key-purpose}
pub const KEY_PURPOSE_2: EfuseField = EfuseField::new(0, 3, 96, 5);
/// Represents the purpose of Key3. See Table \ref{tab:efuse-key-purpose}
pub const KEY_PURPOSE_3: EfuseField = EfuseField::new(0, 3, 101, 5);
/// Represents the purpose of Key4. See Table \ref{tab:efuse-key-purpose}
pub const KEY_PURPOSE_4: EfuseField = EfuseField::new(0, 3, 106, 5);
/// Represents the purpose of Key5. See Table \ref{tab:efuse-key-purpose}
pub const KEY_PURPOSE_5: EfuseField = EfuseField::new(0, 3, 111, 5);
/// Represents the security level of anti-DPA attack. The level is adjusted by
/// configuring the clock random frequency division mode.0: Security level is
/// SEC\_DPA\_OFF1: Security level is SEC\_DPA\_LOW2: Security level is
/// SEC\_DPA\_MIDDLE3: Security level is SEC\_DPA\_HIGHFor more information;
/// please refer to Chapter \ref{mod:sysreg} \textit{\nameref{mod:sysreg}} >
/// Section \ref{sec:sysreg-anti-dpa-attack-security-control}
/// \textit{\nameref{sec:sysreg-anti-dpa-attack-security-control}}.
pub const SEC_DPA_LEVEL: EfuseField = EfuseField::new(0, 3, 116, 2);
/// Represents the starting flash sector (flash sector size is 0x1000) of the
/// recovery bootloader used by the ROM bootloader If the primary bootloader
/// fails. 0 and 0xFFF - this feature is disabled. (The high part of the field)
pub const RECOVERY_BOOTLOADER_FLASH_SECTOR_HI: EfuseField = EfuseField::new(0, 3, 118, 3);
/// Represents whether Secure Boot is enabled.1: Enabled0: Disabled
pub const SECURE_BOOT_EN: EfuseField = EfuseField::new(0, 3, 121, 1);
/// Represents whether aggressive revocation of Secure Boot is enabled.1:
/// Enabled0: Disabled
pub const SECURE_BOOT_AGGRESSIVE_REVOKE: EfuseField = EfuseField::new(0, 3, 122, 1);
/// Represents which key flash encryption uses.0: XTS-AES-256 key1: XTS-AES-128
/// key
pub const KM_XTS_KEY_LENGTH_256: EfuseField = EfuseField::new(0, 3, 123, 1);
/// Represents the flash waiting time after power-up. Measurement unit: ms. When
/// the value is less than 15; the waiting time is the programmed value.
/// Otherwise; the waiting time is a fixed value; i.e. 30 ms
pub const FLASH_TPUW: EfuseField = EfuseField::new(0, 3, 124, 4);
/// Represents whether Download mode is disable or enable. 1. Disable 0: Enable
pub const DIS_DOWNLOAD_MODE: EfuseField = EfuseField::new(0, 4, 128, 1);
/// Represents whether direct boot mode is disabled or enabled. 1. Disable 0:
/// Enable
pub const DIS_DIRECT_BOOT: EfuseField = EfuseField::new(0, 4, 129, 1);
/// Represents whether print from USB-Serial-JTAG is disabled or enabled. 1.
/// Disable 0: Enable
pub const DIS_USB_SERIAL_JTAG_ROM_PRINT: EfuseField = EfuseField::new(0, 4, 130, 1);
/// Represents whether the keys in the Key Manager are locked after
/// deployment.0: Not locked1: Locked
pub const LOCK_KM_KEY: EfuseField = EfuseField::new(0, 4, 131, 1);
/// Represents whether the USB-Serial-JTAG download function is disabled or
/// enabled. 1: Disable 0: Enable
pub const DIS_USB_SERIAL_JTAG_DOWNLOAD_MODE: EfuseField = EfuseField::new(0, 4, 132, 1);
/// Represents whether security download is enabled. Only downloading into flash
/// is supported. Reading/writing RAM or registers is not supported (i.e. stub
/// download is not supported).1: Enabled0: Disabled
pub const ENABLE_SECURITY_DOWNLOAD: EfuseField = EfuseField::new(0, 4, 133, 1);
/// Set the default UARTboot message output mode
pub const UART_PRINT_CONTROL: EfuseField = EfuseField::new(0, 4, 134, 2);
/// Represents whether ROM code is forced to send a resume command during SPI
/// boot.1: Forced. 0: Not forced.
pub const FORCE_SEND_RESUME: EfuseField = EfuseField::new(0, 4, 136, 1);
/// Represents the app secure version used by ESP-IDF anti-rollback feature
pub const SECURE_VERSION: EfuseField = EfuseField::new(0, 4, 137, 9);
/// Reserved; it was created by set_missed_fields_in_regs func
pub const RESERVE_0_146: EfuseField = EfuseField::new(0, 4, 146, 7);
/// Represents whether FAST VERIFY ON WAKE is disabled when Secure Boot is
/// enabled.1: Disabled0: Enabled
pub const SECURE_BOOT_DISABLE_FAST_WAKE: EfuseField = EfuseField::new(0, 4, 153, 1);
/// Represents whether the hysteresis function of PAD0 – PAD27 is enabled.1:
/// Enabled0: Disabled
pub const HYS_EN_PAD: EfuseField = EfuseField::new(0, 4, 154, 1);
/// Represents the pseudo round level of XTS-AES anti-DPA attack.0: Disabled1:
/// Low2: Moderate3: High
pub const XTS_DPA_PSEUDO_LEVEL: EfuseField = EfuseField::new(0, 4, 155, 2);
/// Represents whether XTS-AES anti-DPA attack clock is enabled.0: Disable1:
/// Enabled
pub const XTS_DPA_CLK_ENABLE: EfuseField = EfuseField::new(0, 4, 157, 1);
/// Reserved; it was created by set_missed_fields_in_regs func
pub const RESERVE_0_158: EfuseField = EfuseField::new(0, 4, 158, 1);
/// Represents if the chip supports Secure Boot using SHA-384
pub const SECURE_BOOT_SHA384_EN: EfuseField = EfuseField::new(0, 4, 159, 1);
/// Represents whether the HUK generate mode is valid.Odd count of bits with a
/// value of 1: InvalidEven count of bits with a value of 1: Valid
pub const HUK_GEN_STATE: EfuseField = EfuseField::new(0, 5, 160, 9);
/// Represents whether XTAL frequency is 48MHz or not. If not; 40MHz XTAL will
/// be used. If this field contains Odd number bit 1: Enable 48MHz XTAL\ Even
/// number bit 1: Enable 40MHz XTAL
pub const XTAL_48M_SEL: EfuseField = EfuseField::new(0, 5, 169, 3);
/// Represents what determines the XTAL frequency in \textbf{Joint Download
/// Boot} mode.  For more information; please refer to Chapter
/// \ref{mod:bootctrl} \textit{\nameref{mod:bootctrl}}.0: Strapping PAD state1:
/// \hyperref[fielddesc:EFUSEXTAL48MSEL]{EFUSE\_XTAL\_48M\_SEL} in eFuse
pub const XTAL_48M_SEL_MODE: EfuseField = EfuseField::new(0, 5, 172, 1);
/// Represents whether to force ECC to use constant-time mode for point
/// multiplication calculation. 0: Not force1: Force
pub const ECC_FORCE_CONST_TIME: EfuseField = EfuseField::new(0, 5, 173, 1);
/// Represents the starting flash sector (flash sector size is 0x1000) of the
/// recovery bootloader used by the ROM bootloader If the primary bootloader
/// fails. 0 and 0xFFF - this feature is disabled. (The low part of the field)
pub const RECOVERY_BOOTLOADER_FLASH_SECTOR_LO: EfuseField = EfuseField::new(0, 5, 174, 9);
/// Reserved; it was created by set_missed_fields_in_regs func
pub const RESERVE_0_183: EfuseField = EfuseField::new(0, 5, 183, 9);
/// MAC address
pub const MAC0: EfuseField = EfuseField::new(1, 0, 0, 32);
/// MAC address
pub const MAC1: EfuseField = EfuseField::new(1, 1, 32, 16);
/// Represents the extended bits of MAC address
pub const MAC_EXT: EfuseField = EfuseField::new(1, 1, 48, 16);
/// Minor chip version
pub const WAFER_VERSION_MINOR: EfuseField = EfuseField::new(1, 2, 64, 4);
/// Minor chip version
pub const WAFER_VERSION_MAJOR: EfuseField = EfuseField::new(1, 2, 68, 2);
/// Disables check of wafer version major
pub const DISABLE_WAFER_VERSION_MAJOR: EfuseField = EfuseField::new(1, 2, 70, 1);
/// Disables check of blk version major
pub const DISABLE_BLK_VERSION_MAJOR: EfuseField = EfuseField::new(1, 2, 71, 1);
/// BLK_VERSION_MINOR of BLOCK2
pub const BLK_VERSION_MINOR: EfuseField = EfuseField::new(1, 2, 72, 3);
/// BLK_VERSION_MAJOR of BLOCK2
pub const BLK_VERSION_MAJOR: EfuseField = EfuseField::new(1, 2, 75, 2);
/// Flash capacity
pub const FLASH_CAP: EfuseField = EfuseField::new(1, 2, 77, 3);
/// Flash vendor
pub const FLASH_VENDOR: EfuseField = EfuseField::new(1, 2, 80, 3);
/// Psram capacity
pub const PSRAM_CAP: EfuseField = EfuseField::new(1, 2, 83, 3);
/// Psram vendor
pub const PSRAM_VENDOR: EfuseField = EfuseField::new(1, 2, 86, 2);
/// Temp (die embedded inside)
pub const TEMP: EfuseField = EfuseField::new(1, 2, 88, 2);
/// Package version
pub const PKG_VERSION: EfuseField = EfuseField::new(1, 2, 90, 3);
/// PADC CAL PA trim version
pub const PA_TRIM_VERSION: EfuseField = EfuseField::new(1, 2, 93, 3);
/// PADC CAL N bias
pub const TRIM_N_BIAS: EfuseField = EfuseField::new(1, 3, 96, 5);
/// PADC CAL P bias
pub const TRIM_P_BIAS: EfuseField = EfuseField::new(1, 3, 101, 5);
/// Active HP DBIAS of fixed voltage
pub const ACTIVE_HP_DBIAS: EfuseField = EfuseField::new(1, 3, 106, 4);
/// Active LP DBIAS of fixed voltage
pub const ACTIVE_LP_DBIAS: EfuseField = EfuseField::new(1, 3, 110, 4);
/// LSLP HP DBG of fixed voltage
pub const LSLP_HP_DBG: EfuseField = EfuseField::new(1, 3, 114, 2);
/// LSLP HP DBIAS of fixed voltage
pub const LSLP_HP_DBIAS: EfuseField = EfuseField::new(1, 3, 116, 4);
/// DSLP LP DBG of fixed voltage
pub const DSLP_LP_DBG: EfuseField = EfuseField::new(1, 3, 120, 4);
/// DSLP LP DBIAS of fixed voltage
pub const DSLP_LP_DBIAS: EfuseField = EfuseField::new(1, 3, 124, 5);
/// DBIAS gap between LP and HP
pub const LP_HP_DBIAS_VOL_GAP: EfuseField = EfuseField::new(1, 4, 129, 5);
/// REF PADC Calibration Curr
pub const REF_CURR_CODE: EfuseField = EfuseField::new(1, 4, 134, 4);
/// RES PADC Calibration Tune
pub const RES_TUNE_CODE: EfuseField = EfuseField::new(1, 4, 138, 5);
/// reserved
pub const RESERVED_1_143: EfuseField = EfuseField::new(1, 4, 143, 17);
/// Represents the third 32-bit of zeroth part of system data
pub const SYS_DATA_PART0_2: EfuseField = EfuseField::new(1, 5, 160, 32);
/// Optional unique 128-bit ID
pub const OPTIONAL_UNIQUE_ID: EfuseField = EfuseField::new(2, 0, 0, 128);
/// Temperature calibration data
pub const TEMPERATURE_SENSOR: EfuseField = EfuseField::new(2, 4, 128, 9);
/// ADC OCode
pub const OCODE: EfuseField = EfuseField::new(2, 4, 137, 8);
/// Average initcode of ADC1 atten0
pub const ADC1_AVE_INITCODE_ATTEN0: EfuseField = EfuseField::new(2, 4, 145, 10);
/// Average initcode of ADC1 atten0
pub const ADC1_AVE_INITCODE_ATTEN1: EfuseField = EfuseField::new(2, 4, 155, 10);
/// Average initcode of ADC1 atten0
pub const ADC1_AVE_INITCODE_ATTEN2: EfuseField = EfuseField::new(2, 5, 165, 10);
/// Average initcode of ADC1 atten0
pub const ADC1_AVE_INITCODE_ATTEN3: EfuseField = EfuseField::new(2, 5, 175, 10);
/// HI DOUT of ADC1 atten0
pub const ADC1_HI_DOUT_ATTEN0: EfuseField = EfuseField::new(2, 5, 185, 10);
/// HI DOUT of ADC1 atten1
pub const ADC1_HI_DOUT_ATTEN1: EfuseField = EfuseField::new(2, 6, 195, 10);
/// HI DOUT of ADC1 atten2
pub const ADC1_HI_DOUT_ATTEN2: EfuseField = EfuseField::new(2, 6, 205, 10);
/// HI DOUT of ADC1 atten3
pub const ADC1_HI_DOUT_ATTEN3: EfuseField = EfuseField::new(2, 6, 215, 10);
/// Gap between ADC1 CH0 and average initcode
pub const ADC1_CH0_ATTEN0_INITCODE_DIFF: EfuseField = EfuseField::new(2, 7, 225, 4);
/// Gap between ADC1 CH1 and average initcode
pub const ADC1_CH1_ATTEN0_INITCODE_DIFF: EfuseField = EfuseField::new(2, 7, 229, 4);
/// Gap between ADC1 CH2 and average initcode
pub const ADC1_CH2_ATTEN0_INITCODE_DIFF: EfuseField = EfuseField::new(2, 7, 233, 4);
/// Gap between ADC1 CH3 and average initcode
pub const ADC1_CH3_ATTEN0_INITCODE_DIFF: EfuseField = EfuseField::new(2, 7, 237, 4);
/// Gap between ADC1 CH4 and average initcode
pub const ADC1_CH4_ATTEN0_INITCODE_DIFF: EfuseField = EfuseField::new(2, 7, 241, 4);
/// Gap between ADC1 CH5 and average initcode
pub const ADC1_CH5_ATTEN0_INITCODE_DIFF: EfuseField = EfuseField::new(2, 7, 245, 4);
/// reserved
pub const RESERVED_2_249: EfuseField = EfuseField::new(2, 7, 249, 7);
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
