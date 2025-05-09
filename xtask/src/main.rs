use std::path::PathBuf;

use clap::Parser;

// Import modules
mod efuse_generator;
mod test_runner;

// Type definition for results
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

// ----------------------------------------------------------------------------
// Command-line Interface

#[derive(Debug, Parser)]
enum Cli {
    /// Generate eFuse field definitions
    GenerateEfuseFields(efuse_generator::GenerateEfuseFieldsArgs),

    /// Run espflash tests (replacing bash scripts)
    RunTests(test_runner::RunTestsArgs),
}

// ----------------------------------------------------------------------------
// Application

fn main() -> Result<()> {
    env_logger::Builder::new()
        .filter_module("xtask", log::LevelFilter::Info)
        .init();

    // The directory containing the cargo manifest for the 'xtask' package is a
    // subdirectory within the cargo workspace:
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace = workspace.parent().unwrap().canonicalize()?;

    match Cli::parse() {
        Cli::GenerateEfuseFields(args) => efuse_generator::generate_efuse_fields(&workspace, args),
        Cli::RunTests(args) => test_runner::run_tests(&workspace, args),
    }
}

// ----------------------------------------------------------------------------
// Run Tests

fn run_tests(workspace: &Path, args: RunTestsArgs) -> Result<()> {
    log::info!("Running espflash tests");

    let tests_dir = workspace.join("espflash").join("tests");
    let test_runner = TestRunner::new(workspace, tests_dir, args.timeout);

    match args.test.as_str() {
        "all" => {
            if let Err(e) = test_runner.run_all_tests(args.chip.as_deref()) {
                log::error!("Test suite failed: {}", e);
                return Err(e);
            }
        }
        specific_test => {
            if let Err(e) = test_runner.run_specific_test(specific_test, args.chip.as_deref()) {
                log::error!("Test '{}' failed: {}", specific_test, e);
                return Err(e);
            }
        }
    }

    Ok(())
}

struct TestRunner {
    workspace: PathBuf,
    tests_dir: PathBuf,
    timeout: Duration,
    flash_timeout: Duration, // Longer timeout for flash operations
    chip: Option<String>,    // Chip under test
}

impl TestRunner {
    fn new(workspace: &Path, tests_dir: PathBuf, timeout_secs: u64) -> Self {
        Self {
            workspace: workspace.to_path_buf(),
            tests_dir,
            timeout: Duration::from_secs(timeout_secs),
            flash_timeout: Duration::from_secs(timeout_secs * 2), /* Double timeout for flash
                                                                   * operations */
            chip: None,
        }
    }

    fn run_all_tests(&self, chip_override: Option<&str>) -> Result<()> {
        log::info!("Running all tests");

        let chip = chip_override.or(self.chip.as_deref()).unwrap_or("esp32");

        // Run all individual tests in the same order as the HIL workflow
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

    fn run_specific_test(&self, test_name: &str, chip_override: Option<&str>) -> Result<()> {
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
            _ => Err(format!("Unknown test: {}", test_name).into()),
        }
    }

