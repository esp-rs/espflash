use std::{
    env,
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use clap::{ArgAction, Args};
use log::info;

use crate::Result;

const SUPPORTED_CHIPS: [&str; 12] = [
    "esp32", "esp32c2", "esp32c3", "esp32c5", "esp32c6", "esp32c61", "esp32h2", "esp32h4",
    "esp32p4", "esp32s2", "esp32s3", "esp32s31",
];

type SpawnedCommand = (
    Child,
    Arc<Mutex<String>>,
    thread::JoinHandle<()>,
    thread::JoinHandle<()>,
);

struct CommandOutput {
    status: ExitStatus,
    output: String,
    timed_out: bool,
}

/// Arguments for running tests
#[derive(Debug, Args)]
pub struct RunTestsArgs {
    /// Which test to run (or "all" to run all tests)
    #[clap(default_value = "all")]
    pub test: String,

    /// Chip target
    #[clap(
        short,
        long,
        value_parser = clap::builder::PossibleValuesParser::new(SUPPORTED_CHIPS)
    )]
    pub chip: Option<String>,

    /// Maximum duration of each command, in seconds
    #[clap(short, long, default_value = "60")]
    pub timeout: u64,

    /// Do not build espflash; find it in PATH instead
    #[arg(long = "no-build", action = ArgAction::SetFalse, default_value_t = true)]
    pub build_espflash: bool,

    /// espflash executable to test (also disables the local build)
    #[arg(long, value_name = "PATH")]
    pub espflash: Option<PathBuf>,

    /// Baud rate for transfer-heavy hardware tests
    #[arg(long, value_name = "BAUD")]
    pub baud: Option<u32>,

    /// Run extended hardware command and option coverage
    #[arg(long)]
    pub extended: bool,

    /// Run the subset supported in secure download mode
    #[arg(long)]
    pub sdm: bool,
}

/// A struct to manage and run tests for espflash.
pub struct TestRunner {
    /// The workspace directory where the tests are located
    pub workspace: PathBuf,
    /// The directory containing the test files
    pub tests_dir: PathBuf,
    /// Maximum duration of each command
    pub timeout: Duration,
    /// espflash executable under test
    pub espflash: PathBuf,
    /// Baud rate for transfer-heavy hardware tests
    pub baud: Option<u32>,
}

impl TestRunner {
    /// Creates a new [TestRunner] instance.
    pub fn new(
        workspace: &Path,
        tests_dir: PathBuf,
        timeout_secs: u64,
        espflash: PathBuf,
        baud: Option<u32>,
    ) -> Self {
        Self {
            workspace: workspace.to_path_buf(),
            tests_dir,
            timeout: Duration::from_secs(timeout_secs),
            espflash,
            baud,
        }
    }

    fn setup_command(&self, cmd: &mut Command) {
        cmd.current_dir(&self.workspace)
            // Update checks make HIL slower and introduce an unnecessary network
            // dependency. This also exercises the global environment option.
            .env("ESPFLASH_SKIP_UPDATE_CHECK", "true");
    }

    fn restore_terminal() {
        #[cfg(unix)]
        if std::io::IsTerminal::is_terminal(&std::io::stdin()) {
            let _ = Command::new("stty").arg("sane").status();
        }
    }

