// #![allow(unused_must_use)]
#![allow(dead_code)]

use serial::{SerialPort, BaudRate};
use std::thread::sleep;
use std::time::Duration;
use byteorder::{WriteBytesExt, LittleEndian};
use slip_codec::{Encoder, Decoder};
use bytemuck::{Pod, Zeroable, bytes_of};
use std::io::Cursor;
use elf2esp::{FirmwareImage, ESP8266V1};
use std::env::args;
use std::fs::read;

#[derive(Copy, Clone)]
#[repr(u64)]
enum Timeouts {
    Default = 3000,
    Sync = 100,
}

fn main() {
    let mut serial = serial::open("/dev/ttyUSB0").unwrap();
    serial.reconfigure(&|settings| {
        settings.set_baud_rate(BaudRate::Baud115200)?;

        Ok(())
    });
    serial.set_timeout(Duration::from_millis(Timeouts::Default as u64)).unwrap();
    let mut flasher = Flasher::new(serial);

    let mut args = args();
    let bin = args.next().unwrap();
    let input = args
        .next()
        .expect(&format!("usage: {} <input>", bin));

    let input_bytes = read(&input).unwrap();

    flasher.connect();
    flasher.mem_elf(&input_bytes);
}

const MAX_RAM_BLOCK_SIZE: u32 = 1024; //0x1800;
const ESP_ROM_BAUD: u32 = 0x1000;

#[derive(Copy, Clone, Debug)]
#[repr(u8)]
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

#[derive(Debug)]
struct CommandResponse {
    resp: u8,
    return_op: u8,
    return_length: u16,
    value: u32,
    data: Vec<u8>,
}

struct Flasher {
    serial: Box<dyn SerialPort>,
    encoder: Encoder,
    decoder: Decoder,
}

impl Flasher {
    pub fn new(serial: impl SerialPort + 'static) -> Self {
        Flasher {
            serial: Box::new(serial),
            encoder: Encoder::new(),
            decoder: Decoder::new(1024),
        }
    }

    fn reset_to_flash(&mut self) {
        self.serial.set_dtr(false).unwrap();
        self.serial.set_rts(true).unwrap();

        sleep(Duration::from_millis(100));

        self.serial.set_dtr(true).unwrap();
        self.serial.set_rts(false).unwrap();

        sleep(Duration::from_millis(50));

        self.serial.set_dtr(true).unwrap();
    }

    fn read_response(&mut self, timeout: Timeouts) -> Result<Option<CommandResponse>, slip_codec::Error> {
        let response = self.read(timeout)?;
        if response.len() < 8 {
            return Ok(None);
        }

        let resp = response[0];
        let return_op = response[1];
        let return_length = u16::from_le_bytes([response[2], response[3]]);
        let value = u32::from_le_bytes([response[4], response[5], response[6], response[7]]);

        println!("resp_op {}", return_op);

        Ok(Some(CommandResponse {
            resp,
            return_op,
            return_length,
            value,
            data: response[8..].to_vec(),
        }))
    }

    fn send_command(&mut self, command: Command, data: &[u8], check: u32, timeout: Timeouts) -> Result<CommandResponse, slip_codec::Error> {
        println!("command: {:?}", command);
        let mut packet = Vec::new();
        packet.push(0);
        packet.push(command as u8);
        packet.write_u16::<LittleEndian>(data.len() as u16)?;
        packet.write_u32::<LittleEndian>(check)?;
        packet.extend_from_slice(data);

        self.write(&packet);

        for _ in 0..100 {
            match dbg!(self.read_response(timeout)?) {
                Some(response)if response.return_op == command as u8 => return Ok(response),
                _ => continue
            };
        }
        panic!("timeout?");
    }

    fn write(&mut self, packet: &[u8]) {
        let mut buff = Vec::new();
        self.encoder.encode(packet, &mut buff).unwrap();
        self.serial.write(&buff).unwrap();
    }

    fn read(&mut self, timeout: Timeouts) -> Result<Vec<u8>, slip_codec::Error> {
        let mut buff = vec![0; 64];
        self.serial.set_timeout(Duration::from_millis(timeout as u64)).unwrap();
        let count = self.serial.read(&mut buff)?;
        buff.resize(count, 0);
        self.decoder.decode(&mut Cursor::new(buff))
    }

    fn sync(&mut self) -> Result<(), slip_codec::Error> {
        let mut data = Vec::with_capacity(40);
        data.extend_from_slice(&[0x07, 0x07, 0x012, 0x20]);
        data.extend_from_slice(&[0x55; 32]);

        self.send_command(Command::Sync, &data, 0, Timeouts::Sync)?;

        for _ in 0..7 {
            loop {
                match self.read_response(Timeouts::Sync)? {
                    Some(_) => break,
                    _ => continue
                }
            }
        }

        Ok(())
    }

    fn connect(&mut self) {
        self.reset_to_flash();
        for _ in 0..10 {
            // let mut buff = vec![0; 1024];
            // self.serial.read(&mut buff);
            self.serial.flush().unwrap();
            if let Ok(_) = self.sync() {
                return;
            }
        }
    }

    fn mem_begin(&mut self, size: u32, blocks: u32, block_size: u32, offset: u32) {
        #[derive(Zeroable, Pod, Copy, Clone, Debug)]
        #[repr(C)]
        struct MemBeginParams {
            size: u32,
            blocks: u32,
            block_size: u32,
            offset: u32,
        }

        let params = dbg!(MemBeginParams { size, blocks, block_size, offset });
        self.send_command(Command::MemBegin, bytes_of(&params), 0, Timeouts::Default).unwrap();
    }

    fn mem_block(&mut self, data: &[u8], sequence: u32) {
        #[derive(Zeroable, Pod, Copy, Clone, Debug)]
        #[repr(C)]
        struct MemBlockParams {
            size: u32,
            sequence: u32,
            dummy1: u32,
            dummy2: u32,
        }

        let params = dbg!(MemBlockParams {
            size: data.len() as u32,
            sequence,
            dummy1: 0,
            dummy2: 0,
        });

        let mut buff = Vec::new();
        buff.extend_from_slice(bytes_of(&params));
        buff.extend_from_slice(data);
        self.send_command(Command::MemData, &buff, checksum(&data, CHECKSUM_INIT) as u32, Timeouts::Default).unwrap();
    }

    fn mem_finish(&mut self, entry: u32) {
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
        self.send_command(Command::MemEnd, bytes_of(&params), 0, Timeouts::Default).unwrap();
    }

    fn mem_elf(&mut self, elf_data: &[u8]) {
        let image = FirmwareImage::from_data(elf_data).unwrap();

        let segments = image.save::<ESP8266V1>();

        for segment in image.ram_segments() {
            let block_count = (segment.data.len() as u32 + MAX_RAM_BLOCK_SIZE - 1) / MAX_RAM_BLOCK_SIZE;
            self.mem_begin(segment.data.len() as u32, block_count as u32, MAX_RAM_BLOCK_SIZE, segment.addr);

            for (i, block) in segment.data.chunks(MAX_RAM_BLOCK_SIZE as usize).enumerate() {
                dbg!(block.len());
                self.mem_block(block, i as u32);
            }
        }

        eprintln!("entry: {:#x}", image.entry());
        self.mem_finish(image.entry())
    }
}

const CHECKSUM_INIT: u8 = 0xEF;

fn checksum(data: &[u8], mut checksum: u8) -> u8 {
    for byte in data.as_ref() {
        checksum ^= *byte;
    }

    checksum
}