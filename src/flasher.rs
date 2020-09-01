use crate::elf::{FirmwareImage, ESP8266V1};
use crate::encoder::SlipEncoder;
use crate::error::RomError;
use crate::Error;
use bytemuck::{bytes_of, from_bytes, Pod, Zeroable};
use serial::SerialPort;
use slip_codec::Decoder;
use std::io::Write;
use std::mem::size_of;
use std::thread::sleep;
use std::time::Duration;

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

#[derive(Debug, Zeroable, Pod, Copy, Clone)]
#[repr(C)]
#[repr(packed)]
struct CommandResponse {
    resp: u8,
    return_op: u8,
    return_length: u16,
    value: u32,
    status: u8,
    error: u8,
}

pub struct Flasher {
    serial: Box<dyn SerialPort>,
    decoder: Decoder,
    connected: bool,
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

impl Flasher {
    pub fn new(serial: impl SerialPort + 'static) -> Self {
        Flasher {
            serial: Box::new(serial),
            decoder: Decoder::new(1024),
            connected: false,
        }
    }

    fn reset_to_flash(&mut self) -> Result<(), Error> {
        self.serial.set_dtr(false)?;
        self.serial.set_rts(true)?;

        sleep(Duration::from_millis(100));

        self.serial.set_dtr(true)?;
        self.serial.set_rts(false)?;

        sleep(Duration::from_millis(50));

        self.serial.set_dtr(true)?;

        Ok(())
    }

    fn read_response(&mut self, timeout: Timeouts) -> Result<Option<CommandResponse>, Error> {
        let response = self.read(timeout)?;
        if response.len() < 10 {
            return Ok(None);
        }

        let header: CommandResponse = *from_bytes(&response[0..10]);

        Ok(Some(header))
    }

    fn write_command(
        &mut self,
        command: Command,
        data: impl CommandData<Box<dyn SerialPort>>,
        check: u32,
    ) -> Result<(), Error> {
        let mut encoder = SlipEncoder::new(&mut self.serial)?;
        encoder.write(&[0])?;
        encoder.write(&[command as u8])?;
        encoder.write(&(data.length().to_le_bytes()))?;
        encoder.write(&(check.to_le_bytes()))?;
        data.write(&mut encoder)?;
        encoder.finish()?;
        Ok(())
    }

    fn command<'a>(
        &mut self,
        command: Command,
        data: impl CommandData<Box<dyn SerialPort>>,
        check: u32,
        timeout: Timeouts,
    ) -> Result<CommandResponse, Error> {
        self.write_command(command, data, check)?;

        match self.read_response(timeout)? {
            Some(response) if response.return_op == command as u8 => {
                if response.status == 1 {
                    Err(Error::RomError(RomError::from(response.error)))
                } else {
                    Ok(response)
                }
            }
            _ => Err(Error::ConnectionFailed),
        }
    }

    fn read(&mut self, timeout: Timeouts) -> Result<Vec<u8>, Error> {
        self.serial
            .set_timeout(Duration::from_millis(timeout as u64))
            .unwrap();
        Ok(self.decoder.decode(&mut self.serial)?)
    }

    fn sync(&mut self) -> Result<(), Error> {
        let data = &[
            0x07u8, 0x07, 0x012, 0x20, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55,
            0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55,
            0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55,
        ][..];

        self.write_command(Command::Sync, data, 0)?;

        for _ in 0..10 {
            match self.read_response(Timeouts::Sync)? {
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
                match self.read_response(Timeouts::Sync)? {
                    Some(_) => break,
                    _ => continue,
                }
            }
        }

        Ok(())
    }

    pub fn connect(&mut self) -> Result<(), Error> {
        if self.connected {
            return Ok(());
        }
        self.reset_to_flash()?;
        for _ in 0..10 {
            self.serial.flush()?;
            if let Ok(_) = self.sync() {
                return Ok(());
            }
        }
        self.connected = true;
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
        self.command(command, bytes_of(&params), 0, Timeouts::Default)?;
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

        self.command(
            command,
            (length as u16, |encoder: &mut Encoder| {
                encoder.write(bytes_of(&params))?;
                encoder.write(&data)?;
                let padding = &[padding_byte; FLASH_WRITE_SIZE][0..padding];
                encoder.write(padding)?;
                Ok(())
            }),
            checksum(&data, CHECKSUM_INIT) as u32,
            Timeouts::Default,
        )?;
        Ok(())
    }

    fn mem_finish(&mut self, entry: u32) -> Result<(), Error> {
        let params = EntryParams {
            no_entry: (entry == 0) as u32,
            entry,
        };
        self.write_command(Command::MemEnd, bytes_of(&params), 0)?;
        Ok(())
    }

    fn flash_finish(&mut self, reboot: bool) -> Result<(), Error> {
        self.write_command(Command::FlashEnd, &[(!reboot) as u8][..], 0)?;
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
        self.connect()?;
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
        self.connect()?;
        self.enable_flash()?;
        let image = FirmwareImage::from_data(elf_data).map_err(|_| Error::InvalidElf)?;

        for segment in image.save::<ESP8266V1>() {
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

trait CommandData<W: Write> {
    fn write(self, encoder: &mut SlipEncoder<W>) -> Result<(), Error>;

    fn length(&self) -> u16;
}

impl<W: Write> CommandData<W> for &[u8] {
    fn write(self, encoder: &mut SlipEncoder<W>) -> Result<(), Error> {
        encoder.write(self)?;
        Ok(())
    }

    fn length(&self) -> u16 {
        self.len() as u16
    }
}

impl<W: Write, F: Fn(&mut SlipEncoder<W>) -> Result<(), Error>> CommandData<W> for (u16, F) {
    fn write(self, encoder: &mut SlipEncoder<W>) -> Result<(), Error> {
        self.1(encoder)
    }

    fn length(&self) -> u16 {
        self.0
    }
}