    fn spawn_and_capture_output(&self, cmd: &mut Command) -> Result<SpawnedCommand> {
        self.setup_command(cmd);
        info!("Spawning command: {cmd:?}");
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = cmd.spawn()?;
        let stdout = child.stdout.take().expect("stdout was configured as piped");
        let stderr = child.stderr.take().expect("stderr was configured as piped");

        let output = Arc::new(Mutex::new(String::new()));
        let stdout_output = Arc::clone(&output);
        let stderr_output = Arc::clone(&output);

        let stdout_handle = thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(|line| line.ok()) {
                println!("{line}");
                if let Ok(mut output) = stdout_output.lock() {
                    output.push_str(&line);
                    output.push('\n');
                }
            }
        });
        let stderr_handle = thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(|line| line.ok()) {
                eprintln!("{line}");
                if let Ok(mut output) = stderr_output.lock() {
                    output.push_str(&line);
                    output.push('\n');
                }
            }
        });

        Ok((child, output, stdout_handle, stderr_handle))
    }

    fn output_contains_all(output: &Arc<Mutex<String>>, expected: &[&str]) -> bool {
        output
            .lock()
            .map(|output| expected.iter().all(|value| output.contains(value)))
            .unwrap_or(false)
    }

    fn execute_command(
        &self,
        cmd: &mut Command,
        timeout: Duration,
        stop_after: Option<&[&str]>,
    ) -> Result<CommandOutput> {
        let (mut child, output, stdout_handle, stderr_handle) =
            self.spawn_and_capture_output(cmd)?;
        let start = Instant::now();
        let mut timed_out = false;

        let status = loop {
            // Monitors do not exit by themselves. Stop as soon as all expected
            // output arrives instead of sleeping for the whole timeout.
            if stop_after.is_some_and(|expected| Self::output_contains_all(&output, expected)) {
                let _ = child.kill();
                break child.wait()?;
            }

            if let Some(status) = child.try_wait()? {
                break status;
            }

            if start.elapsed() >= timeout {
                timed_out = true;
                log::warn!(
                    "Command timed out after {timeout:?}; terminating process {}",
                    child.id()
                );
                let _ = child.kill();
                break child.wait()?;
            }

            thread::sleep(Duration::from_millis(50));
        };

        let _ = stdout_handle.join();
        let _ = stderr_handle.join();
        Self::restore_terminal();

        let output = output
            .lock()
            .map_err(|_| "command output mutex was poisoned")?
            .clone();
        Ok(CommandOutput {
            status,
            output,
            timed_out,
        })
    }

    /// Runs a command with a timeout, returning the exit code.
    pub fn run_command_with_timeout(&self, cmd: &mut Command, timeout: Duration) -> Result<i32> {
        let result = self.execute_command(cmd, timeout, None)?;
        if result.timed_out {
            return Err(format!("Command timed out after {timeout:?}: {cmd:?}").into());
        }
        Ok(result.status.code().unwrap_or(1))
    }

    fn build_espflash(workspace: &Path) -> Result<PathBuf> {
        log::info!("Building espflash...");
        let status = Command::new("cargo")
            .current_dir(workspace)
            .args(["build", "-p", "espflash", "--release"])
            .status()?;
        if !status.success() {
            return Err(format!("espflash build failed with status: {status}").into());
        }

        let target_dir = match env::var_os("CARGO_TARGET_DIR") {
            Some(path) if Path::new(&path).is_absolute() => PathBuf::from(path),
            Some(path) => workspace.join(path),
            None => workspace.join("target"),
        };
        Ok(target_dir
            .join("release")
            .join(format!("espflash{}", env::consts::EXE_SUFFIX)))
    }

    fn create_espflash_command(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new(&self.espflash);
        cmd.args(args);
        cmd
    }

    fn create_transfer_command(&self, args: &[&str]) -> Command {
        let mut cmd = self.create_espflash_command(args);
        if let Some(baud) = self.baud {
            cmd.arg("--baud").arg(baud.to_string());
        }
        cmd
    }

    /// Runs a command to completion and verifies its status and output.
    pub fn run_simple_command_test(
        &self,
        args: &[&str],
        expected_contains: Option<&[&str]>,
        timeout: Duration,
        test_name: &str,
    ) -> Result<()> {
        log::info!("Running {test_name} test");
        let mut cmd = self.create_espflash_command(args);
        let result = self.execute_command(&mut cmd, timeout, None)?;

        if result.timed_out {
            return Err(format!("{test_name} timed out after {timeout:?}").into());
        }
        if !result.status.success() {
            return Err(format!("{test_name} failed with status {}", result.status).into());
        }
        if let Some(expected) = expected_contains {
            for value in expected {
                if !result.output.contains(value) {
                    return Err(format!("{test_name}: missing expected output: {value}").into());
                }
            }
        }

        log::info!("{test_name} test passed");
        Ok(())
    }

    /// Runs a command until all expected output is observed or the timeout
    /// expires. This is used for monitor commands which intentionally do not
    /// terminate naturally.
    pub fn run_timed_command_test(
        &self,
        args: &[&str],
        expected_contains: Option<&[&str]>,
        timeout: Duration,
        test_name: &str,
    ) -> Result<()> {
        let cmd = self.create_espflash_command(args);
        self.run_timed_command(cmd, expected_contains, timeout, test_name)
    }

    fn run_timed_transfer_command_test(
        &self,
        args: &[&str],
        expected_contains: Option<&[&str]>,
        timeout: Duration,
        test_name: &str,
    ) -> Result<()> {
        let cmd = self.create_transfer_command(args);
        self.run_timed_command(cmd, expected_contains, timeout, test_name)
    }

    fn run_timed_command(
        &self,
        mut cmd: Command,
        expected_contains: Option<&[&str]>,
        timeout: Duration,
        test_name: &str,
    ) -> Result<()> {
        log::info!("Running {test_name} test");
        let result = self.execute_command(&mut cmd, timeout, expected_contains)?;

        if let Some(expected) = expected_contains {
            for value in expected {
                if !result.output.contains(value) {
                    let reason = if result.timed_out {
                        format!("timed out after {timeout:?}")
                    } else {
                        format!("exited with status {}", result.status)
                    };
                    return Err(
                        format!("{test_name} {reason}; missing expected output: {value}").into(),
                    );
                }
            }
        } else if !result.status.success() && !result.timed_out {
            return Err(format!("{test_name} failed with status {}", result.status).into());
        }

        log::info!("{test_name} test passed");
        Ok(())
    }

    fn is_flash_empty(&self, file_path: &Path, chip: Option<&str>) -> Result<bool> {
        let flash_data = fs::read(file_path)?;
        Ok(Self::is_erased_flash_data(&flash_data, chip))
    }

    fn is_erased_flash_data(data: &[u8], chip: Option<&str>) -> bool {
        data.iter().enumerate().all(|(offset, &byte)| {
            byte == 0xFF || Self::is_esp32p4_erased_read_artifact(offset, byte, chip)
        })
    }

    fn is_esp32p4_erased_read_artifact(offset: usize, byte: u8, chip: Option<&str>) -> bool {
        if !matches!(chip, Some("esp32p4")) || byte != 0x00 {
            return false;
        }

        // ESP32-P4 flash stub reads erased flash as 0x00 in a few regular ranges.
        // Treat only those ranges as erased so the test can still verify all other
        // bytes.
        let offset_in_sector = offset % 0x1000;
        (offset >= 0x400 && offset % 0x400 < 0x100)
            || (offset >= 0x1000 && (0x0e0..0x100).contains(&offset_in_sector))
    }

    fn flash_output_file(&self) -> PathBuf {
        self.tests_dir.join("flash_content.bin")
    }

    /// Runs all tests in the test suite, optionally overriding the chip target
    pub fn run_all_tests(
        &self,
        chip_override: Option<&str>,
        sdm: bool,
        extended: bool,
    ) -> Result<()> {
        log::info!("Running all tests");

        let chip = chip_override.unwrap_or("esp32");

        if sdm {
            self.test_board_info(chip)?;
            self.test_save_image_write_bin(Some(chip))?;
            self.test_hold_in_reset()?;
            self.test_reset()?;
            self.test_list_ports(false)?;
            self.test_flash(Some(chip), false)?;
            self.test_monitor(chip)?;
        } else if chip == "esp32p4" {
            // ESP32-P4 flash stub currently reports erased bytes and MD5 checksums
            // differently in some ranges, so keep the default suite to tests that
            // are stable on this target.
            self.test_board_info(chip)?;
            self.test_erase_flash(Some(chip))?;
            self.test_hold_in_reset()?;
            self.test_reset()?;
            self.test_list_ports(extended)?;
        } else {
            self.test_board_info(chip)?;
            self.test_erase_flash(Some(chip))?;
            self.test_save_image_write_bin(Some(chip))?;
            if extended {
                self.test_erase_parts(Some(chip))?;
            }
            self.test_erase_region(Some(chip))?;
            self.test_hold_in_reset()?;
            self.test_reset()?;
            self.test_list_ports(extended)?;
            self.test_checksum_md5()?;
            self.test_read_flash()?;
            self.test_flash(Some(chip), extended)?;
            self.test_monitor(chip)?;
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
        let chip = chip_override.unwrap_or("esp32");

        if sdm {
            return match test_name {
                "board-info" => self.test_board_info(chip),
                "flash" => self.test_flash(Some(chip), false),
                "save-image" | "write-bin" | "save-image-write-bin" => {
                    self.test_save_image_write_bin(Some(chip))
                }
                "hold-in-reset" => self.test_hold_in_reset(),
                "reset" => self.test_reset(),
                "list-ports" => self.test_list_ports(false),
                "monitor" => self.test_monitor(chip),
                _ => Err(format!("Unknown or unsupported SDM test: {test_name}").into()),
            };
        }

        match test_name {
            "board-info" => self.test_board_info(chip),
            "flash" => self.test_flash(Some(chip), false),
            "monitor" => self.test_monitor(chip),
            "erase-flash" => self.test_erase_flash(Some(chip)),
            "erase-parts" => self.test_erase_parts(Some(chip)),
            "save-image" | "write-bin" | "save-image-write-bin" => {
                self.test_save_image_write_bin(Some(chip))
            }
            "erase-region" => self.test_erase_region(Some(chip)),
            "hold-in-reset" => self.test_hold_in_reset(),
            "reset" => self.test_reset(),
            "checksum-md5" => self.test_checksum_md5(),
            "list-ports" => self.test_list_ports(true),
            "partition-table" | "offline" => self.test_offline_commands(chip),
            "read-flash" => self.test_read_flash(),
            _ => Err(format!("Unknown test: {test_name}").into()),
        }
    }

    // Board info test
    pub fn test_board_info(&self, chip: &str) -> Result<()> {
        self.run_simple_command_test(
            &[
                "board-info",
                "--baud",
                "115200",
                "--after",
                "hard-reset",
                "--before",
                "default-reset",
                "--confirm-port",
                "--list-all-ports",
                "--non-interactive",
            ],
            Some(&[&format!("Chip type:         {chip}")]),
            self.timeout,
            "board-info",
        )
    }

    /// Tests commands that do not require hardware and exercises image options
    /// which would otherwise make every HIL target perform another flash.
    pub fn test_offline_commands(&self, chip: &str) -> Result<()> {
        log::info!("Running extended offline command tests for {chip}");
        let partition_csv = "espflash/tests/data/partitions.csv";
        let partition_bin = self.tests_dir.join("partitions.bin");
        let partition_roundtrip = self.tests_dir.join("partitions-roundtrip.csv.bin");
        let options_image = self.tests_dir.join("options-image.bin");
        let app = format!("espflash/tests/data/{chip}");

        self.run_simple_command_test(
            &[
                "partition-table",
                "--to-binary",
                "--output",
                partition_bin.to_str().unwrap(),
                partition_csv,
            ],
            None,
            self.timeout,
            "partition table CSV to binary",
        )?;
        self.run_simple_command_test(
            &[
                "partition-table",
                "--to-csv",
                "--output",
                partition_roundtrip.to_str().unwrap(),
                partition_bin.to_str().unwrap(),
            ],
            None,
            self.timeout,
            "partition table binary to CSV",
        )?;
        let roundtrip = fs::read_to_string(&partition_roundtrip)?;
        for label in ["nvs", "phy_init", "factory"] {
            if !roundtrip.contains(label) {
                return Err(format!("partition table roundtrip lost partition {label}").into());
            }
        }
        self.run_simple_command_test(
            &["partition-table", partition_csv],
            Some(&["factory", "0x10000"]),
            self.timeout,
            "partition table display",
        )?;

        let flash_frequency = match chip {
            "esp32c2" => "30mhz",
            "esp32h2" | "esp32h4" => "24mhz",
            _ => "40mhz",
        };
        let bootloader_name = match chip {
            "esp32p4" => "esp32p4-v3-bootloader.bin".to_owned(),
            _ => format!("{chip}-bootloader.bin"),
        };
        let bootloader = self
            .workspace
            .join("espflash/resources/bootloaders")
            .join(bootloader_name);
        let mut args = vec![
            "save-image",
            "--merge",
            "--skip-padding",
            "--chip",
            chip,
            "--flash-freq",
            flash_frequency,
            "--flash-mode",
            "dio",
            "--flash-size",
            "8mb",
            "--ignore-app-descriptor",
            "--format",
            "esp-idf",
            "--bootloader",
            bootloader.to_str().unwrap(),
            "--partition-table",
            partition_csv,
            "--partition-table-offset",
            "0x8000",
            "--target-app-partition",
            "factory",
            &app,
            options_image.to_str().unwrap(),
        ];
        if matches!(chip, "esp32c2" | "esp32c6" | "esp32h2" | "esp32h4") {
            args.extend(["--mmu-page-size", "0x10000"]);
        }
        if chip == "esp32c2" {
            args.extend(["--xtal-freq", "26mhz"]);
        }
        if chip == "esp32p4" {
            args.extend(["--min-chip-rev", "3.0"]);
        }
        self.run_simple_command_test(
            &args,
            Some(&["Image successfully saved!"]),
            self.timeout,
            "save-image options",
        )?;

        Ok(())
    }

    // Flash test
    pub fn test_flash(&self, chip: Option<&str>, extended: bool) -> Result<()> {
        let chip = chip.unwrap_or("esp32");
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
            Some(&["The partition table does not fit into the flash"]),
            self.timeout,
            "partition too big",
        )?;

        // Additional tests for ESP32-C6 with manual log-format
        if chip == "esp32c6" {
            // Test with manual log-format and with auto-detected log-format
            self.test_flash_with_defmt(&app)?;
            // Backtrace test
            self.test_backtrace(&app_backtrace)?;
        }

        // Exercise less common flash and image options on one representative
        // target without adding another flash operation to every HIL job.
        let mut standard_args = vec!["flash", "--no-skip", "--monitor", "--non-interactive"];
        if extended {
            let flash_frequency = match chip {
                "esp32c2" => "30mhz",
                "esp32h2" | "esp32h4" => "24mhz",
                _ => "40mhz",
            };
            standard_args.extend([
                "--chip",
                chip,
                "--no-verify",
                "--force",
                "--flash-freq",
                flash_frequency,
                "--flash-mode",
                "dio",
                "--erase-parts",
                "nvs",
                "--erase-data-parts",
                "nvs",
            ]);
        }
        standard_args.push(&app);
        self.run_timed_transfer_command_test(
            &standard_args,
            Some(&["Flashing has completed!", "Hello world!"]),
            self.timeout,
            "standard flashing",
        )?;

        // Keep default-baud flash coverage on the representative extended runner.
        if extended && self.baud.is_some_and(|baud| baud != 115_200) {
            self.run_timed_command_test(
                &["flash", "--no-skip", "--monitor", "--non-interactive", &app],
                Some(&["Flashing has completed!", "Hello world!"]),
                self.timeout,
                "standard flashing with default baud rate",
            )?;
        }

        Ok(())
    }

    fn test_flash_with_defmt(&self, app: &str) -> Result<()> {
        let app_defmt = format!("{app}_defmt");

        // Test with manual log-format
        self.run_timed_transfer_command_test(
            &[
                "flash",
                "--no-skip",
                "--monitor",
                "--non-interactive",
                &app_defmt,
                "--log-format",
                "defmt",
                "--output-format",
                "full",
            ],
            Some(&["Flashing has completed!", "Hello world!"]),
            self.timeout,
            "defmt manual log-format",
        )?;

        // Test with auto-detected log-format
        self.run_timed_transfer_command_test(
            &[
                "flash",
                "--no-skip",
                "--monitor",
                "--non-interactive",
                &app_defmt,
            ],
            Some(&["Flashing has completed!", "Hello world!"]),
            self.timeout,
            "defmt auto-detected log-format",
        )?;

        Ok(())
    }

    fn test_backtrace(&self, app_backtrace: &str) -> Result<()> {
        // Test flashing with backtrace
        self.run_timed_transfer_command_test(
            &[
                "flash",
                "--no-skip",
                "--monitor",
                "--non-interactive",
                "--all-addresses",
                app_backtrace,
            ],
            Some(&[
                "0x420012c8",
                "main",
                "esp32c6_backtrace/src/bin/main.rs:",
                "0x42001280",
                "hal_main",
            ]),
            self.timeout,
            "backtrace test",
        )?;

        Ok(())
    }

    /// Tests listing available ports
    pub fn test_list_ports(&self, extended: bool) -> Result<()> {
        log::info!("Running list-ports test");
        let mut cmd = self.create_espflash_command(&["list-ports"]);
        let result = self.execute_command(&mut cmd, self.timeout, None)?;
        if result.timed_out || !result.status.success() {
            return Err(format!("list-ports failed with status {}", result.status).into());
        }

        let accept_output = result.output.contains("Silicon Labs")
            || result.output.contains("Espressif")
            || result.output.contains(":303A"); // Espressif USB VID
        if !accept_output {
            return Err("list-ports did not include the connected Espressif device".into());
        }

        if extended {
            self.run_simple_command_test(
                &["list-ports", "--list-all-ports", "--name-only"],
                Some(&["/dev/"]),
                self.timeout,
                "list-ports all names",
            )?;
        }

        log::info!("list-ports test passed and output verified");
        Ok(())
    }

    /// Tests erasing the flash memory
    pub fn test_erase_flash(&self, chip: Option<&str>) -> Result<()> {
        log::info!("Running erase-flash test");
        let flash_output = self.flash_output_file();

        self.run_simple_command_test(
            &["erase-flash"],
            Some(&["Flash has been erased!"]),
            self.timeout,
            "erase-flash",
        )?;

        // Read a portion of the flash to verify it's erased
        self.run_simple_command_test(
            &["read-flash", "0", "0x4000", flash_output.to_str().unwrap()],
            Some(&["Flash content successfully read"]),
            self.timeout,
            "read after erase",
        )?;

        // Verify the flash is empty (all 0xFF)
        if let Ok(is_empty) = self.is_flash_empty(&flash_output, chip) {
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
    pub fn test_erase_region(&self, chip: Option<&str>) -> Result<()> {
        log::info!("Running erase-region test");
        let flash_output = self.flash_output_file();

        // Test unaligned address (not multiple of 4096)
        let mut cmd = self.create_espflash_command(&["erase-region", "0x1001", "0x1000"]);
        let exit_code = self.run_command_with_timeout(&mut cmd, self.timeout)?;
        if exit_code == 0 {
            return Err("Unaligned address erase should have failed but succeeded".into());
        }

        // Test unaligned size (not multiple of 4096)
        let mut cmd = self.create_espflash_command(&["erase-region", "0x1000", "0x1001"]);
        let exit_code = self.run_command_with_timeout(&mut cmd, self.timeout)?;
        if exit_code == 0 {
            return Err("Unaligned size erase should have failed but succeeded".into());
        }

        // Valid erase - should succeed
        self.run_simple_command_test(
            &["erase-region", "0x1000", "0x1000"],
            Some(&["Erasing region at"]),
            self.timeout,
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
            self.timeout,
            "read after erase-region",
        )?;

        // Check flash contents - first part should be erased
        if let Ok(flash_data) = fs::read(&flash_output) {
            // First 0x1000 bytes should be 0xFF (erased)
            let first_part = &flash_data[0..4096];
            if !Self::is_erased_flash_data(first_part, chip) {
                return Err("First 0x1000 bytes should be empty (0xFF)".into());
            }

            // Next 0x1000 bytes should contain some non-erased bytes
            let second_part = &flash_data[4096..8192];
            if Self::is_erased_flash_data(second_part, chip) {
                return Err("Next 0x1000 bytes should contain some non-erased bytes".into());
            }
        } else {
            return Err("Failed to read flash_content.bin file".into());
        }

        log::info!("erase-region test passed");
        Ok(())
    }

    /// Tests erasing a named partition and verifies the resulting bytes.
    pub fn test_erase_parts(&self, chip: Option<&str>) -> Result<()> {
        log::info!("Running erase-parts test");
        let pattern_file = self.tests_dir.join("partition-pattern.bin");
        let flash_output = self.flash_output_file();
        let pattern = vec![0x5a; 256];
        fs::write(&pattern_file, &pattern)?;

        self.run_simple_command_test(
            &["write-bin", "0x9000", pattern_file.to_str().unwrap()],
            Some(&["Binary successfully written to flash!"]),
            self.timeout,
            "populate partition",
        )?;
        self.run_simple_command_test(
            &[
                "erase-parts",
                "nvs",
                "--partition-table",
                "espflash/tests/data/partitions.csv",
            ],
            Some(&["Specified partitions successfully erased!"]),
            self.timeout,
            "erase named partition",
        )?;
        self.run_simple_command_test(
            &[
                "read-flash",
                "0x9000",
                "0x100",
                flash_output.to_str().unwrap(),
            ],
            Some(&["Flash content successfully read"]),
            self.timeout,
            "read erased partition",
        )?;

        if !self.is_flash_empty(&flash_output, chip)? {
            return Err("named partition was not erased".into());
        }
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

        // Ensure the test region can be programmed regardless of the current flash
        // contents.
        self.run_simple_command_test(
            &["erase-region", "0x0", "0x1000"],
            Some(&["Erasing region at"]),
            self.timeout,
            "erase read-flash test region",
        )?;

        // Write the pattern to the flash
        self.run_simple_command_test(
            &["write-bin", "0x0", pattern_file.to_str().unwrap()],
            Some(&["Binary successfully written to flash!"]),
            self.timeout,
            "write pattern",
        )?;

        // Test reading various lengths
        for &len in &[2, 5, 10, 26] {
            log::info!("Testing read-flash with length: {len}");

            // Test normal read
            self.run_simple_command_test(
                &[
                    "read-flash",
                    "--block-size",
                    "0x400",
                    "--max-in-flight",
                    "4",
                    "0x0",
                    &len.to_string(),
                    flash_output.to_str().unwrap(),
                ],
                Some(&["Flash content successfully read and written to"]),
                self.timeout,
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
                    "--block-size",
                    "0x400",
                    "--max-in-flight",
                    "4",
                    "0x0",
                    &len.to_string(),
                    flash_output.to_str().unwrap(),
                ],
                Some(&["Flash content successfully read and written to"]),
                self.timeout,
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
        let chip = chip.unwrap_or("esp32");
        log::info!("Running save-image and write-bin test for chip: {chip}");

        let app = format!("espflash/tests/data/{chip}");
        let app_bin = self.tests_dir.join("app.bin");

        // Test the `--merge` option
        let mut args = vec![
            "save-image",
            "--merge",
            // Required for SDM HIL tests
            "--skip-padding",
            "--chip",
            chip,
            &app,
            app_bin.to_str().unwrap(),
        ];

        // Add chip-specific options.
        if chip == "esp32c2" {
            args.extend(["-x", "26mhz"]);
        }
        if chip == "esp32p4" {
            args.extend(["--min-chip-rev", "3.0"]);
        }

        // Save image
        self.run_simple_command_test(
            &args,
            Some(&["Image successfully saved!"]),
            self.timeout,
            "save-image",
        )?;

        // Write the image and monitor
        self.run_timed_transfer_command_test(
            &[
                "write-bin",
                "--monitor",
                "0x0",
                app_bin.to_str().unwrap(),
                "--non-interactive",
            ],
            Some(&["Hello world!"]),
            self.timeout,
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

        // Add chip-specific options.
        if chip == "esp32c2" {
            args.extend(["-x", "26mhz"]);
        }
        if chip == "esp32p4" {
            args.extend(["--min-chip-rev", "3.0"]);
        }

        // Save image
        self.run_simple_command_test(
            &args,
            Some(&["Image successfully saved!"]),
            self.timeout,
            "save-image",
        )?;

        // Write the image and monitor
        self.run_timed_transfer_command_test(
            &[
                "write-bin",
                "--monitor",
                "0x10000",
                app_bin.to_str().unwrap(),
                "--non-interactive",
            ],
            Some(&["Hello world!"]),
            self.timeout,
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
            self.timeout,
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
            self.timeout,
            "erase-flash for checksum",
        )?;

        // Then check the MD5 checksum of a region
        self.run_simple_command_test(
            &["checksum-md5", "0x1000", "0x100"],
            Some(&["0x827f263ef9fb63d05499d14fcef32f60"]),
            self.timeout,
            "checksum-md5",
        )?;

        log::info!("checksum-md5 test passed");
        Ok(())
    }

    /// Tests the monitor command
    pub fn test_monitor(&self, chip: &str) -> Result<()> {
        let app = format!("espflash/tests/data/{chip}");
        self.run_timed_command_test(
            &[
                "monitor",
                "--non-interactive",
                "--monitor-baud",
                "115200",
                "--log-format",
                "serial",
                "--elf",
                &app,
                "--no-addresses",
            ],
            Some(&["Hello world!"]),
            self.timeout,
            "monitor",
        )?;
        Ok(())
    }

    /// Tests resetting the target device
    pub fn test_reset(&self) -> Result<()> {
        self.run_simple_command_test(
            &["reset"],
            Some(&["Resetting target device"]),
            self.timeout,
            "reset",
        )?;
        Ok(())
    }

    /// Tests holding the target device in reset
    pub fn test_hold_in_reset(&self) -> Result<()> {
        self.run_simple_command_test(
            &["hold-in-reset"],
            Some(&["Holding target device in reset"]),
            self.timeout,
            "hold-in-reset",
        )?;
        Ok(())
    }
}

/// Runs the tests based on the provided arguments
pub fn run_tests(workspace: &Path, args: RunTestsArgs) -> Result<()> {
    log::info!("Running espflash tests");

    let tests_dir = workspace.join("espflash").join("tests");
    // Build once and invoke the resulting executable directly. Running `cargo
    // run` for every assertion used to add overhead and could orphan espflash
    // when the cargo process was killed at a monitor timeout.
    let espflash = if let Some(espflash) = args.espflash {
        if espflash.is_absolute() || espflash.components().count() == 1 {
            espflash
        } else {
            workspace.join(espflash)
        }
    } else if args.build_espflash {
        TestRunner::build_espflash(workspace)?
    } else {
        PathBuf::from("espflash")
    };
    let test_runner = TestRunner::new(workspace, tests_dir, args.timeout, espflash, args.baud);

    match args.test.as_str() {
        "all" => {
            if let Err(e) = test_runner.run_all_tests(args.chip.as_deref(), args.sdm, args.extended)
            {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn runner() -> TestRunner {
        let workspace = env::current_dir().unwrap();
        TestRunner::new(
            &workspace,
            workspace.join("espflash/tests"),
            1,
            PathBuf::from("espflash"),
            None,
        )
    }

    #[cfg(unix)]
    #[test]
    fn command_timeout_really_terminates_process() {
        let runner = runner();
        let mut command = Command::new("sh");
        // Use a shell builtin loop so killing the shell closes its output pipes;
        // a spawned `sleep` process would inherit those pipes and make the reader
        // threads wait for an unrelated descendant.
        command.args(["-c", "while :; do :; done"]);
        let start = Instant::now();
        let result = runner
            .execute_command(&mut command, Duration::from_millis(100), None)
            .unwrap();

        assert!(result.timed_out);
        assert!(start.elapsed() < Duration::from_secs(2));
    }

    #[cfg(unix)]
    #[test]
    fn expected_output_does_not_hide_a_failed_command() {
        let mut runner = runner();
        runner.espflash = PathBuf::from("sh");
        let result = runner.run_simple_command_test(
            &["-c", "echo expected; exit 1"],
            Some(&["expected"]),
            Duration::from_secs(1),
            "failure",
        );

        assert!(result.is_err());
    }

    #[cfg(unix)]
    #[test]
    fn expected_output_stops_long_running_command_early() {
        let runner = runner();
        let mut command = Command::new("sh");
        command.args(["-c", "echo ready; while :; do :; done"]);
        let start = Instant::now();
        let result = runner
            .execute_command(&mut command, Duration::from_secs(2), Some(&["ready"]))
            .unwrap();

        assert!(!result.timed_out);
        assert!(result.output.contains("ready"));
        assert!(start.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn configured_baud_only_applies_to_transfer_commands() {
        let mut runner = runner();
        runner.baud = Some(921_600);

        let transfer = runner.create_transfer_command(&["flash", "app"]);
        let transfer_args: Vec<_> = transfer
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert_eq!(transfer_args, ["flash", "app", "--baud", "921600"]);

        let regular = runner.create_espflash_command(&["reset"]);
        let regular_args: Vec<_> = regular
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert_eq!(regular_args, ["reset"]);
    }

    #[test]
    fn erased_flash_artifacts_are_limited_to_esp32p4() {
        assert!(TestRunner::is_erased_flash_data(&[0xff; 0x1200], None));

        let mut artifact = vec![0xff; 0x1200];
        artifact[0x400] = 0;
        assert!(TestRunner::is_erased_flash_data(&artifact, Some("esp32p4")));
        assert!(!TestRunner::is_erased_flash_data(
            &artifact,
            Some("esp32c6")
        ));

        artifact[0x300] = 0;
        assert!(!TestRunner::is_erased_flash_data(
            &artifact,
            Some("esp32p4")
        ));
    }
}
