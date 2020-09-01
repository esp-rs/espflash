use crate::elf::FirmwareImage;
use crate::encoder::SlipEncoder;
use crate::Error;
use bytemuck::{bytes_of, from_bytes, Pod, Zeroable};
use serial::SerialPort;
use slip_codec::Decoder;
use std::io::Write;
use std::thread::sleep;
use std::time::Duration;

#[derive(Copy, Clone)]
#[repr(u64)]
enum Timeouts {
    Default = 3000,
    Sync = 100,
}

const MAX_RAM_BLOCK_SIZE: u32 = 0x1800;

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

    fn write_command<'a>(
        &mut self,
        command: Command,
        data: &[u8],
        check: u32,
    ) -> Result<(), Error> {
        let mut encoder = SlipEncoder::new(&mut self.serial)?;
        encoder.write(&[0])?;
        encoder.write(&[command as u8])?;
        encoder.write(&((data.len() as u16).to_le_bytes()))?;
        encoder.write(&(check.to_le_bytes()))?;
        encoder.write(&data)?;
        encoder.finish()?;
        Ok(())
    }

    fn command<'a>(
        &mut self,
        command: Command,
        data: &[u8],
        check: u32,
        timeout: Timeouts,
    ) -> Result<CommandResponse, Error> {
        self.write_command(command, data, check)?;

        for _ in 0..10 {
            match self.read_response(timeout)? {
                Some(response) if response.return_op == command as u8 => return Ok(response),
                _ => continue,
            };
        }
        panic!("timeout?");
    }

    fn read(&mut self, timeout: Timeouts) -> Result<Vec<u8>, Error> {
        self.serial
            .set_timeout(Duration::from_millis(timeout as u64))
            .unwrap();
        Ok(self.decoder.decode(&mut self.serial)?)
    }

    fn sync(&mut self) -> Result<(), Error> {
        let mut data = Vec::with_capacity(40);
        data.extend_from_slice(&[0x07, 0x07, 0x012, 0x20]);
        data.extend_from_slice(&[0x55; 32]);

        self.command(Command::Sync, &data, 0, Timeouts::Sync)?;

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

    fn mem_begin(
        &mut self,
        size: u32,
        blocks: u32,
        block_size: u32,
        offset: u32,
    ) -> Result<(), Error> {
        #[derive(Zeroable, Pod, Copy, Clone, Debug)]
        #[repr(C)]
        struct MemBeginParams {
            size: u32,
            blocks: u32,
            block_size: u32,
            offset: u32,
        }

        let params = MemBeginParams {
            size,
            blocks,
            block_size,
            offset,
        };
        self.command(Command::MemBegin, bytes_of(&params), 0, Timeouts::Default)?;
        Ok(())
    }

    fn mem_block(&mut self, data: &[u8], sequence: u32) -> Result<(), Error> {
        #[derive(Zeroable, Pod, Copy, Clone, Debug)]
        #[repr(C)]
        struct MemBlockParams {
            size: u32,
            sequence: u32,
            dummy1: u32,
            dummy2: u32,
        }

        let params = MemBlockParams {
            size: data.len() as u32,
            sequence,
            dummy1: 0,
            dummy2: 0,
        };

        let mut buff = Vec::new();
        buff.extend_from_slice(bytes_of(&params));
        buff.extend_from_slice(data);
        self.command(
            Command::MemData,
            &buff,
            checksum(&data, CHECKSUM_INIT) as u32,
            Timeouts::Default,
        )?;
        Ok(())
    }

    fn mem_finish(&mut self, entry: u32) -> Result<(), Error> {
        #[derive(Zeroable, Pod, Copy, Clone)]
        #[repr(C)]
        struct EntryParams {
            no_entry: u32,
            entry: u32,
        }
        let params = EntryParams {
            no_entry: (entry == 0) as u32,
            entry,
        };
        self.write_command(Command::MemEnd, bytes_of(&params), 0)?;
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
            let block_count =
                (segment.data.len() as u32 + MAX_RAM_BLOCK_SIZE - 1) / MAX_RAM_BLOCK_SIZE;
            self.mem_begin(
                segment.data.len() as u32,
                block_count as u32,
                MAX_RAM_BLOCK_SIZE,
                segment.addr,
            )?;

            for (i, block) in segment.data.chunks(MAX_RAM_BLOCK_SIZE as usize).enumerate() {
                let mut block = block.to_vec();
                let padding = 4 - block.len() % 4;
                block.resize(block.len() + padding, 0);
                self.mem_block(&block, i as u32)?;
            }
        }

        self.mem_finish(image.entry())?;

        Ok(())
    }
}

const CHECKSUM_INIT: u8 = 0xEF;

fn checksum(data: &[u8], mut checksum: u8) -> u8 {
    for byte in data.as_ref() {
        checksum ^= *byte;
    }

    checksum
}
