#[test]
fn flash_test() {
    use espflash::cli::{
        config::Config, connect, erase_partitions, flash_elf_image, monitor::LogFormat,
        parse_partition_table, ConnectArgs, EspflashProgress, FlashArgs, FlashConfigArgs,
    };
    use espflash::targets::Chip;
    use std::{fs, path::PathBuf};

    // let image = PathBuf::from("esp32c3");
    let image = PathBuf::from(std::env::var("ESPFLASH_APP").expect("ESPFLASH_APP not set"));
    let port = std::env::var("ESPFLASH_PORT").expect("ESPFLASH_PORT not set");

    let config = Config::default();

    let conn = ConnectArgs {
        baud: Some(460800),
        chip: Some(Chip::Esp32c3),
        confirm_port: false,
        no_stub: false,
        port: Some(port),
    };

    let mut flasher = connect(&conn, &config).unwrap();

    let flash_config_args = FlashConfigArgs {
        flash_freq: None,
        flash_mode: None,
        flash_size: None,
    };

    if let Some(flash_size) = flash_config_args.flash_size {
        flasher.set_flash_size(flash_size);
    }

    let flash_args = FlashArgs {
        bootloader: None,
        erase_parts: None,
        erase_data_parts: None,
        format: None,
        log_format: LogFormat::Serial,
        monitor: true,
        monitor_baud: None,
        partition_table: None,
        target_app_partition: None,
        partition_table_offset: None,
        ram: false,
    };

    // Read the ELF data from the build path and load it to the target.
    let elf_data = fs::read(&image).unwrap();

    if flash_args.ram {
        flasher
            .load_elf_to_ram(&elf_data, Some(&mut EspflashProgress::default()))
            .unwrap();
    } else {
        let bootloader = flash_args.bootloader.as_deref();
        let partition_table = flash_args.partition_table.as_deref();

        // if let Some(path) = bootloader {
        //     println!("Bootloader:        {}", path.display());
        // }
        // if let Some(path) = partition_table {
        //     println!("Partition table:   {}", path.display());
        // }

        let partition_table = match partition_table {
            Some(path) => Some(parse_partition_table(path).unwrap()),
            None => None,
        };

        if flash_args.erase_parts.is_some() || flash_args.erase_data_parts.is_some() {
            erase_partitions(
                &mut flasher,
                partition_table.clone(),
                flash_args.erase_parts,
                flash_args.erase_data_parts,
            )
            .unwrap();
        }

        flash_elf_image(
            &mut flasher,
            &elf_data,
            bootloader,
            partition_table,
            flash_args.target_app_partition,
            flash_args.format,
            flash_config_args.flash_mode,
            flash_config_args.flash_size,
            flash_config_args.flash_freq,
            flash_args.partition_table_offset,
        )
        .unwrap();
    }
    // println!("Firmware flashing completed. ");
}
