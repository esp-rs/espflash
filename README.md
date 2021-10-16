# ESPFlash

[![Actions Status](https://github.com/esp-rs/espflash/workflows/CI/badge.svg)](https://github.com/esp-rs/espflash/actions?query=workflow%3A"CI")

_ESP8266_ and _ESP32_ family serial flasher based on [esptool.py](https://github.com/espressif/esptool).

* [espflash library & binary](https://github.com/esp-rs/espflash/tree/master/espflash)
* [espflash cargo subcommand](https://github.com/esp-rs/espflash/tree/master/cargo-espflash)

## Usage

Note: the documentation below is for the `cargo espflash` sub-command, which is probably what you are looking for.
For the standalone espflash binary, follow the link above

```text
cargo-espflash 1.1.0                                                                                                                                                                                                                                           
Cargo subcommand for flashing Espressif devices over serial                                                                                                                                                                                                    
                                                                                                                                                                                                                                                               
USAGE:                                                                                                                                                                                                                                                         
    cargo espflash [FLAGS] [OPTIONS] [SERIAL] [SUBCOMMAND]                                                                                                                                                                                                     
                                                                                                                                                                                                                                                               
FLAGS:                                                                                                                                                                                                                                                         
        --board-info    Display the connected board's information (deprecated, use the `board-info` subcommand instead)                                                                                                                                        
    -h, --help          Prints help information                                                                                                                                                                                                                
        --monitor       Open a serial monitor after flashing                                                                                                                                                                                                   
        --ram           Load the application to RAM instead of Flash                                                                                                                                                                                           
        --release       Build the application using the release profile                                                                                                                                                                                        
    -V, --version       Prints version information                                                                                                                                                                                                             
                                                                                                                                                                                                                                                               
OPTIONS:                                                                                                                                                                                                                                                       
        --bootloader <PATH>         Path to a binary (.bin) bootloader file                                                                                                                                                                                    
        --example <EXAMPLE>         Example to build and flash                                                                                                                                                                                                 
        --features <FEATURES>       Comma delimited list of build features                                                                                                                                                                                     
        --format <image format>     Image format to flash                                                                                                                                                                                                      
        --partition-table <PATH>    Path to a CSV file containing partition table                                                                                                                                                                              
        --speed <SPEED>             Baud rate at which to flash target device                                                                                                                                                                                  
                                                                                                                                                                                                                                                               
ARGS:                                                                                                                                                                                                                                                          
    <SERIAL>    Serial port connected to target device                                                                                                                                                                                                         
                                                                                                                                                                                                                                                               
SUBCOMMANDS:                                                                                                                                                                                                                                                   
    board-info    Display the connected board's information                                                                                                                                                                                                    
    help          Prints this message or the help of the given subcommand(s)                                                                                                                                                                                   
    save-image    Save the image to disk instead of flashing to device
```

When the `--ram` option is specified, the provided ELF image will be loaded into ram and executed without touching the flash.

### Config

You can also specify the serial port by setting it in the config file located at `~/.config/espflash/espflash.toml` or Linux
or `%APPDATA%/esp/espflash/espflash.toml` on Windows.

```toml
[connection]
serial = "/dev/ttyUSB0"
```

### Package metadata

You can also specify the bootloader, partition table or image format for a project in the package metadata in `Cargo.toml`

```toml
[package.metadata.espflash]                                                                                                                                                                                                                                    
partition_table = "partitions.csv"
bootloader = "bootloader.bin"
format = "direct-boot"
```

### Example

```bash
$ cargo espflash --release --example blinky /dev/ttyUSB0
```

## License

Licensed under the GNU General Public License Version 2. See [LICENSE](LICENSE) for more details.

## Quickstart - Docker

The docker image `esprs/espflash` contains all necessary toolchains and tooling including espflash to build and flash.
To clone, build and flash the [esp32-hal](https://github.com/esp-rs/esp32-hal) examples run the following:

```cmd
git clone https://github.com/esp-rs/esp32-hal
cd esp32-hal
docker run -v "$(pwd):/espflash" --device=/dev/ttyUSB0 -ti esprs/espflash --release --tool=cargo --example=blinky /dev/ttyUSB0
```

### Custom Docker Build

```cmd
git clone --depth 1 https://github.com/esp-rs/espflash.git
cd espflash
docker build -t esprs/espflash .
```
