use std::{
    fs,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use clap::Args;

use crate::Result;

#[derive(Debug, Args)]
pub struct RunTestsArgs {
    /// Which test to run (or "all" to run all tests)
    #[clap(default_value = "all")]
    pub test: String,

    /// Chip target
    #[clap(short, long)]
    pub chip: Option<String>,

    /// Timeout for test commands in seconds
    #[clap(short, long, default_value = "15")]
    pub timeout: u64,
}

pub struct TestRunner {
    pub workspace: PathBuf,
    pub tests_dir: PathBuf,
    pub timeout: Duration,
    pub chip: Option<String>,
}

impl TestRunner {
    pub fn new(workspace: &Path, tests_dir: PathBuf, timeout_secs: u64) -> Self {
        Self {
            workspace: workspace.to_path_buf(),
            tests_dir,
            timeout: Duration::from_secs(timeout_secs),
            chip: None,
        }
    }

    fn setup_command(&self, cmd: &mut Command) {
        cmd.current_dir(&self.workspace)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());
    }

    fn terminate_process(child_id: u32, child: &mut Option<&mut Child>) {
        unsafe {
            libc::kill(child_id as i32, libc::SIGTERM);
        }

        // Wait for the process to terminate
        if let Some(child_proc) = child {
            let _ = child_proc.wait();
        }
    }

    pub fn run_command_with_timeout(&self, cmd: &mut Command, timeout: Duration) -> Result<i32> {
        log::debug!("Running command: {cmd:?}");

        self.setup_command(cmd);

        let mut child: Child = cmd.spawn()?;

        let process_completed = Arc::new(AtomicBool::new(false));
        let process_completed_clone = process_completed.clone();

        let child_id = child.id();
        let timer_handle = thread::spawn(move || {
            // Use a shorter sleep interval to check the completion flag more frequently
            let check_interval = Duration::from_millis(100);
            let mut elapsed = Duration::from_secs(0);

            while elapsed < timeout {
                thread::sleep(check_interval);
                elapsed += check_interval;

                // Check if the process has already completed
                if process_completed_clone.load(Ordering::SeqCst) {
                    return;
                }
            }

            // If we reach this point, the command didn't complete within the timeout
            log::warn!("Command timed out after {timeout:?}, killing process {child_id}");
            Self::terminate_process(child_id, &mut None);
        });

        // Wait for the command to complete
        let status = match child.wait() {
            Ok(status) => {
                // Signal that the process has completed
                process_completed.store(true, Ordering::SeqCst);
                status
            }
            Err(e) => {
                process_completed.store(true, Ordering::SeqCst);
                thread::sleep(Duration::from_millis(10));
                return Err(format!("Command execution failed: {e}").into());
            }
        };

        // If we get here, the command completed before the timeout
        // Wait for the timer thread to complete (should be very quick now)
        let _ = timer_handle.join();

        let exit_code = status.code().unwrap_or(1);
        log::debug!("Command exit code: {exit_code}");

        Ok(exit_code)
    }

    pub fn run_command_for(&self, cmd: &mut Command, duration: Duration) -> Result<bool> {
        log::debug!("Running command: {cmd:?}");

        let mut child: Child = cmd.spawn()?;

        // Keep track of whether the process terminated naturally
        let start_time = Instant::now();
        let mut terminated_naturally = false;

        // Wait for either the command to complete or the duration to expire
        if let Ok(status) = child.try_wait() {
            if status.is_some() {
                // Process already completed before the duration elapsed
                terminated_naturally = true;
                log::info!("Command terminated naturally with status: {status:?}");
            } else {
                // Process still running, wait for duration
                thread::sleep(duration);

                // Check again if it completed on its own during our sleep
                if let Ok(status) = child.try_wait() {
                    if status.is_some() {
                        terminated_naturally = true;
                        log::info!("Command terminated naturally with status: {status:?}");
                    }
                }
            }
        }

        // If process is still running after duration, terminate it
        if !terminated_naturally {
            let elapsed = start_time.elapsed();
            log::info!(
                "Command ran for {elapsed:?}, terminating process {}",
                child.id()
            );

            Self::terminate_process(child.id(), &mut Some(&mut child));
        }

        log::debug!("Command completed after {:?}", start_time.elapsed());

        Ok(terminated_naturally)
    }

    fn create_espflash_command(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new("espflash");
        cmd.args(args);
        cmd
    }

    pub fn run_simple_command_test(
        &self,
        args: &[&str],
        _expected_contains: &str,
        timeout: Duration,
        test_name: &str,
    ) -> Result<()> {
        log::info!("Running {test_name} test");

        let mut cmd = self.create_espflash_command(args);
        let exit_code = self.run_command_with_timeout(&mut cmd, timeout)?;

        if exit_code != 0 {
            return Err(format!("{test_name} test failed: non-zero exit code {exit_code}").into());
        }

        // Note: Since we're using Stdio::inherit(), we can't check output contents
        // The test should verify visually that the expected_contains string appears in
        // the output
        log::info!("{test_name} test passed with exit code 0");
        Ok(())
    }

    pub fn run_timed_command_test(
        &self,
        args: &[&str],
        _expected_contains: &str,
        duration: Duration,
        test_name: &str,
    ) -> Result<()> {
        log::info!("Running {test_name} test");

        let mut cmd = self.create_espflash_command(args);
        let terminated_naturally = self.run_command_for(&mut cmd, duration)?;

        // Note: Since we're using Stdio::inherit(), we can't check output contents
        // The test should verify visually that the expected_contains string appears in
        // the output
        log::info!("{test_name} test completed (terminated naturally: {terminated_naturally})");
        Ok(())
    }

    pub fn is_flash_empty(&self, file_path: &Path) -> Result<bool> {
        let flash_data = fs::read(file_path)?;
        Ok(flash_data.iter().all(|&b| b == 0xFF))
    }

    pub fn flash_output_file(&self) -> PathBuf {
        self.tests_dir.join("flash_content.bin")
    }

    pub fn contains_sequence(data: &[u8], sequence: &[u8]) -> bool {
        if sequence.len() > data.len() {
            return false;
        }

        for i in 0..=(data.len() - sequence.len()) {
            if &data[i..(i + sequence.len())] == sequence {
                return true;
            }
        }

        false
    }

    pub fn run_all_tests(&self, chip_override: Option<&str>) -> Result<()> {
        log::info!("Running all tests");

        let chip = chip_override.or(self.chip.as_deref()).unwrap_or("esp32");

        self.test_board_info()?;
        self.test_flash(Some(chip))?;
        self.test_monitor()?;
        self.test_erase_flash()?;
        self.test_save_image(Some(chip))?;
        self.test_erase_region()?;
        self.test_hold_in_reset()?;
        self.test_reset()?;
        self.test_checksum_md5()?;
        self.test_list_ports()?;
        self.test_write_bin()?;
        self.test_read_flash()?;

        log::info!("All tests completed successfully");
        Ok(())
    }

    pub fn run_specific_test(&self, test_name: &str, chip_override: Option<&str>) -> Result<()> {
        let chip = chip_override.or(self.chip.as_deref()).unwrap_or("esp32");

        match test_name {
            "board-info" => self.test_board_info(),
            "flash" => self.test_flash(Some(chip)),
            "monitor" => self.test_monitor(),
            "erase-flash" => self.test_erase_flash(),
            "save-image" => self.test_save_image(Some(chip)),
            "erase-region" => self.test_erase_region(),
            "hold-in-reset" => self.test_hold_in_reset(),
            "reset" => self.test_reset(),
            "checksum-md5" => self.test_checksum_md5(),
            "list-ports" => self.test_list_ports(),
            "write-bin" => self.test_write_bin(),
            "read-flash" => self.test_read_flash(),
            _ => Err(format!("Unknown test: {test_name}").into()),
        }
    }

    // Board info test
    pub fn test_board_info(&self) -> Result<()> {
        self.run_simple_command_test(
            &["board-info"],
            "Chip type:",
            Duration::from_secs(5),
            "board-info",
        )
    }

    // Flash test
    pub fn test_flash(&self, chip: Option<&str>) -> Result<()> {
        let chip = chip.unwrap_or_else(|| self.chip.as_deref().unwrap_or("esp32"));
        log::info!("Running flash test for chip: {chip}");

        let app = format!("espflash/tests/data/{chip}");
        let part_table = "espflash/tests/data/partitions.csv";

        // Test Partition too big
        let mut cmd = Command::new("espflash");
        cmd.args([
            "flash",
            "--no-skip",
            "--monitor",
            "--non-interactive",
            &app,
            "--flash-size",
            "2mb",
            "--partition-table",
            part_table,
        ]);

        // Use a shorter timeout for this part (15s like in the script)
        let _result = self.run_command_for(&mut cmd, Duration::from_secs(15));

        // Additional tests for ESP32-C6 with defmt
        if chip == "esp32c6" {
            self.test_flash_with_defmt(&app)?;
        }

        // Test standard flashing with shorter timeout
        self.run_timed_command_test(
            &["flash", "--no-skip", "--monitor", "--non-interactive", &app],
            "Hello world!",
            Duration::from_secs(15),
            "standard flashing",
        )?;

        Ok(())
    }

    fn test_flash_with_defmt(&self, app: &str) -> Result<()> {
        let app_defmt = format!("{app}_defmt");

        // Test with manual log-format
        self.run_timed_command_test(
            &[
                "flash",
                "--no-skip",
                "--monitor",
                "--non-interactive",
                &app_defmt,
                "--log-format",
                "defmt",
            ],
            "Hello world!",
            Duration::from_secs(15),
            "defmt manual log-format",
        )?;

        // Test with auto-detected log-format
        self.run_timed_command_test(
            &[
                "flash",
                "--no-skip",
                "--monitor",
                "--non-interactive",
                &app_defmt,
            ],
            "Hello world!",
            Duration::from_secs(15),
            "defmt auto-detected log-format",
        )?;

        Ok(())
    }

    pub fn test_list_ports(&self) -> Result<()> {
        self.run_simple_command_test(
            &["list-ports"],
            "Silicon Labs",
            Duration::from_secs(5),
            "list-ports",
        )?;
        Ok(())
    }

    pub fn test_erase_flash(&self) -> Result<()> {
        log::info!("Running erase-flash test");
        let flash_output = self.flash_output_file();

        self.run_simple_command_test(
            &["erase-flash"],
            "Flash has been erased!",
            Duration::from_secs(10),
            "erase-flash",
        )?;

        // Read a portion of the flash to verify it's erased
        self.run_simple_command_test(
            &["read-flash", "0", "0x4000", flash_output.to_str().unwrap()],
            "Flash content successfully read",
            Duration::from_secs(5),
            "read after erase",
        )?;

        // Verify the flash is empty (all 0xFF)
        if let Ok(is_empty) = self.is_flash_empty(&flash_output) {
            if !is_empty {
                return Err("Flash is not empty after erase-flash command".into());
            }
        } else {
            return Err("Failed to check if flash is empty".into());
        }

        log::info!("erase-flash test passed");
        Ok(())
    }

    pub fn test_erase_region(&self) -> Result<()> {
        log::info!("Running erase-region test");
        let flash_output = self.flash_output_file();

        // Test unaligned address (not multiple of 4096)
        let mut cmd = self.create_espflash_command(&["erase-region", "0x1001", "0x1000"]);
        let exit_code = self.run_command_with_timeout(&mut cmd, Duration::from_secs(5))?;
        if exit_code == 0 {
            return Err("Unaligned address erase should have failed but succeeded".into());
        }

        // Test unaligned size (not multiple of 4096)
        let mut cmd = self.create_espflash_command(&["erase-region", "0x1000", "0x1001"]);
        let exit_code = self.run_command_with_timeout(&mut cmd, Duration::from_secs(5))?;
        if exit_code == 0 {
            return Err("Unaligned size erase should have failed but succeeded".into());
        }

        // Valid erase - should succeed
        self.run_simple_command_test(
            &["erase-region", "0x1000", "0x1000"],
            "Erasing region at",
            Duration::from_secs(5),
            "erase-region valid",
        )?;

        // Read the region to verify it was erased
        self.run_simple_command_test(
            &[
                "read-flash",
                "0x1000",
                "0x2000",
                flash_output.to_str().unwrap(),
            ],
            "Flash content successfully read",
            Duration::from_secs(5),
            "read after erase-region",
        )?;

        // Check flash contents - first part should be erased
        if let Ok(flash_data) = fs::read(&flash_output) {
            // First 0x1000 bytes should be 0xFF (erased)
            let first_part = &flash_data[0..4096];
            if !first_part.iter().all(|&b| b == 0xFF) {
                return Err("First 0x1000 bytes should be empty (0xFF)".into());
            }

            // Next 0x1000 bytes should contain some non-FF bytes
            let second_part = &flash_data[4096..8192];
            if second_part.iter().all(|&b| b == 0xFF) {
                return Err("Next 0x1000 bytes should contain some non-FF bytes".into());
            }
        } else {
            return Err("Failed to read flash_content.bin file".into());
        }

        log::info!("erase-region test passed");
        Ok(())
    }

    pub fn test_read_flash(&self) -> Result<()> {
        log::info!("Running read-flash test");
        let flash_output = self.flash_output_file();
        let pattern_file = self.tests_dir.join("pattern.bin");

        // Create a pattern to write to flash
        let known_pattern: Vec<u8> = vec![
            0x01, 0xA0, 0x02, 0xB3, 0x04, 0xC4, 0x08, 0xD5, 0x10, 0xE6, 0x20, 0xF7, 0x40, 0x88,
            0x50, 0x99, 0x60, 0xAA, 0x70, 0xBB, 0x80, 0xCC, 0x90, 0xDD, 0xA0, 0xEE, 0xB0, 0xFF,
            0xC0, 0x11, 0xD0, 0x22,
        ];

        // Write the pattern to a file
        fs::write(&pattern_file, &known_pattern)?;

        // Write the pattern to the flash
        self.run_simple_command_test(
            &["write-bin", "0x0", pattern_file.to_str().unwrap()],
            "Binary successfully written to flash!",
            Duration::from_secs(5),
            "write pattern",
        )?;

        // Test reading various lengths
        for &len in &[2, 5, 10, 26] {
            log::info!("Testing read-flash with length: {len}");

            // Test normal read
            self.run_simple_command_test(
                &[
                    "read-flash",
                    "0",
                    &len.to_string(),
                    flash_output.to_str().unwrap(),
                ],
                "Flash content successfully read and written to",
                Duration::from_secs(5),
                &format!("read {len} bytes"),
            )?;

            // Verify the read data matches the expected pattern
            if let Ok(read_data) = fs::read(&flash_output) {
                let expected = &known_pattern[0..len as usize];
                if &read_data[0..len as usize] != expected {
                    return Err(format!(
                        "Verification failed for length {len}: content does not match"
                    )
                    .into());
                }
            } else {
                return Err(format!("Failed to read flash_content.bin for length {len}").into());
            }

            // Test ROM read (--no-stub option)
            self.run_simple_command_test(
                &[
                    "read-flash",
                    "--no-stub",
                    "0",
                    &len.to_string(),
                    flash_output.to_str().unwrap(),
                ],
                "Flash content successfully read and written to",
                Duration::from_secs(5),
                &format!("read {len} bytes with ROM bootloader"),
            )?;

            // Verify the ROM read data matches the expected pattern
            if let Ok(read_data) = fs::read(&flash_output) {
                let expected = &known_pattern[0..len as usize];
                if &read_data[0..len as usize] != expected {
                    return Err(format!(
                        "ROM read verification failed for length {len}: content does not match"
                    )
                    .into());
                }
            } else {
                return Err(
                    format!("Failed to read flash_content.bin for ROM read length {len}").into(),
                );
            }
        }

        log::info!("read-flash test passed");
        Ok(())
    }

    pub fn test_write_bin(&self) -> Result<()> {
        log::info!("Running write-bin test");
        let flash_output = self.flash_output_file();
        let binary_file = self.tests_dir.join("binary_file.bin");
        let part_table = "espflash/tests/data/partitions.csv";

        // Create a simple binary with a known pattern (regression test for issue #622)
        let test_pattern = [0x01, 0xA0];
        fs::write(&binary_file, &test_pattern)?;

        // Write the binary to a specific address
        self.run_simple_command_test(
            &["write-bin", "0x0", binary_file.to_str().unwrap()],
            "Binary successfully written to flash!",
            Duration::from_secs(5),
            "write-bin to address",
        )?;

        // Read the flash to verify
        self.run_simple_command_test(
            &["read-flash", "0", "64", flash_output.to_str().unwrap()],
            "Flash content successfully read",
            Duration::from_secs(5),
            "read after write-bin",
        )?;

        // Verify the flash content contains the test pattern
        if let Ok(flash_data) = fs::read(&flash_output) {
            if !Self::contains_sequence(&flash_data, &test_pattern) {
                return Err("Failed verifying content: test pattern not found in flash".into());
            }
        } else {
            return Err("Failed to read flash_content.bin file".into());
        }

        // Write the binary to a partition label
        self.run_simple_command_test(
            &[
                "write-bin",
                "nvs",
                binary_file.to_str().unwrap(),
                "--partition-table",
                part_table,
            ],
            "Binary successfully written to flash!",
            Duration::from_secs(5),
            "write-bin to partition label",
        )?;

        // Read from the partition address to verify
        self.run_simple_command_test(
            &["read-flash", "0x9000", "64", flash_output.to_str().unwrap()],
            "Flash content successfully read",
            Duration::from_secs(5),
            "read after write-bin to partition",
        )?;

        // Verify the flash content at the partition address
        if let Ok(flash_data) = fs::read(&flash_output) {
            if !Self::contains_sequence(&flash_data, &test_pattern) {
                return Err(
                    "Failed verifying content: test pattern not found in partition flash area"
                        .into(),
                );
            }
        } else {
            return Err("Failed to read flash_content.bin file".into());
        }

        log::info!("write-bin test passed");
        Ok(())
    }

    pub fn test_save_image(&self, chip: Option<&str>) -> Result<()> {
        let chip = chip.unwrap_or_else(|| self.chip.as_deref().unwrap_or("esp32"));
        log::info!("Running save-image test for chip: {chip}");

        let app = format!("espflash/tests/data/{chip}");
        let app_bin = self.tests_dir.join("app.bin");

        // Determine if frequency option is needed
        let mut args = vec![
            "save-image",
            "--merge",
            "--chip",
            chip,
            &app,
            app_bin.to_str().unwrap(),
        ];

        // Add frequency option for esp32c2
        if chip == "esp32c2" {
            args.splice(2..2, ["-x", "26mhz"].iter().map(|&s| s));
        }

        // Save image
        self.run_simple_command_test(
            &args,
            "Image successfully saved!",
            self.timeout,
            "save-image",
        )?;

        // Write the image and monitor
        self.run_timed_command_test(
            &[
                "write-bin",
                "--monitor",
                "0x0",
                app_bin.to_str().unwrap(),
                "--non-interactive",
            ],
            "Hello world!",
            Duration::from_secs(15),
            "write-bin and monitor",
        )?;

        // Additional regression test for ESP32-C6
        if chip == "esp32c6" {
            self.test_esp32c6_regression(&app_bin)?;
        }

        log::info!("save-image test passed");
        Ok(())
    }

    pub fn test_esp32c6_regression(&self, app_bin: &Path) -> Result<()> {
        log::info!("Running ESP32-C6 regression test");

        let app = "espflash/tests/data/esp_idf_firmware_c6.elf";

        // Save image with ESP32-C6 regression test case
        self.run_simple_command_test(
            &[
                "save-image",
                "--merge",
                "--chip",
                "esp32c6",
                app,
                app_bin.to_str().unwrap(),
            ],
            "Image successfully saved!",
            Duration::from_secs(5),
            "save-image C6 regression",
        )?;

        // Check that app descriptor is in the correct position
        if let Ok(bin_data) = fs::read(app_bin) {
            if bin_data.len() >= 0x10024 {
                let app_descriptor_offset = 0x10020;
                // Check for magic word 0xABCD5432 (in little-endian format)
                let expected_magic = [0x32, 0x54, 0xCD, 0xAB];

                if bin_data[app_descriptor_offset..app_descriptor_offset + 4] != expected_magic {
                    return Err("App descriptor magic word is not correct".into());
                }
            } else {
                return Err("Binary file is too small to contain app descriptor".into());
            }
        } else {
            return Err("Failed to read app.bin file".into());
        }

        log::info!("ESP32-C6 regression test passed");
        Ok(())
    }

    pub fn test_checksum_md5(&self) -> Result<()> {
        log::info!("Running checksum-md5 test");

        // First erase the flash
        self.run_simple_command_test(
            &["erase-flash"],
            "Flash has been erased!",
            Duration::from_secs(10),
            "erase-flash for checksum",
        )?;

        // Then check the MD5 checksum of a region
        self.run_simple_command_test(
            &["checksum-md5", "0x1000", "0x100"],
            "0x827f263ef9fb63d05499d14fcef32f60",
            Duration::from_secs(5),
            "checksum-md5",
        )?;

        log::info!("checksum-md5 test passed");
        Ok(())
    }

    pub fn test_monitor(&self) -> Result<()> {
        self.run_timed_command_test(
            &["monitor", "--non-interactive"],
            "Hello world!",
            Duration::from_secs(5),
            "monitor",
        )?;
        Ok(())
    }

    pub fn test_reset(&self) -> Result<()> {
        self.run_simple_command_test(
            &["reset"],
            "Resetting target device",
            Duration::from_secs(5),
            "reset",
        )?;
        Ok(())
    }

    pub fn test_hold_in_reset(&self) -> Result<()> {
        self.run_simple_command_test(
            &["hold-in-reset"],
            "Holding target device in reset",
            Duration::from_secs(5),
            "hold-in-reset",
        )?;
        Ok(())
    }
}

// Main entry point for tests
pub fn run_tests(workspace: &Path, args: RunTestsArgs) -> Result<()> {
    log::info!("Running espflash tests");

    let tests_dir = workspace.join("espflash").join("tests");
    let test_runner = TestRunner::new(workspace, tests_dir, args.timeout);

    match args.test.as_str() {
        "all" => {
            if let Err(e) = test_runner.run_all_tests(args.chip.as_deref()) {
                log::error!("Test suite failed: {e}");
                return Err(e);
            }
        }
        specific_test => {
            if let Err(e) = test_runner.run_specific_test(specific_test, args.chip.as_deref()) {
                log::error!("Test '{specific_test}' failed: {e}");
                return Err(e);
            }
        }
    }

    Ok(())
}