    // Run command utilities remain the same
    fn run_command_with_timeout(
        &self,
        cmd: &mut Command,
        timeout: Duration,
    ) -> Result<(String, i32)> {
        use std::{
            io::{BufRead, BufReader},
            process::{Child, Stdio},
            sync::{Arc, Mutex},
            thread,
        };

        log::debug!("Running command: {:?}", cmd);

        // Configure the command to use pipes for stdout and stderr
        cmd.current_dir(&self.workspace)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Start the command
        let mut child: Child = cmd.spawn()?;

        // Set up stdout and stderr readers
        let stdout = child.stdout.take().expect("Failed to capture stdout");
        let stderr = child.stderr.take().expect("Failed to capture stderr");

        // Collect output in thread-safe containers
        let stdout_output = Arc::new(Mutex::new(Vec::new()));
        let stderr_output = Arc::new(Mutex::new(Vec::new()));

        // Clone for use in threads
        let stdout_clone = Arc::clone(&stdout_output);
        let stderr_clone = Arc::clone(&stderr_output);

        // Thread for reading stdout
        let stdout_thread = thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(|r| r.ok()) {
                stdout_clone.lock().unwrap().push(line);
            }
        });

        // Thread for reading stderr
        let stderr_thread = thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines().map_while(|r| r.ok()) {
                stderr_clone.lock().unwrap().push(line);
            }
        });

        // Create a thread to handle the timeout
        let child_id = child.id();
        let timer_handle = thread::spawn(move || {
            thread::sleep(timeout);
            // If we reach this point, the command didn't complete within the timeout
            log::warn!(
                "Command timed out after {:?}, killing process {}",
                timeout,
                child_id
            );
            #[cfg(unix)]
            unsafe {
                libc::kill(child_id as i32, libc::SIGTERM);
            }
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                unsafe {
                    winapi::um::processthreadsapi::TerminateProcess(
                        child.as_raw_handle() as *mut winapi::ctypes::c_void,
                        1,
                    );
                }
            }
        });

        // Wait for the command to complete
        let status = match child.wait() {
            Ok(status) => status,
            Err(e) => {
                // The command failed - let's make sure the timer thread doesn't continue
                // running This is a best effort, as the thread might already
                // have terminated the process
                drop(timer_handle);
                return Err(format!("Command execution failed: {}", e).into());
            }
        };

        // If we get here, the command completed before the timeout
        // We can safely drop the timer thread handle - it will either exit normally or
        // terminate
        drop(timer_handle);

        // Wait for the stdout and stderr threads to finish
        stdout_thread.join().unwrap();
        stderr_thread.join().unwrap();

        // Combine the outputs
        let stdout_str = {
            let lines = stdout_output.lock().unwrap();
            lines.join("\n")
        };

        let stderr_str = {
            let lines = stderr_output.lock().unwrap();
            lines.join("\n")
        };

        let combined_output = format!("{}\n{}", stdout_str, stderr_str);

        log::debug!("Command exit code: {:?}", status.code());

        Ok((combined_output, status.code().unwrap_or(1)))
    }

    /// Run a command for a specified duration and then return whatever output
    /// has been produced. Unlike run_command_with_timeout, this does not
    /// treat non-termination as an error. It will run the command for the
    /// specified duration, collect the output, then terminate the process
    /// and return the output that was produced during that time.
    fn run_command_for(&self, cmd: &mut Command, duration: Duration) -> Result<(String, bool)> {
        use std::{
            io::{BufRead, BufReader},
            process::{Child, Stdio},
            sync::{Arc, Mutex},
            thread,
            time::Instant,
        };

        log::debug!("Running command for {:?}: {:?}", duration, cmd);

        // Configure the command to use pipes for stdout and stderr
        cmd.current_dir(&self.workspace)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Start the command
        let mut child: Child = cmd.spawn()?;

        // Set up stdout and stderr readers
        let stdout = child.stdout.take().expect("Failed to capture stdout");
        let stderr = child.stderr.take().expect("Failed to capture stderr");

        // Collect output in thread-safe containers
        let stdout_output = Arc::new(Mutex::new(Vec::new()));
        let stderr_output = Arc::new(Mutex::new(Vec::new()));

        // Clone for use in threads
        let stdout_clone = Arc::clone(&stdout_output);
        let stderr_clone = Arc::clone(&stderr_output);

        // Thread for reading stdout
        let stdout_thread = thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(|r| r.ok()) {
                stdout_clone.lock().unwrap().push(line);
            }
        });

        // Thread for reading stderr
        let stderr_thread = thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines().map_while(|r| r.ok()) {
                stderr_clone.lock().unwrap().push(line);
            }
        });

        // Keep track of whether the process terminated naturally
        let start_time = Instant::now();
        let mut terminated_naturally = false;

        // Wait for either the command to complete or the duration to expire
        if let Ok(status) = child.try_wait() {
            if status.is_some() {
                // Process already completed before the duration elapsed
                terminated_naturally = true;
                log::info!("Command terminated naturally with status: {:?}", status);
            } else {
                // Process still running, wait for duration
                thread::sleep(duration);

                // Check again if it completed on its own during our sleep
                if let Ok(status) = child.try_wait() {
                    if status.is_some() {
                        terminated_naturally = true;
                        log::info!("Command terminated naturally with status: {:?}", status);
                    }
                }
            }
        }

        // If process is still running after duration, terminate it
        if !terminated_naturally {
            let elapsed = start_time.elapsed();
            log::info!(
                "Command ran for {:?}, terminating process {}",
                elapsed,
                child.id()
            );

            #[cfg(unix)]
            unsafe {
                libc::kill(child.id() as i32, libc::SIGTERM);
            }
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                unsafe {
                    winapi::um::processthreadsapi::TerminateProcess(
                        child.as_raw_handle() as *mut winapi::ctypes::c_void,
                        1,
                    );
                }
            }

            // Give it a moment to actually terminate
            let _ = child.wait();
        }

        // Wait for the stdout and stderr threads to finish
        let _ = stdout_thread.join();
        let _ = stderr_thread.join();

        // Combine the outputs
        let stdout_str = {
            let lines = stdout_output.lock().unwrap();
            lines.join("\n")
        };

        let stderr_str = {
            let lines = stderr_output.lock().unwrap();
            lines.join("\n")
        };

        let combined_output = format!("{}\n{}", stdout_str, stderr_str);

        log::debug!("Command output collected after {:?}", start_time.elapsed());

        Ok((combined_output, terminated_naturally))
    }

    // Helper methods for common test patterns

    /// Run a simple command test that checks for successful completion by
    /// examining exit code and expected output
    fn run_simple_command_test(
        &self,
        args: &[&str],
        expected_output: &str,
        timeout: Duration,
        test_name: &str,
    ) -> Result<String> {
        log::info!("Running {} test", test_name);

        let mut cmd = Command::new("espflash");
        cmd.args(args);

        let (output, exit_code) = self.run_command_with_timeout(&mut cmd, timeout)?;
        log::debug!("Output: {}", output);

        if exit_code != 0 || !output.contains(expected_output) {
            return Err(format!(
                "{} test failed: expected output containing '{}'",
                test_name, expected_output
            )
            .into());
        }

        log::info!("{} test passed", test_name);
        Ok(output)
    }

    /// Run a command for a fixed duration and check if expected output appears
    fn run_timed_command_test(
        &self,
        args: &[&str],
        expected_output: &str,
        duration: Duration,
        test_name: &str,
    ) -> Result<String> {
        log::info!("Running {} test", test_name);

        let mut cmd = Command::new("espflash");
        cmd.args(args);

        let (output, _) = self.run_command_for(&mut cmd, duration)?;
        log::debug!("Output: {}", output);

        if !output.contains(expected_output) {
            return Err(format!(
                "{} test failed: expected output containing '{}'",
                test_name, expected_output
            )
            .into());
        }

        log::info!("{} test passed", test_name);
        Ok(output)
    }

    /// Helper to check if file contains only 0xFF bytes
    fn is_flash_empty(&self, file_path: &Path) -> Result<bool> {
        let flash_data = fs::read(file_path)?;
        Ok(flash_data.iter().all(|&b| b == 0xFF))
    }

    /// Helper to get a temporary output file path for flash content
    fn flash_output_file(&self) -> PathBuf {
        self.tests_dir.join("flash_content.bin")
    }

    // Refactored and simplified test implementations
    fn test_board_info(&self) -> Result<()> {
        let output = self.run_simple_command_test(
            &["board-info"],
            "Chip type:",
            self.timeout,
            "board-info",
        )?;

        // Extract chip type from output
        let detected_chip = output
            .lines()
            .find(|line| line.contains("Chip type:"))
            .and_then(|line| line.split(':').nth(1))
            .and_then(|chip_info| chip_info.split_whitespace().next())
            .map(|s| s.to_string())
            .unwrap_or_else(|| self.chip.clone().unwrap_or_else(|| "unknown".to_string()));

        // Check security features based on chip type
        if detected_chip == "esp32" {
            if !output.contains("Security features: None") {
                return Err("ESP32 should show 'Security features: None'".into());
            }
        } else if !output.contains("Security Information:") || !output.contains("Flags") {
            return Err("Non-ESP32 should show 'Security Information:' and 'Flags'".into());
        }

        Ok(())
    }

    fn test_flash(&self, chip: Option<&str>) -> Result<()> {
        let chip = chip.unwrap_or_else(|| self.chip.as_deref().unwrap_or("esp32"));
        log::info!("Running flash test for chip: {}", chip);

        let app = format!("espflash/tests/data/{}", chip);
        let part_table = "espflash/tests/data/partitions.csv";

        // Test case 1: Should fail with partition table that's too big
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

        let (output, _) = self.run_command_for(&mut cmd, Duration::from_secs(15))?;
        log::debug!("Output: {}", output);

        if !output.contains("espflash::partition_table::does_not_fit") {
            return Err("Flashing should have failed with partition table size error".into());
        }

        // Additional tests for ESP32-C6 with defmt
        if chip == "esp32c6" {
            self.test_flash_with_defmt(&app)?;
        }

        // Test standard flashing
        self.run_timed_command_test(
            &["flash", "--no-skip", "--monitor", "--non-interactive", &app],
            "Hello world!",
            Duration::from_secs(15),
            "standard flashing",
        )?;

        Ok(())
    }

    fn test_flash_with_defmt(&self, app: &str) -> Result<()> {
        let app_defmt = format!("{}_defmt", app);

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

    fn test_list_ports(&self) -> Result<()> {
        self.run_simple_command_test(&["list-ports"], "Silicon Labs", self.timeout, "list-ports")?;

        Ok(())
    }

    fn test_erase_flash(&self) -> Result<()> {
        // Erase flash
        self.run_simple_command_test(
            &["erase-flash"],
            "Flash has been erased!",
            self.flash_timeout,
            "erase-flash",
        )?;

        // Read flash to verify it's erased
        let temp_bin = self.flash_output_file();
        self.run_simple_command_test(
            &["read-flash", "0", "0x4000", temp_bin.to_str().unwrap()],
            "Flash content successfully read and written to",
            self.flash_timeout,
            "read after erase",
        )?;

        // Check if flash is empty (all 0xFF)
        if !self.is_flash_empty(&temp_bin)? {
            return Err("Flash is not empty after erase".into());
        }

        Ok(())
    }

    fn test_erase_region(&self) -> Result<()> {
        log::info!("Running erase-region test");

        // First, write some non-FF data to the flash memory
        let pattern_file = self.tests_dir.join("region_test_pattern.bin");
        let full_data = vec![0x55; 0x2000]; // Fill with 0x55 (non-FF value)
        fs::write(&pattern_file, &full_data)?;

        // Write the pattern to flash
        self.run_simple_command_test(
            &["write-bin", "0x1000", pattern_file.to_str().unwrap()],
            "Binary successfully written to flash!",
            self.flash_timeout,
            "write test pattern",
        )?;

        // Test unaligned addresses and sizes (should fail)
        let unaligned_test_cases = [
            ("0x1001", "0x1000", "Unaligned address"),
            ("0x1000", "0x1001", "Unaligned size"),
            ("0x1003", "0x1005", "Unaligned address and size"),
        ];

        for (address, size, test_desc) in &unaligned_test_cases {
            let mut cmd = Command::new("espflash");
            cmd.args(["erase-region", address, size]);

            let (output, _) = self.run_command_with_timeout(&mut cmd, self.timeout)?;
            log::debug!("{} test output: {}", test_desc, output);

            if !output.contains("Invalid `address`") {
                return Err(format!(
                    "{} was not rejected: address={}, size={}",
                    test_desc, address, size
                )
                .into());
            }
            log::info!(
                "{} correctly rejected: address={}, size={}",
                test_desc,
                address,
                size
            );
        }

        // Test valid erase parameters
        self.run_simple_command_test(
            &["erase-region", "0x1000", "0x1000"],
            "Erasing region at",
            self.timeout,
            "valid erase operation",
        )?;

        // Verify erased region
        let temp_bin = self.flash_output_file();
        self.run_simple_command_test(
            &["read-flash", "0x1000", "0x2000", temp_bin.to_str().unwrap()],
            "Flash content successfully read and written to",
            self.flash_timeout,
            "read after region erase",
        )?;

        // Read the flash content and check it
        let flash_data = fs::read(&temp_bin)?;

        // First 0x1000 bytes should be all 0xFF (erased)
        if !flash_data[0..0x1000].iter().all(|&b| b == 0xFF) {
            return Err("First 0x1000 bytes should be empty (all 0xFF)".into());
        }

        // Next 0x1000 bytes should contain some non-FF bytes
        if flash_data[0x1000..0x2000].iter().all(|&b| b == 0xFF) {
            return Err("Next 0x1000 bytes should contain some non-FF bytes".into());
        }

        Ok(())
    }

    fn test_read_flash(&self) -> Result<()> {
        log::info!("Running read-flash test");

        // Create a known pattern to write to flash
        let known_pattern = vec![
            0x01, 0xA0, 0x02, 0xB3, 0x04, 0xC4, 0x08, 0xD5, 0x10, 0xE6, 0x20, 0xF7, 0x40, 0x88,
            0x50, 0x99, 0x60, 0xAA, 0x70, 0xBB, 0x80, 0xCC, 0x90, 0xDD, 0xA0, 0xEE, 0xB0, 0xFF,
            0xC0, 0x11, 0xD0, 0x22, 0xE0, 0x33, 0xF0, 0x44, 0x05, 0x55, 0x15, 0x66, 0x25, 0x77,
            0x35, 0x88, 0x45, 0x99, 0x55, 0xAA, 0x65, 0xBB, 0x75, 0xCC, 0x85, 0xDD, 0x95, 0xEE,
            0xA5, 0xFF, 0xB5, 0x00, 0xC5, 0x11, 0xD5, 0x22, 0xE5, 0x33, 0xF5, 0x44, 0x06, 0x55,
            0x16, 0x66, 0x26, 0x77, 0x36, 0x88, 0x46, 0x99, 0x56, 0xAA, 0x66, 0xBB, 0x76, 0xCC,
            0x86, 0xDD, 0x96, 0xEE, 0xA6, 0xFF, 0xB6, 0x00, 0xC6, 0x11, 0xD6, 0x22,
        ];

        let pattern_file = self.tests_dir.join("pattern.bin");
        fs::write(&pattern_file, &known_pattern)?;

        // Write the pattern to flash
        self.run_simple_command_test(
            &["write-bin", "0x0", pattern_file.to_str().unwrap()],
            "Binary successfully written to flash!",
            self.flash_timeout,
            "write pattern",
        )?;

        // Test reading different lengths from flash
        let lengths = [2, 5, 10, 26, 44, 86];
        let flash_content_file = self.flash_output_file();

        for &len in &lengths {
            log::info!("Testing read-flash with length: {}", len);

            // Test with normal read (with stub)
            self.run_simple_command_test(
                &[
                    "read-flash",
                    "0",
                    &len.to_string(),
                    flash_content_file.to_str().unwrap(),
                ],
                "Flash content successfully read and written to",
                self.flash_timeout,
                &format!("read flash {} bytes with stub", len),
            )?;

            // Verify the content matches the expected pattern
            let read_data = fs::read(&flash_content_file)?;
            let expected_data = &known_pattern[0..len as usize];

            if read_data != expected_data {
                return Err(format!(
                    "Verification failed: content does not match expected for length {}",
                    len
                )
                .into());
            }

            // Test with ROM read (no stub)
            self.run_simple_command_test(
                &[
                    "read-flash",
                    "--no-stub",
                    "0",
                    &len.to_string(),
                    flash_content_file.to_str().unwrap(),
                ],
                "Flash content successfully read and written to",
                self.flash_timeout,
                &format!("read flash {} bytes without stub", len),
            )?;

            // Verify the content matches again
            let read_data = fs::read(&flash_content_file)?;
            if read_data != expected_data {
                return Err(format!(
                    "Verification failed: ROM read content does not match expected for length {}",
                    len
                )
                .into());
            }
        }

        Ok(())
    }

    fn test_write_bin(&self) -> Result<()> {
        log::info!("Running write-bin test");

        let part_table = "espflash/tests/data/partitions.csv";
        let binary_file = self.tests_dir.join("binary_file.bin");
        let flash_content_file = self.flash_output_file();

        // Create test binary file with [0x01, 0xA0] content
        fs::write(&binary_file, [0x01, 0xA0])?;

        // Test 1: Write binary to address 0x0
        self.run_simple_command_test(
            &["write-bin", "0x0", binary_file.to_str().unwrap()],
            "Binary successfully written to flash!",
            self.flash_timeout,
            "write binary to address 0x0",
        )?;

        // Read back and verify
        self.run_simple_command_test(
            &[
                "read-flash",
                "0",
                "64",
                flash_content_file.to_str().unwrap(),
            ],
            "Flash content successfully read and written to",
            self.flash_timeout,
            "read after writing to address 0x0",
        )?;

        // Verify content contains the expected bytes
        let flash_content = fs::read(&flash_content_file)?;
        if !Self::contains_sequence(&flash_content, &[0x01, 0xA0]) {
            return Err(
                "Verification failed: flash content does not contain expected bytes [0x01, 0xA0]"
                    .into(),
            );
        }

        // Test 2: Write binary to the nvs partition label
        self.run_simple_command_test(
            &[
                "write-bin",
                "nvs",
                binary_file.to_str().unwrap(),
                "--partition-table",
                part_table,
            ],
            "Binary successfully written to flash!",
            self.flash_timeout,
            "write binary to nvs partition",
        )?;

        // Read back and verify
        self.run_simple_command_test(
            &[
                "read-flash",
                "0x9000",
                "64",
                flash_content_file.to_str().unwrap(),
            ],
            "Flash content successfully read and written to",
            self.flash_timeout,
            "read after writing to nvs partition",
        )?;

        // Verify content contains the expected bytes again
        let flash_content = fs::read(&flash_content_file)?;
        if !Self::contains_sequence(&flash_content, &[0x01, 0xA0]) {
            return Err(
                "Verification failed: nvs partition does not contain expected bytes [0x01, 0xA0]"
                    .into(),
            );
        }

        Ok(())
    }

    // Helper function to check if a byte slice contains a specific sequence
    fn contains_sequence(data: &[u8], sequence: &[u8]) -> bool {
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

    fn test_save_image(&self, chip: Option<&str>) -> Result<()> {
        let chip = chip.unwrap_or_else(|| self.chip.as_deref().unwrap_or("esp32"));
        log::info!("Running save-image test for chip: {}", chip);

        let app = format!("espflash/tests/data/{}", chip);
        let app_bin = self.tests_dir.join("app.bin");

        // Add frequency flag for ESP32-C2
        let mut args = vec!["save-image", "--merge", "--chip", chip];
        if chip == "esp32c2" {
            args.extend(["-x", "26mhz"]);
        }
        args.push(&app);
        args.push(app_bin.to_str().unwrap());

        // First, save the image
        self.run_simple_command_test(
            &args,
            "Image successfully saved!",
            self.timeout,
            "save image",
        )?;

        // Then write the binary and monitor
        self.run_timed_command_test(
            &[
                "write-bin",
                "--monitor",
                "0x0",
                app_bin.to_str().unwrap(),
                "--non-interactive",
            ],
            "Hello world!",
            Duration::from_secs(45),
            "write binary and monitor",
        )?;

        // Special regression test for ESP32-C6
        if chip == "esp32c6" {
            self.test_esp32c6_regression(&app_bin)?;
        }

        Ok(())
    }

    fn test_esp32c6_regression(&self, app_bin: &Path) -> Result<()> {
        log::info!("Running ESP32-C6 specific regression test");

        let app = "espflash/tests/data/esp_idf_firmware_c6.elf";

        // Save the image with ESP-IDF firmware
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
            self.timeout,
            "ESP32-C6 regression test save image",
        )?;

        // Check app descriptor magic word
        log::info!("Checking that app descriptor is first");

        // Read the binary file and verify
        let binary_data = fs::read(app_bin)?;

        if binary_data.len() < 0x10024 {
            return Err("Binary file is too small to contain app descriptor".into());
        }

        let magic_word = &binary_data[0x10020..0x10024];
        let expected_magic = [0x32, 0x54, 0xCD, 0xAB]; // Little-endian representation of 0xABCD5432

        if magic_word != expected_magic {
            return Err(format!(
                "App descriptor magic word is not correct: {:02x?} (expected: {:02x?})",
                magic_word, expected_magic
            )
            .into());
        }

        Ok(())
    }

    fn test_monitor(&self) -> Result<()> {
        self.run_timed_command_test(
            &["monitor", "--non-interactive"],
            "Hello world!",
            Duration::from_secs(5),
            "monitor",
        )?;

        Ok(())
    }

    fn test_reset(&self) -> Result<()> {
        self.run_simple_command_test(&["reset"], "Resetting target device", self.timeout, "reset")?;

        Ok(())
    }

    fn test_hold_in_reset(&self) -> Result<()> {
        self.run_simple_command_test(
            &["hold-in-reset"],
            "Holding target device in reset",
            self.timeout,
            "hold-in-reset",
        )?;

        Ok(())
    }

    fn test_checksum_md5(&self) -> Result<()> {
        // First, erase the flash to ensure consistent state
        self.run_simple_command_test(
            &["erase-flash"],
            "Flash has been erased!",
            self.flash_timeout,
            "erase before checksum",
        )?;

        // Then, run the checksum-md5 command
        self.run_simple_command_test(
            &["checksum-md5", "0x1000", "0x100"],
            "0x827f263ef9fb63d05499d14fcef32f60",
            self.timeout,
            "checksum-md5",
        )?;

        Ok(())
    }
}

