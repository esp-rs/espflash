use crate::PartitionTable;

#[allow(clippy::module_inception)]
mod esp32;
mod esp32c3;
mod esp32s2;

pub use esp32::Esp32;
pub use esp32c3::Esp32c3;
pub use esp32s2::Esp32s2;

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
    pub fn default_partition_table(&self) -> PartitionTable {
        PartitionTable::basic(
            self.nvs_addr,
            self.nvs_size,
            self.phy_init_data_addr,
            self.phy_init_data_size,
            self.app_addr,
            self.app_size,
        )
    }
}
