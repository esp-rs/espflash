use std::{
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use clap::{ArgAction, Args};
use log::info;

use crate::Result;

type SpawnedCommandOutput = (
    Child,
    Arc<Mutex<String>>,
    thread::JoinHandle<()>,
    thread::JoinHandle<()>,
);
/// Arguments for running tests
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

    /// Whether to build espflash before running tests, true by default
    #[arg(long = "no-build", action = ArgAction::SetFalse, default_value_t = true)]
    pub build_espflash: bool,

    /// Flag to run SDM HIL tests
    #[arg(long = "sdm", action = ArgAction::SetTrue, default_value_t = false)]
    pub sdm: bool,
}

/// A struct to manage and run tests for the espflash
pub struct TestRunner {
    /// The workspace directory where the tests are located
    pub workspace: PathBuf,
    /// The directory containing the test files
    pub tests_dir: PathBuf,
    /// Timeout for test commands
    pub timeout: Duration,
    /// Optional chip target for tests
    pub chip: Option<String>,
    /// Build espflash before running tests
    pub build_espflash: bool,
}

impl TestRunner {
    /// Creates a new [TestRunner] instance
    pub fn new(
        workspace: &Path,
        tests_dir: PathBuf,
        timeout_secs: u64,
        build_espflash: bool,
    ) -> Self {
        Self {
            workspace: workspace.to_path_buf(),
            tests_dir,
            timeout: Duration::from_secs(timeout_secs),
            chip: None,
            build_espflash,
        }
    }