// ----------------------------------------------------------------------------
// Generate eFuse Fields

const HEADER: &str = r#"
//! eFuse field definitions for the $CHIP
//!
//! This file was automatically generated, please do not edit it manually!
//!
//! Generated: $DATE
//! Version:   $VERSION

#![allow(unused)]

use super::EfuseField;

"#;

type EfuseFields = HashMap<String, EfuseYaml>;

#[derive(Debug, serde::Deserialize)]
struct EfuseYaml {
    #[serde(rename = "VER_NO")]
    version: String,
    #[serde(rename = "EFUSES")]
    fields: HashMap<String, EfuseAttrs>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
struct EfuseAttrs {
    #[serde(rename = "blk")]
    block: u32,
    word: u32,
    len: u32,
    start: u32,
    #[serde(rename = "desc")]
    description: String,
}

impl PartialOrd for EfuseAttrs {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for EfuseAttrs {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.block.cmp(&other.block) {
            Ordering::Equal => {}
            ord => return ord,
        }

        match self.word.cmp(&other.word) {
            Ordering::Equal => {}
            ord => return ord,
        }

        self.start.cmp(&other.start)
    }
}

fn generate_efuse_fields(workspace: &Path, args: GenerateEfuseFieldsArgs) -> Result<()> {
    let efuse_yaml_path = args
        .esptool_path
        .join("espefuse")
        .join("efuse_defs")
        .canonicalize()?;

    let espflash_path = workspace.join("espflash").canonicalize()?;

    let mut efuse_fields = parse_efuse_fields(&efuse_yaml_path)?;
    process_efuse_definitions(&mut efuse_fields)?;
    generate_efuse_definitions(&espflash_path, efuse_fields)?;

    Command::new("cargo")
        .args(["+nightly", "fmt"])
        .current_dir(workspace)
        .output()?;

    Ok(())
}

fn parse_efuse_fields(efuse_yaml_path: &Path) -> Result<EfuseFields> {
    // TODO: We can probably handle this better, e.g. by defining a `Chip` enum
    //       which can be iterated over, but for now this is good enough.
    const CHIPS: &[&str] = &[
        "esp32", "esp32c2", "esp32c3", "esp32c5", "esp32c6", "esp32h2", "esp32p4", "esp32s2",
        "esp32s3",
    ];

    let mut efuse_fields = EfuseFields::new();

    for result in fs::read_dir(efuse_yaml_path)? {
        let path = result?.path();
        if path.extension().is_none_or(|ext| ext != OsStr::new("yaml")) {
            continue;
        }

        let chip = path.file_stem().unwrap().to_string_lossy().to_string();
        if !CHIPS.contains(&chip.as_str()) {
            continue;
        }

        let efuse_yaml = fs::read_to_string(&path)?;
        let efuse_yaml: EfuseYaml = serde_yaml::from_str(&efuse_yaml)?;

        efuse_fields.insert(chip.to_string(), efuse_yaml);
    }

    Ok(efuse_fields)
}

fn process_efuse_definitions(efuse_fields: &mut EfuseFields) -> Result<()> {
    // This is all a special case for the MAC field, which is larger than a single
    // word (i.e. 32-bits) in size. To handle this, we just split it up into two
    // separate fields, and update the fields' attributes accordingly.
    for yaml in (*efuse_fields).values_mut() {
        let mac_attrs = yaml.fields.get("MAC").unwrap();

        let mut mac0_attrs = mac_attrs.clone();
        mac0_attrs.len = 32;

        let mut mac1_attrs = mac_attrs.clone();
        mac1_attrs.start = mac0_attrs.start + 32;
        mac1_attrs.word = mac1_attrs.start / 32;
        mac1_attrs.len = 16;

        yaml.fields.remove("MAC").unwrap();
        yaml.fields.insert("MAC0".into(), mac0_attrs);
        yaml.fields.insert("MAC1".into(), mac1_attrs);
    }

    // The ESP32-S2 seems to be missing a reserved byte at the end of BLOCK0
    // (Or, something else weird is going on).
    efuse_fields.entry("esp32s2".into()).and_modify(|yaml| {
        yaml.fields
            .entry("RESERVED_0_162".into())
            .and_modify(|field| field.len = 30);
    });

    Ok(())
}

fn generate_efuse_definitions(espflash_path: &Path, efuse_fields: EfuseFields) -> Result<()> {
    let targets_efuse_path = espflash_path
        .join("src")
        .join("target")
        .join("efuse")
        .canonicalize()?;

    for (chip, yaml) in efuse_fields {
        let f = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(targets_efuse_path.join(format!("{chip}.rs")))?;

        let mut writer = BufWriter::new(f);

        write!(
            writer,
            "{}",
            HEADER
                .replace("$CHIP", &chip)
                .replace(
                    "$DATE",
                    &chrono::Utc::now().format("%Y-%m-%d %H:%M").to_string()
                )
                .replace("$VERSION", &yaml.version)
                .trim_start()
        )?;

        generate_efuse_block_sizes(&mut writer, &yaml.fields)?;
        generate_efuse_constants(&mut writer, &yaml.fields)?;
    }

    Ok(())
}

fn generate_efuse_block_sizes(
    writer: &mut dyn Write,
    fields: &HashMap<String, EfuseAttrs>,
) -> Result<()> {
    let mut field_attrs = fields.values().collect::<Vec<_>>();
    field_attrs.sort();

    let block_sizes = field_attrs
        .chunk_by(|a, b| a.block == b.block)
        .enumerate()
        .map(|(block, attrs)| {
            let last = attrs.last().unwrap();
            let size_bits = last.start + last.len;
            assert!(size_bits % 8 == 0);

            (block, size_bits / 8)
        })
        .collect::<BTreeMap<_, _>>();

    writeln!(writer, "/// Total size in bytes of each block")?;
    writeln!(
        writer,
        "pub(crate) const BLOCK_SIZES: &[u32] = &[{}];\n",
        block_sizes
            .values()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    )?;

    Ok(())
}

fn generate_efuse_constants(
    writer: &mut dyn Write,
    fields: &HashMap<String, EfuseAttrs>,
) -> Result<()> {
    let mut sorted = fields.iter().collect::<Vec<_>>();
    sorted.sort_by(|a, b| (a.1).cmp(b.1));

    for (name, attrs) in sorted {
        let EfuseAttrs {
            block,
            word,
            len,
            start,
            description,
        } = attrs;

        let description = description.replace('[', "\\[").replace(']', "\\]");

        writeln!(writer, "/// {description}")?;
        writeln!(
            writer,
            "pub const {name}: EfuseField = EfuseField::new({block}, {word}, {start}, {len});"
        )?;
    }

    Ok(())
}
