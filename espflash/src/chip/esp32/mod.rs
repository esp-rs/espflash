pub use self::{
    esp32::Esp32, esp32c2::Esp32c2, esp32c3::Esp32c3, esp32s2::Esp32s2, esp32s3::Esp32s3,
};
use crate::PartitionTable;

#[allow(clippy::module_inception)]
mod esp32;
mod esp32c2;
mod esp32c3;
mod esp32s2;
mod esp32s3;

#[derive(Clone, Copy, Debug)]
pub struct Esp32Params {
    pub boot_addr: u32,
    pub partition_addr: u32,
    pub nvs_addr: u32,
    pub nvs_size: u32,
    pub phy_init_data_addr: u32,
    pub phy_init_data_size: u32,
    pub app_addr: u32,
    pub app_size: u32,
    pub chip_id: u16,
    pub default_bootloader: &'static [u8],
}

impl Esp32Params {
    pub const fn new(
        boot_addr: u32,
        app_addr: u32,
        app_size: u32,
        chip_id: u16,
        bootloader: &'static [u8],
    ) -> Self {
        Self {
            boot_addr,
            partition_addr: 0x8000,
            nvs_addr: 0x9000,
            nvs_size: 0x6000,
            phy_init_data_addr: 0xf000,
            phy_init_data_size: 0x1000,
            app_addr,
            app_size,
            chip_id,
            default_bootloader: bootloader,
        }
    }

    /// Generates a default partition table.
    /// `flash_size` is used to scale app partition when present, otherwise the
    /// param defaults are used.
    pub fn default_partition_table(&self, flash_size: Option<u32>) -> PartitionTable {
        PartitionTable::basic(
            self.nvs_addr,
            self.nvs_size,
            self.phy_init_data_addr,
            self.phy_init_data_size,
            self.app_addr,
            flash_size.map_or(self.app_size, |size| size - self.app_addr),
        )
    }
}