    fn setup_command(&self, cmd: &mut Command) {
        cmd.current_dir(&self.workspace)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());
    }

    fn terminate_process(child: &mut Option<&mut Child>) {
        if let Some(child_proc) = child {
            let _ = child_proc.kill();

            // Wait for the process to terminate
            if let Some(child_proc) = child {
                let _ = child_proc.wait();
            }
        }
    }

    fn restore_terminal() {
        #[cfg(unix)]
        {
            let _ = Command::new("stty").arg("sane").status();
        }
    }

    fn spawn_and_capture_output(cmd: &mut Command) -> Result<SpawnedCommandOutput> {
        info!("Spawning command: {cmd:?}");
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = cmd.spawn()?;
        let stdout = child.stdout.take().expect("Failed to capture stdout");
        let stderr = child.stderr.take().expect("Failed to capture stderr");

        let output = Arc::new(Mutex::new(String::new()));
        let out_clone1 = Arc::clone(&output);
        let out_clone2 = Arc::clone(&output);

        let stdout_handle = thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(|line| line.ok()) {
                println!("{line}");
                out_clone1.lock().unwrap().push_str(&line);
                out_clone1.lock().unwrap().push('\n');
            }
        });

        let stderr_handle = thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines().map_while(|line| line.ok()) {
                println!("{line}");
                out_clone2.lock().unwrap().push_str(&line);
                out_clone2.lock().unwrap().push('\n');
            }
        });

        Ok((child, output, stdout_handle, stderr_handle))
    }

    fn run_command_capture_output_with_timeout(
        cmd: &mut Command,
        timeout: Duration,
        test_name: &str,
    ) -> Result<String> {
        let (mut child, output, h1, h2) = Self::spawn_and_capture_output(cmd)?;
        let start_time = Instant::now();
        let grace = Duration::from_millis(500);
        let mut terminated_naturally = false;

        while start_time.elapsed() < timeout + grace {
            if let Ok(Some(_)) = child.try_wait() {
                terminated_naturally = true;
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }

        if !terminated_naturally && let Ok(Some(_)) = child.try_wait() {
            terminated_naturally = true;
        }

        if !terminated_naturally {
            log::warn!("{test_name} test timed out after {timeout:?}, terminating process");
            let _ = child.kill();
            let _ = child.wait();
        }

        let _ = h1.join();
        let _ = h2.join();

        let output = output.lock().unwrap();
        Ok(output.clone())
    }

    /// Runs a command with a timeout, returning the exit code
    pub fn run_command_with_timeout(&self, cmd: &mut Command, timeout: Duration) -> Result<i32> {
        log::debug!("Running command: {cmd:?}");
        self.setup_command(cmd);

        let mut child = cmd.spawn()?;
        let completed = Arc::new(AtomicBool::new(false));
        let child_id = child.id();
        let completed_clone = Arc::clone(&completed);

        let timer = thread::spawn(move || {
            let interval = Duration::from_millis(100);
            let mut elapsed = Duration::ZERO;

            while elapsed < timeout {
                thread::sleep(interval);
                elapsed += interval;
                if completed_clone.load(Ordering::SeqCst) {
                    return;
                }
            }

            log::warn!("Command timed out after {timeout:?}, killing process {child_id}");
            Self::terminate_process(&mut None);
        });

        let status = match child.wait() {
            Ok(s) => {
                completed.store(true, Ordering::SeqCst);
                s
            }
            Err(e) => {
                completed.store(true, Ordering::SeqCst);
                thread::sleep(Duration::from_millis(10));
                return Err(format!("Command execution failed: {e}").into());
            }
        };

        let _ = timer.join();
        let exit_code = status.code().unwrap_or(1);
        log::debug!("Command exit code: {exit_code}");
        Ok(exit_code)
    }

    /// Runs a command for a specified duration, returning whether it terminated
    /// naturally
    pub fn run_command_for(&self, cmd: &mut Command, duration: Duration) -> Result<bool> {
        log::debug!("Running command: {cmd:?}");
        let mut child = cmd.spawn()?;
        let start_time = Instant::now();
        let mut naturally_terminated = false;

        if let Ok(Some(_)) = child.try_wait() {
            naturally_terminated = true;
        } else {
            thread::sleep(duration);
            if let Ok(Some(_)) = child.try_wait() {
                naturally_terminated = true;
            }
        }

        if !naturally_terminated {
            log::info!(
                "Command ran for {:?}, terminating process {}",
                start_time.elapsed(),
                child.id()
            );
            Self::terminate_process(&mut Some(&mut child));
        }

        Self::restore_terminal();
        log::debug!("Command completed after {:?}", start_time.elapsed());

        Ok(naturally_terminated)
    }

    fn build_espflash(&self) {
        let mut cmd = Command::new("cargo");

        log::info!("Building espflash...");
        cmd.args(["build", "-p", "espflash", "--release", "--"]);

        let status = cmd.status().expect("Failed to build espflash");
        if !status.success() {
            panic!("espflash build failed with status: {status}");
        }
    }

    fn create_espflash_command(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new("cargo");

        // we need to distinguish between local and CI runs, on CI we are building
        // espflash and then copying the binary, so we can use just `espflash`
        match self.build_espflash {
            true => {
                log::info!("Running cargo run...");
                cmd.args(["run", "-p", "espflash", "--release", "--quiet", "--"]);
            }
            false => {
                log::info!("Using system espflash");
                let mut cmd = Command::new("espflash");
                cmd.args(args);
                return cmd;
            }
        }

        cmd.args(args);

        cmd
    }

    /// Runs a simple command test, capturing output and checking for expected
    /// outputs
    pub fn run_simple_command_test(
        &self,
        args: &[&str],
        expected_contains: Option<&[&str]>,
        timeout: Duration,
        test_name: &str,
    ) -> Result<()> {
        log::info!("Running {test_name} test");
        let mut cmd = self.create_espflash_command(args);

        if let Some(expected) = expected_contains {
            let output =
                Self::run_command_capture_output_with_timeout(&mut cmd, timeout, test_name)?;
            for &expected in expected {
                if !output.contains(expected) {
                    Self::restore_terminal();
                    return Err(format!("Missing expected output: {expected}").into());
                }
            }

            log::info!("{test_name} test passed and output verified");
        } else {
            let exit_code = self.run_command_with_timeout(&mut cmd, timeout)?;
            if exit_code != 0 {
                return Err(
                    format!("{test_name} test failed: non-zero exit code {exit_code}").into(),
                );
            }

            log::info!("{test_name} test passed with exit code 0");
        }

        Ok(())
    }

    /// Runs a timed command test, capturing output and checking for expected
    /// outputs after a specified duration
    pub fn run_timed_command_test(
        &self,
        args: &[&str],
        expected_contains: Option<&[&str]>,
        duration: Duration,
        test_name: &str,
    ) -> Result<()> {
        log::info!("Running {test_name} test");
        let mut cmd = self.create_espflash_command(args);

        if let Some(expected) = expected_contains {
            let (mut child, output, h1, h2) = Self::spawn_and_capture_output(&mut cmd)?;
            thread::sleep(duration);
            let _ = child.kill();
            let _ = child.wait();
            let _ = h1.join();
            let _ = h2.join();

            let output = output.lock().unwrap();
            for &expected in expected {
                if !output.contains(expected) {
                    Self::restore_terminal();
                    return Err(format!("Missing expected output: {expected}").into());
                }
            }

            log::info!("{test_name} test passed and output verified");
        } else {
            let terminated_naturally = self.run_command_for(&mut cmd, duration)?;
            log::info!("{test_name} test completed (terminated naturally: {terminated_naturally})");
        }

        Self::restore_terminal();
        Ok(())
    }

    fn is_flash_empty(&self, file_path: &Path) -> Result<bool> {
        let flash_data = fs::read(file_path)?;
        Ok(flash_data.iter().all(|&b| b == 0xFF))
    }

    fn flash_output_file(&self) -> PathBuf {
        self.tests_dir.join("flash_content.bin")
    }

    /// Runs all tests in the test suite, optionally overriding the chip target
    pub fn run_all_tests(&self, chip_override: Option<&str>, sdm: bool) -> Result<()> {
        log::info!("Running all tests");

        let chip = chip_override.or(self.chip.as_deref()).unwrap_or("esp32");

        if sdm {
            self.test_board_info()?;
            self.test_save_image_write_bin(Some(chip))?;
            self.test_hold_in_reset()?;
            self.test_reset()?;
            self.test_list_ports()?;
            self.test_flash(Some(chip))?;
            self.test_monitor()?;
        } else {
            self.test_board_info()?;
            self.test_erase_flash()?;
            self.test_save_image_write_bin(Some(chip))?;
            self.test_erase_region()?;
            self.test_hold_in_reset()?;
            self.test_reset()?;
            self.test_list_ports()?;
            self.test_checksum_md5()?;
            self.test_read_flash()?;
            self.test_flash(Some(chip))?;
            self.test_monitor()?;
        }

        log::info!("All tests completed successfully");
        Ok(())
    }

    /// Runs a specific test by name, optionally overriding the chip target
    pub fn run_specific_test(
        &self,
        test_name: &str,
        chip_override: Option<&str>,
        sdm: bool,
    ) -> Result<()> {
        let chip = chip_override.or(self.chip.as_deref()).unwrap_or("esp32");

        if sdm {
            return match test_name {
                "board-info" => self.test_board_info(),
                "save-image" | "write-bin" | "save-image-write-bin" => {
                    self.test_save_image_write_bin(Some(chip))
                }
                "hold-in-reset" => self.test_hold_in_reset(),
                "reset" => self.test_reset(),
                "list-ports" => self.test_list_ports(),
                "monitor" => self.test_monitor(),
                _ => Err(format!("Unknown or unsupported SDM test: {test_name}").into()),
            };
        }

        match test_name {
            "board-info" => self.test_board_info(),
            "flash" => self.test_flash(Some(chip)),
            "monitor" => self.test_monitor(),
            "erase-flash" => self.test_erase_flash(),
            "save-image" | "write-bin" | "save-image-write-bin" => {
                self.test_save_image_write_bin(Some(chip))
            }
            "erase-region" => self.test_erase_region(),
            "hold-in-reset" => self.test_hold_in_reset(),
            "reset" => self.test_reset(),
            "checksum-md5" => self.test_checksum_md5(),
            "list-ports" => self.test_list_ports(),
            "read-flash" => self.test_read_flash(),
            _ => Err(format!("Unknown test: {test_name}").into()),
        }
    }

    // Board info test
    pub fn test_board_info(&self) -> Result<()> {
        self.run_simple_command_test(
            &["board-info"],
            Some(&["Chip type:"]),
            Duration::from_secs(10),
            "board-info",
        )
    }

    // Flash test
    pub fn test_flash(&self, chip: Option<&str>) -> Result<()> {
        let chip = chip.unwrap_or_else(|| self.chip.as_deref().unwrap_or("esp32"));
        log::info!("Running flash test for chip: {chip}");

        let app = format!("espflash/tests/data/{chip}");
        let app_backtrace = format!("espflash/tests/data/{chip}_backtrace");
        let part_table = "espflash/tests/data/partitions.csv";

        // Partition table is too big
        self.run_timed_command_test(
            &[
                "flash",
                "--no-skip",
                "--monitor",
                "--non-interactive",
                &app,
                "--flash-size",
                "2mb",
                "--partition-table",
                part_table,
            ],
            Some(&["espflash::partition_table::does_not_fit"]),
            Duration::from_secs(10),
            "partition too big",
        )?;

        // Additional tests for ESP32-C6 with manual log-format
        if chip == "esp32c6" {
            // Test with manual log-format and with auto-detected log-format
            self.test_flash_with_defmt(&app)?;
            // Backtrace test
            self.test_backtrace(&app_backtrace)?;
        }

        // Test standard flashing
        self.run_timed_command_test(
            &["flash", "--no-skip", "--monitor", "--non-interactive", &app],
            Some(&["Flashing has completed!", "Hello world!"]),
            Duration::from_secs(15),
            "standard flashing",
        )?;

        // Test standard flashing
        self.run_timed_command_test(
            &[
                "flash",
                "--no-skip",
                "--monitor",
                "--non-interactive",
                "--baud",
                "921600",
                &app,
            ],
            Some(&["Flashing has completed!", "Hello world!"]),
            Duration::from_secs(15),
            "standard flashing with high baud rate",
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
            Some(&["Flashing has completed!", "Hello world!"]),
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
            Some(&["Flashing has completed!", "Hello world!"]),
            Duration::from_secs(15),
            "defmt auto-detected log-format",
        )?;

        Ok(())
    }

    fn test_backtrace(&self, app_backtrace: &str) -> Result<()> {
        // Test flashing with backtrace
        self.run_timed_command_test(
            &[
                "flash",
                "--no-skip",
                "--monitor",
                "--non-interactive",
                app_backtrace,
            ],
            Some(&[
                "0x420012c8",
                "main",
                "esp32c6_backtrace/src/bin/main.rs:",
                "0x42001280",
                "hal_main",
            ]),
            Duration::from_secs(15),
            "backtrace test",
        )?;

        Ok(())
    }

    /// Tests listing available ports
    pub fn test_list_ports(&self) -> Result<()> {
        log::info!("Running list-ports test");
        let mut cmd = self.create_espflash_command(&["list-ports"]);
        let timeout = Duration::from_secs(10);
        let output =
            Self::run_command_capture_output_with_timeout(&mut cmd, timeout, "list-ports")?;
        // Accept either "Silicon Labs" or "Espressif" in the output
        if !output.contains("Silicon Labs") && !output.contains("Espressif") {
            Self::restore_terminal();
            return Err(
                "Missing expected output: neither 'Silicon Labs' nor 'Espressif' found".into(),
            );
        }

        log::info!("list-ports test passed and output verified");
        Ok(())
    }

    /// Tests erasing the flash memory
    pub fn test_erase_flash(&self) -> Result<()> {
        log::info!("Running erase-flash test");
        let flash_output = self.flash_output_file();

        self.run_simple_command_test(
            &["erase-flash"],
            Some(&["Flash has been erased!"]),
            Duration::from_secs(40),
            "erase-flash",
        )?;

        // Read a portion of the flash to verify it's erased
        self.run_simple_command_test(
            &["read-flash", "0", "0x4000", flash_output.to_str().unwrap()],
            Some(&["Flash content successfully read"]),
            Duration::from_secs(10),
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

    /// Tests erasing a specific region of the flash memory
    pub fn test_erase_region(&self) -> Result<()> {
        log::info!("Running erase-region test");
        let flash_output = self.flash_output_file();

        // Test unaligned address (not multiple of 4096)
        let mut cmd = self.create_espflash_command(&["erase-region", "0x1001", "0x1000"]);
        let exit_code = self.run_command_with_timeout(&mut cmd, Duration::from_secs(10))?;
        if exit_code == 0 {
            return Err("Unaligned address erase should have failed but succeeded".into());
        }

        // Test unaligned size (not multiple of 4096)
        let mut cmd = self.create_espflash_command(&["erase-region", "0x1000", "0x1001"]);
        let exit_code = self.run_command_with_timeout(&mut cmd, Duration::from_secs(10))?;
        if exit_code == 0 {
            return Err("Unaligned size erase should have failed but succeeded".into());
        }

        // Valid erase - should succeed
        self.run_simple_command_test(
            &["erase-region", "0x1000", "0x1000"],
            Some(&["Erasing region at"]),
            Duration::from_secs(20),
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
            Some(&["Flash content successfully read"]),
            Duration::from_secs(20),
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

    /// Tests reading the flash memory
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
            Some(&["Binary successfully written to flash!"]),
            Duration::from_secs(10),
            "write pattern",
        )?;

        // Test reading various lengths
        for &len in &[2, 5, 10, 26] {
            log::info!("Testing read-flash with length: {len}");

            // Test normal read
            self.run_simple_command_test(
                &[
                    "read-flash",
                    "0x0",
                    &len.to_string(),
                    flash_output.to_str().unwrap(),
                ],
                Some(&["Flash content successfully read and written to"]),
                Duration::from_secs(10),
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
                    "0x0",
                    &len.to_string(),
                    flash_output.to_str().unwrap(),
                ],
                Some(&["Flash content successfully read and written to"]),
                Duration::from_secs(10),
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

    /// Tests saving an image to the flash memory
    pub fn test_save_image_write_bin(&self, chip: Option<&str>) -> Result<()> {
        let chip = chip.unwrap_or_else(|| self.chip.as_deref().unwrap_or("esp32"));
        log::info!("Running save-image and write-bin test for chip: {chip}");

        let app = format!("espflash/tests/data/{chip}");
        let app_bin = self.tests_dir.join("app.bin");

        // Test the `--merge` option
        let mut args = vec![
            "save-image",
            "--merge",
            "--skip-padding",
            "--chip",
            chip,
            &app,
            app_bin.to_str().unwrap(),
        ];

        // Add frequency option for esp32c2
        if chip == "esp32c2" {
            args.extend(["-x", "26mhz"]);
        }

        // Save image
        self.run_simple_command_test(
            &args,
            Some(&["Image successfully saved!"]),
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
            Some(&["Hello world!"]),
            Duration::from_secs(80),
            "write-bin and monitor",
        )?;

        // Only save the app image
        let mut args = vec![
            "save-image",
            "--chip",
            chip,
            &app,
            app_bin.to_str().unwrap(),
        ];

        // Add frequency option for esp32c2
        if chip == "esp32c2" {
            args.extend(["-x", "26mhz"]);
        }

        // Save image
        self.run_simple_command_test(
            &args,
            Some(&["Image successfully saved!"]),
            self.timeout,
            "save-image",
        )?;

        // Write the image and monitor
        self.run_timed_command_test(
            &[
                "write-bin",
                "--monitor",
                "0x10000",
                app_bin.to_str().unwrap(),
                "--non-interactive",
            ],
            Some(&["Hello world!"]),
            Duration::from_secs(80),
            "write-bin and monitor",
        )?;

        // Additional regression test for ESP32-C6
        if chip == "esp32c6" {
            self.test_esp32c6_regression(&app_bin)?;
        }

        log::info!("save-image test passed");
        Ok(())
    }

    /// Tests the ESP32-C6 regression case
    fn test_esp32c6_regression(&self, app_bin: &Path) -> Result<()> {
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
            Some(&["Image successfully saved!"]),
            Duration::from_secs(10),
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

    /// Tests the MD5 checksum command
    pub fn test_checksum_md5(&self) -> Result<()> {
        log::info!("Running checksum-md5 test");

        // First erase the flash
        self.run_simple_command_test(
            &["erase-flash"],
            Some(&["Flash has been erased!"]),
            Duration::from_secs(40),
            "erase-flash for checksum",
        )?;

        // Then check the MD5 checksum of a region
        self.run_simple_command_test(
            &["checksum-md5", "0x1000", "0x100"],
            Some(&["0x827f263ef9fb63d05499d14fcef32f60"]),
            Duration::from_secs(10),
            "checksum-md5",
        )?;

        log::info!("checksum-md5 test passed");
        Ok(())
    }

    /// Tests the monitor command
    pub fn test_monitor(&self) -> Result<()> {
        self.run_timed_command_test(
            &["monitor", "--non-interactive"],
            Some(&["Hello world!"]),
            Duration::from_secs(10),
            "monitor",
        )?;
        Ok(())
    }

    /// Tests resetting the target device
    pub fn test_reset(&self) -> Result<()> {
        self.run_simple_command_test(
            &["reset"],
            Some(&["Resetting target device"]),
            Duration::from_secs(10),
            "reset",
        )?;
        Ok(())
    }

    /// Tests holding the target device in reset
    pub fn test_hold_in_reset(&self) -> Result<()> {
        self.run_simple_command_test(
            &["hold-in-reset"],
            Some(&["Holding target device in reset"]),
            Duration::from_secs(10),
            "hold-in-reset",
        )?;
        Ok(())
    }
}

/// Runs the tests based on the provided arguments
pub fn run_tests(workspace: &Path, args: RunTestsArgs) -> Result<()> {
    log::info!("Running espflash tests");

    let tests_dir = workspace.join("espflash").join("tests");
    let test_runner = TestRunner::new(workspace, tests_dir, args.timeout, args.build_espflash);

    // Build espflash before running test(s) so we are not "waisting" test's
    // duration or timeout
    if args.build_espflash {
        test_runner.build_espflash();
    }

    match args.test.as_str() {
        "all" => {
            if let Err(e) = test_runner.run_all_tests(args.chip.as_deref(), args.sdm) {
                log::error!("Test suite failed: {e}");
                return Err(e);
            }
        }
        specific_test => {
            if let Err(e) =
                test_runner.run_specific_test(specific_test, args.chip.as_deref(), args.sdm)
            {
                log::error!("Test '{specific_test}' failed: {e}");
                return Err(e);
            }
        }
    }

    Ok(())
}
