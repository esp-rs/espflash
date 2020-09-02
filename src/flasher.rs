use crate::chip::{Chip, ESP8266};
use crate::connection::Connection;
use crate::elf::FirmwareImage;
use crate::encoder::SlipEncoder;
use crate::error::RomError;
use crate::Error;
use bytemuck::{bytes_of, Pod, Zeroable};
use serial::SerialPort;
use std::mem::size_of;

type Encoder<'a> = SlipEncoder<'a, Box<dyn SerialPort>>;

#[derive(Copy, Clone)]
#[repr(u64)]
enum Timeouts {
    Default = 3000,
    Sync = 100,
}

const MAX_RAM_BLOCK_SIZE: usize = 0x1800;
const FLASH_SECTOR_SIZE: usize = 0x1000;
const FLASH_BLOCK_SIZE: usize = 0x100;
const FLASH_SECTORS_PER_BLOCK: usize = FLASH_SECTOR_SIZE / FLASH_BLOCK_SIZE;
const FLASH_WRITE_SIZE: usize = 0x400;

#[derive(Copy, Clone, Debug)]
#[repr(u8)]
#[allow(dead_code)]
enum Command {
    FlashBegin = 0x02,
    FlashData = 0x03,
    FlashEnd = 0x04,
    MemBegin = 0x05,
    MemEnd = 0x06,
    MemData = 0x07,
    Sync = 0x08,
    WriteReg = 0x09,
    ReadReg = 0x0a,
}

#[derive(Zeroable, Pod, Copy, Clone, Debug)]
#[repr(C)]
struct BlockParams {
    size: u32,
    sequence: u32,
    dummy1: u32,
    dummy2: u32,
}

#[derive(Zeroable, Pod, Copy, Clone, Debug)]
#[repr(C)]
struct BeginParams {
    size: u32,
    blocks: u32,
    block_size: u32,
    offset: u32,
}

#[derive(Zeroable, Pod, Copy, Clone)]
#[repr(C)]
struct EntryParams {
    no_entry: u32,
    entry: u32,
}

pub struct Flasher {
    connection: Connection,
}

impl Flasher {
    pub fn connect(serial: impl SerialPort + 'static) -> Result<Self, Error> {
        let mut flasher = Flasher {
            connection: Connection::new(serial),
        };
        flasher.start_connection()?;
        Ok(flasher)
    }

    fn sync(&mut self) -> Result<(), Error> {
        let data = &[
            0x07u8, 0x07, 0x012, 0x20, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55,
            0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55,
            0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55,
        ][..];

        self.connection
            .write_command(Command::Sync as u8, data, 0)?;

        for _ in 0..10 {
            match self.connection.read_response(Timeouts::Sync as u64)? {
                Some(response) if response.return_op == Command::Sync as u8 => {
                    if response.status == 1 {
                        return Err(Error::RomError(RomError::from(response.error)));
                    } else {
                        break;
                    }
                }
                _ => continue,
            }
        }

        for _ in 0..7 {
            loop {
                match self.connection.read_response(Timeouts::Sync as u64)? {
                    Some(_) => break,
                    _ => continue,
                }
            }
        }

        Ok(())
    }

    fn start_connection(&mut self) -> Result<(), Error> {
        self.connection.reset_to_flash()?;
        for _ in 0..10 {
            self.connection.flush()?;
            if let Ok(_) = self.sync() {
                return Ok(());
            }
        }
        Err(Error::ConnectionFailed)
    }

    fn begin_command(
        &mut self,
        command: Command,
        size: u32,
        blocks: u32,
        block_size: u32,
        offset: u32,
    ) -> Result<(), Error> {
        let params = BeginParams {
            size,
            blocks,
            block_size,
            offset,
        };
        self.connection.command(
            command as u8,
            bytes_of(&params),
            0,
            Timeouts::Default as u64,
        )?;
        Ok(())
    }

    fn block_command(
        &mut self,
        command: Command,
        data: &[u8],
        padding: usize,
        padding_byte: u8,
        sequence: u32,
    ) -> Result<(), Error> {
        let params = BlockParams {
            size: (data.len() + padding) as u32,
            sequence,
            dummy1: 0,
            dummy2: 0,
        };

        let length = size_of::<BlockParams>() + data.len() + padding;

        self.connection.command(
            command as u8,
            (length as u16, |encoder: &mut Encoder| {
                encoder.write(bytes_of(&params))?;
                encoder.write(&data)?;
                let padding = &[padding_byte; FLASH_WRITE_SIZE][0..padding];
                encoder.write(padding)?;
                Ok(())
            }),
            checksum(&data, CHECKSUM_INIT) as u32,
            Timeouts::Default as u64,
        )?;
        Ok(())
    }

    fn mem_finish(&mut self, entry: u32) -> Result<(), Error> {
        let params = EntryParams {
            no_entry: (entry == 0) as u32,
            entry,
        };
        self.connection
            .write_command(Command::MemEnd as u8, bytes_of(&params), 0)?;
        Ok(())
    }

    fn flash_finish(&mut self, reboot: bool) -> Result<(), Error> {
        self.connection
            .write_command(Command::FlashEnd as u8, &[(!reboot) as u8][..], 0)?;
        Ok(())
    }

    fn enable_flash(&mut self) -> Result<(), Error> {
        // todo esp32 has a separate command for this
        self.begin_command(Command::FlashBegin, 0, 0, FLASH_WRITE_SIZE as u32, 0)?;
        Ok(())
    }

    /// Load an elf image to ram and execute it
    ///
    /// Note that this will not touch the flash on the device
    pub fn load_elf_to_ram(&mut self, elf_data: &[u8]) -> Result<(), Error> {
        self.start_connection()?;
        let image = FirmwareImage::from_data(elf_data).map_err(|_| Error::InvalidElf)?;

        if image.rom_segments().next().is_some() {
            return Err(Error::ElfNotRamLoadable);
        }

        for segment in image.ram_segments() {
            let padding = 4 - segment.data.len() % 4;
            let block_count =
                (segment.data.len() + padding + MAX_RAM_BLOCK_SIZE - 1) / MAX_RAM_BLOCK_SIZE;
            self.begin_command(
                Command::MemBegin,
                segment.data.len() as u32,
                block_count as u32,
                MAX_RAM_BLOCK_SIZE as u32,
                segment.addr,
            )?;

            for (i, block) in segment.data.chunks(MAX_RAM_BLOCK_SIZE).enumerate() {
                let block_padding = if i == block_count - 1 { padding } else { 0 };
                self.block_command(Command::MemData, &block, block_padding, 0, i as u32)?;
            }
        }

        self.mem_finish(image.entry())?;

        Ok(())
    }

    /// Load an elf image to flash and execute it
    pub fn load_elf_to_flash(&mut self, elf_data: &[u8]) -> Result<(), Error> {
        self.start_connection()?;
        self.enable_flash()?;
        let image = FirmwareImage::from_data(elf_data).map_err(|_| Error::InvalidElf)?;

        for segment in ESP8266::get_flash_segments(&image) {
            let segment = segment?;
            let addr = segment.addr;
            let block_count = (segment.data.len() + FLASH_WRITE_SIZE - 1) / FLASH_WRITE_SIZE;

            self.begin_command(
                Command::FlashBegin,
                get_erase_size(addr as usize, segment.data.len()) as u32,
                block_count as u32,
                FLASH_WRITE_SIZE as u32,
                addr,
            )?;

            for (i, block) in segment.data.chunks(FLASH_WRITE_SIZE).enumerate() {
                let block_padding = FLASH_WRITE_SIZE - block.len();
                self.block_command(Command::FlashData, &block, block_padding, 0xff, i as u32)?;
            }
        }

        self.flash_finish(true)?;

        Ok(())
    }
}

fn get_erase_size(offset: usize, size: usize) -> usize {
    let sector_count = (size + FLASH_SECTOR_SIZE - 1) / FLASH_SECTOR_SIZE;
    let start_sector = offset / FLASH_SECTOR_SIZE;

    let head_sectors = usize::min(
        FLASH_SECTORS_PER_BLOCK - (start_sector % FLASH_SECTORS_PER_BLOCK),
        sector_count,
    );

    if sector_count < 2 * head_sectors {
        (sector_count + 1) / 2 * FLASH_SECTOR_SIZE
    } else {
        (sector_count - head_sectors) * FLASH_SECTOR_SIZE
    }
}

const CHECKSUM_INIT: u8 = 0xEF;

pub fn checksum(data: &[u8], mut checksum: u8) -> u8 {
    for byte in data.as_ref() {
        checksum ^= *byte;
    }

    checksum
}
