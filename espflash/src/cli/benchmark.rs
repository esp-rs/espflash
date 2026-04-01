//! Benchmarks `espflash` connection and transfer paths against a disposable
//! flash region.
//!
//! The implementation intentionally routes operations through the same flasher
//! primitives used by the CLI so the reported numbers stay tied to real device
//! behavior. Connection setup is measured separately, while flash phases keep a
//! single connection open where the underlying transport supports it so the
//! throughput numbers focus on the transfer itself.
//!
//! Each benchmark size runs three phases in order:
//! - `write` programs the scratch region with deterministic test data.
//! - `read` pulls the same region back and validates the contents.
//! - `skip` rewrites the same data to exercise the checksum fast path.

use std::{
    io::{self, IsTerminal, Write},
    time::{Duration, Instant},
};

use clap::Args;
use log::{LevelFilter, max_level, set_max_level};
use md5::{Digest, Md5};
use miette::{Result, miette};

use super::{ConnectArgs, config::Config, connect, parse_u32};
use crate::{
    Error,
    command::{Command, CommandType},
    connection::ResetAfterOperation,
    flasher::{FLASH_SECTOR_SIZE, Flasher},
    target::ProgressCallbacks,
};

const HEADER_LABEL_WIDTH: usize = 16;
const RENDER_INTERVAL: Duration = Duration::from_millis(50);
/// ROM mode only returns up to 64 bytes per `ReadFlashSlow` command.
const ROM_READ_BLOCK_SIZE: usize = 64;

#[derive(Debug, Args, Clone)]
#[non_exhaustive]
pub struct BenchmarkArgs {
    /// Connection configuration
    #[clap(flatten)]
    pub connect_args: ConnectArgs,

    /// Start address of the scratch flash region to benchmark
    ///
    /// WARNING: this command overwrites this region.
    /// The address must point at a disposable region and be sector aligned.
    #[arg(long, value_parser = parse_u32)]
    pub address: u32,

    /// Flash region sizes to benchmark
    #[arg(
        long = "size",
        value_name = "BYTES",
        value_delimiter = ',',
        default_values = ["0x100000"],
        value_parser = parse_u32
    )]
    pub sizes: Vec<u32>,

    /// Number of iterations per benchmark phase
    ///
    /// Connection timing reconnects for every sample. Flash phases reuse a
    /// single connection per phase where possible so transfer rates exclude
    /// reconnect time.
    #[arg(long, default_value_t = 5)]
    pub iterations: usize,

    /// Size of each individual fast-read packet
    #[arg(long, default_value = "0x1000", value_parser = parse_u32)]
    pub block_size: u32,

    /// Maximum number of un-acked fast-read packets
    #[arg(long, default_value_t = 64)]
    pub max_in_flight: u32,
}

#[derive(Debug)]
struct SampleSummary {
    mean: f64,
    stddev: f64,
}

#[derive(Debug)]
struct TransferBenchmark {
    size: u32,
    write: SampleSummary,
    read: SampleSummary,
    skip: SampleSummary,
}

#[derive(Debug)]
struct LogLevelGuard {
    previous: LevelFilter,
}

#[derive(Debug)]
struct BenchmarkProgress {
    label: String,
    interactive: bool,
    rendered_width: usize,
    last_render_at: Option<Instant>,
    phase: &'static str,
    current_iteration: usize,
    total_iterations: usize,
    op_size: usize,
    op_started_at: Instant,
    total_steps: usize,
    current_step: usize,
    latest_sample_bps: Option<f64>,
    verifying: bool,
    last_skipped: bool,
}

/// Run the full benchmark suite for the requested connection and scratch
/// region.
///
/// The reported connection timing includes reconnect cost. Flash phase timings
/// intentionally exclude reconnects where possible so the rates track the read,
/// write, and checksum-skip paths themselves rather than reset behavior.
pub fn benchmark(args: BenchmarkArgs, config: &Config) -> Result<()> {
    #[cfg(debug_assertions)]
    log::warn!("Running in debug mode may result in reduced performance.");
    let _log_level = suppress_benchmark_logs();
    let mut args = args;
    validate_args(&args)?;

    if args.connect_args.port.is_none() {
        let mut resolved = args.connect_args.clone();
        resolved.port =
            Some(super::serial::serial_port_info(&args.connect_args, config)?.port_name);
        args.connect_args = resolved;
    }

    let (chip, baud, baud_ignored) = inspect_device(&args, config)?;
    let max_size = args
        .sizes
        .iter()
        .copied()
        .max()
        .unwrap_or(FLASH_SECTOR_SIZE as u32);
    let test_sizes = args
        .sizes
        .iter()
        .map(|&size| format_size(size as f64))
        .collect::<Vec<_>>()
        .join(", ");

    print_header_field("Chip", chip);
    print_header_field(
        "Loader",
        if args.connect_args.no_stub {
            "ROM"
        } else {
            "stub"
        },
    );
    if baud_ignored {
        print_header_field("Baud", "ignored on USB transport");
    } else {
        print_header_field("Baud", baud);
    }
    print_header_field(
        "Benchmark region",
        format!(
            "{:#x}..{:#x}",
            args.address,
            args.address.saturating_add(max_size)
        ),
    );
    print_header_field("Test size", test_sizes);
    print_header_field("Iterations", args.iterations);
    if args.connect_args.no_stub {
        print_header_field(
            "Read mode",
            "ROM fixed 64 B (`--block-size`/`--max-in-flight` ignored)",
        );
    }
    println!();

    println!("Benchmarking connection time");
    let connection = benchmark_connection(&args, config)?;

    println!("Connection:");
    println!(
        "  connect {:>16} +- {}",
        format_duration(connection.mean),
        format_duration(connection.stddev)
    );
    println!();

    let mut results = Vec::with_capacity(args.sizes.len());
    for &size in &args.sizes {
        results.push(benchmark_size(&args, config, size)?);
    }

    println!("Flash:");
    for result in results {
        println!("  {}", format_size(result.size as f64));
        println!(
            "    write {:>18} +- {}",
            format_rate(result.write.mean),
            format_rate(result.write.stddev)
        );
        println!(
            "    read  {:>18} +- {}",
            format_rate(result.read.mean),
            format_rate(result.read.stddev)
        );
        println!(
            "    skip  {:>18} +- {}",
            format_rate(result.skip.mean),
            format_rate(result.skip.stddev)
        );
    }

    Ok(())
}

fn validate_args(args: &BenchmarkArgs) -> Result<()> {
    let sector_size = FLASH_SECTOR_SIZE as u32;

    if args.iterations == 0 {
        return Err(miette!("`--iterations` must be greater than zero"));
    }

    if args.block_size == 0 {
        return Err(miette!("`--block-size` must be greater than zero"));
    }

    if args.max_in_flight == 0 {
        return Err(miette!("`--max-in-flight` must be greater than zero"));
    }

    if args.address % sector_size != 0 {
        return Err(miette!(
            "benchmark address must be aligned to 0x{FLASH_SECTOR_SIZE:x} bytes"
        ));
    }

    for &size in &args.sizes {
        if size == 0 {
            return Err(miette!("benchmark sizes must be greater than zero"));
        }

        if size % sector_size != 0 {
            return Err(miette!(
                "benchmark sizes must be aligned to 0x{FLASH_SECTOR_SIZE:x} bytes: 0x{size:x}"
            ));
        }

        if args.address.checked_add(size).is_none() {
            return Err(miette!(
                "benchmark range overflows 32-bit address space: address=0x{:x}, size=0x{:x}",
                args.address,
                size
            ));
        }
    }

    Ok(())
}

/// Connect once to inspect the target and validate benchmark prerequisites.
fn inspect_device(args: &BenchmarkArgs, config: &Config) -> Result<(String, u32, bool)> {
    let mut flasher = connect(&args.connect_args, config, true, true)?;

    if flasher.secure_download_mode() {
        return Err(miette!(
            "flash benchmarking is not available in Secure Download Mode because flash reads and skip checks are restricted"
        ));
    }

    let chip = flasher.chip();
    let connection = flasher.connection();
    let baud = connection.baud()?;
    let baud_ignored =
        connection.is_using_usb_serial_jtag() || chip.is_using_usb_otg(connection).unwrap_or(false);
    reset_after_benchmark(&args.connect_args, &mut flasher)?;

    Ok((chip.to_string(), baud, baud_ignored))
}

/// Benchmark connection establishment as an end-to-end reconnect measurement.
fn benchmark_connection(args: &BenchmarkArgs, config: &Config) -> Result<SampleSummary> {
    let mut samples = Vec::with_capacity(args.iterations);

    for _ in 0..args.iterations {
        let started = Instant::now();
        let mut flasher = connect(&args.connect_args, config, true, true)?;
        samples.push(started.elapsed().as_secs_f64());
        reset_after_benchmark(&args.connect_args, &mut flasher)?;
    }

    summarize(&samples)
}

/// Benchmark all transfer phases for a single region size.
///
/// The phases intentionally run in write -> read -> skip order so the scratch
/// region is populated before it is read back and the skip phase can reuse the
/// exact bytes written earlier.
fn benchmark_size(args: &BenchmarkArgs, config: &Config, size: u32) -> Result<TransferBenchmark> {
    let pattern = generate_pattern(size as usize);
    let expected_md5 = Md5::digest(&pattern);
    let mut progress = BenchmarkProgress::new(format_size(size as f64));

    let write = benchmark_write(args, config, size, &pattern, &expected_md5, &mut progress)?;
    let read = benchmark_read(args, config, size, &expected_md5, &mut progress)?;
    let skip = benchmark_skip(args, config, size, &pattern, &mut progress)?;

    Ok(TransferBenchmark {
        size,
        write,
        read,
        skip,
    })
}

/// Benchmark write throughput for a scratch region.
///
/// In stub mode the benchmark keeps one connection open across samples to match
/// normal high-throughput flashing. ROM mode reconnects per sample because its
/// slower path is typically dominated less by steady-state transfer speed and
/// per-sample verification is performed through a fresh connection.
fn benchmark_write(
    args: &BenchmarkArgs,
    config: &Config,
    size: u32,
    pattern: &[u8],
    expected_md5: &[u8],
    progress: &mut BenchmarkProgress,
) -> Result<SampleSummary> {
    if args.connect_args.no_stub {
        return run_phase(
            progress,
            "write",
            args.iterations,
            size as usize,
            |progress| {
                let mut flasher = connect_for_phase(args, config, false)?;
                let started = Instant::now();
                flasher.write_bin_to_flash(args.address, pattern, progress)?;
                let sample = throughput(size, started.elapsed());
                drop(flasher);

                let mut verify_flasher = connect_for_phase(args, config, false)?;
                verify_flash_contents(&mut verify_flasher, args.address, size, expected_md5)?;
                Ok(sample)
            },
        );
    }

    run_connected_phase(
        args,
        config,
        progress,
        "write",
        size as usize,
        false,
        |flasher, progress| {
            let started = Instant::now();
            flasher.write_bin_to_flash(args.address, pattern, progress)?;
            let sample = throughput(size, started.elapsed());
            verify_flash_contents(flasher, args.address, size, expected_md5)?;
            Ok(sample)
        },
    )
}

/// Benchmark flash read throughput using the same protocol commands as the CLI.
fn benchmark_read(
    args: &BenchmarkArgs,
    config: &Config,
    size: u32,
    expected_md5: &[u8],
    progress: &mut BenchmarkProgress,
) -> Result<SampleSummary> {
    run_connected_phase(
        args,
        config,
        progress,
        "read",
        size as usize,
        false,
        |flasher, progress| {
            let started = Instant::now();
            let readback = read_flash_region(
                flasher,
                args.address,
                size,
                args.block_size,
                args.max_in_flight,
                !args.connect_args.no_stub,
                progress,
            )?;

            if Md5::digest(&readback)[..] != expected_md5[..] {
                return Err(miette!("readback contents did not match expected data"));
            }

            Ok(throughput(size, started.elapsed()))
        },
    )
}

/// Benchmark the checksum-based fast-skip path by rewriting identical bytes.
fn benchmark_skip(
    args: &BenchmarkArgs,
    config: &Config,
    size: u32,
    pattern: &[u8],
    progress: &mut BenchmarkProgress,
) -> Result<SampleSummary> {
    if args.connect_args.no_stub {
        return run_phase(
            progress,
            "skip",
            args.iterations,
            size as usize,
            |progress| {
                let mut flasher = connect_for_phase(args, config, true)?;
                let started = Instant::now();
                flasher.write_bin_to_flash(args.address, pattern, progress)?;
                let sample = throughput(size, started.elapsed());
                progress.require_skip()?;
                Ok(sample)
            },
        );
    }

    run_connected_phase(
        args,
        config,
        progress,
        "skip",
        size as usize,
        true,
        |flasher, progress| {
            let started = Instant::now();
            flasher.write_bin_to_flash(args.address, pattern, progress)?;
            let sample = throughput(size, started.elapsed());
            progress.require_skip()?;
            Ok(sample)
        },
    )
}

/// Run one benchmark phase while reusing a single connection across samples.
///
/// This is used for steady-state measurements where reconnect time would drown
/// out the operation we actually want to observe.
fn run_connected_phase<F>(
    args: &BenchmarkArgs,
    config: &Config,
    progress: &mut BenchmarkProgress,
    phase: &'static str,
    op_size: usize,
    skip_enabled: bool,
    mut run_sample: F,
) -> Result<SampleSummary>
where
    F: FnMut(&mut Flasher, &mut BenchmarkProgress) -> Result<f64>,
{
    let mut flasher = connect_for_phase(args, config, skip_enabled)?;
    let summary = run_phase(progress, phase, args.iterations, op_size, |progress| {
        run_sample(&mut flasher, progress)
    })?;
    reset_after_benchmark(&args.connect_args, &mut flasher)?;
    Ok(summary)
}

/// Run one phase repeatedly and summarize its per-iteration samples.
fn run_phase<F>(
    progress: &mut BenchmarkProgress,
    phase: &'static str,
    iterations: usize,
    op_size: usize,
    mut run_sample: F,
) -> Result<SampleSummary>
where
    F: FnMut(&mut BenchmarkProgress) -> Result<f64>,
{
    let mut samples = Vec::with_capacity(iterations);

    for iteration in 1..=iterations {
        progress.begin_phase(phase, iteration, iterations, op_size);
        let sample = run_sample(progress)?;
        samples.push(sample);
        progress.complete_sample(sample);
    }

    summarize(&samples)
}

/// Connect with reset behavior adjusted for repeated benchmark samples.
///
/// Benchmark phases manage their own reset lifecycle so they can reuse a single
/// connection without paying the normal post-operation reset cost after each
/// sample.
fn connect_for_phase(args: &BenchmarkArgs, config: &Config, skip_enabled: bool) -> Result<Flasher> {
    // Clone connection arguments and override the post-operation reset policy.
    let mut connect_args = args.connect_args.clone();
    connect_args.after = if connect_args.no_stub {
        ResetAfterOperation::NoReset
    } else {
        ResetAfterOperation::NoResetNoStub
    };
    connect(&connect_args, config, true, !skip_enabled)
}

impl BenchmarkProgress {
    fn new(label: String) -> Self {
        Self {
            label,
            interactive: io::stderr().is_terminal(),
            rendered_width: 0,
            last_render_at: None,
            phase: "write",
            current_iteration: 0,
            total_iterations: 1,
            op_size: 1,
            op_started_at: Instant::now(),
            total_steps: 1,
            current_step: 0,
            latest_sample_bps: None,
            verifying: false,
            last_skipped: false,
        }
    }

    fn begin_phase(
        &mut self,
        phase: &'static str,
        iteration: usize,
        total_iterations: usize,
        op_size: usize,
    ) {
        if self.interactive
            && self.rendered_width > 0
            && self.current_iteration != 0
            && phase != self.phase
        {
            eprintln!();
            self.rendered_width = 0;
        }

        self.phase = phase;
        self.current_iteration = iteration;
        self.total_iterations = total_iterations;
        self.op_size = op_size.max(1);
        self.op_started_at = Instant::now();
        self.total_steps = 1;
        self.current_step = 0;
        self.latest_sample_bps = None;
        self.verifying = false;
        self.last_skipped = false;
        self.render(true);
    }

    fn complete_sample(&mut self, sample_bps: f64) {
        self.current_step = self.total_steps;
        self.latest_sample_bps = Some(sample_bps);
        self.verifying = false;
        self.render(true);
    }

    fn require_skip(&self) -> Result<()> {
        if self.last_skipped {
            Ok(())
        } else {
            Err(miette!(
                "skip benchmark did not take the checksum fast-skip path"
            ))
        }
    }

    fn render(&mut self, force: bool) {
        let now = Instant::now();
        if !force
            && self
                .last_render_at
                .is_some_and(|last| now.duration_since(last) < RENDER_INTERVAL)
        {
            return;
        }

        let line = progress_line(
            &self.label,
            self.phase,
            self.current_iteration,
            self.total_iterations,
            self.percent(),
            self.latest_sample_bps,
            self.verifying,
        );

        if self.interactive {
            if self.rendered_width == 0 {
                eprint!("{line}");
            } else {
                let padding = " ".repeat(self.rendered_width.saturating_sub(line.len()));
                eprint!("\r{line}{padding}");
            }
            let _ = io::stderr().flush();
            self.rendered_width = line.len();
        } else if force {
            eprintln!("{line}");
        }

        self.last_render_at = Some(now);
    }

    fn percent(&self) -> usize {
        self.current_step.saturating_mul(100) / self.total_steps.max(1)
    }

    fn update_live_rate(&mut self, bytes_done: usize) {
        let elapsed = self.op_started_at.elapsed();
        if !elapsed.is_zero() && bytes_done > 0 {
            self.latest_sample_bps = Some(bytes_done as f64 / elapsed.as_secs_f64());
        }
    }
}

impl ProgressCallbacks for BenchmarkProgress {
    fn init(&mut self, _addr: u32, total: usize) {
        self.total_steps = total.max(1);
        self.current_step = 0;
        self.verifying = false;
    }

    fn update(&mut self, current: usize) {
        self.current_step = current.min(self.total_steps);
        let bytes_done = self.op_size.saturating_mul(self.current_step) / self.total_steps;
        self.update_live_rate(bytes_done);
        self.render(false);
    }

    fn verifying(&mut self) {
        self.current_step = self.total_steps;
        self.verifying = true;
        self.render(true);
    }

    fn finish(&mut self, skipped: bool) {
        self.last_skipped = skipped;
        self.current_step = self.total_steps;
        self.verifying = false;
        self.update_live_rate(self.op_size);
        self.last_render_at = None;
    }
}

impl Drop for BenchmarkProgress {
    fn drop(&mut self) {
        if self.interactive && self.rendered_width > 0 {
            eprintln!();
        }
    }
}

impl Drop for LogLevelGuard {
    fn drop(&mut self) {
        set_max_level(self.previous);
    }
}

/// Temporarily reduce global logging noise while a benchmark is running.
///
/// Benchmark progress is rendered directly to stderr, so verbose `info!` and
/// `debug!` output from the normal flashing code would clutter the display. The
/// guard mainly matters when `espflash` is used as a library, where the caller
/// should get its previous global log level back after the benchmark.
fn suppress_benchmark_logs() -> LogLevelGuard {
    let previous = max_level();
    set_max_level(LevelFilter::Warn);
    LogLevelGuard { previous }
}

fn print_header_field(label: &str, value: impl std::fmt::Display) {
    println!("{label:<HEADER_LABEL_WIDTH$} {value}");
}

fn reset_after_benchmark(connect_args: &ConnectArgs, flasher: &mut Flasher) -> Result<()> {
    let chip = flasher.chip();
    flasher
        .connection()
        .reset_after(!connect_args.no_stub, chip)?;
    Ok(())
}

fn read_flash_region(
    flasher: &mut Flasher,
    offset: u32,
    size: u32,
    block_size: u32,
    max_in_flight: u32,
    use_stub: bool,
    progress: &mut BenchmarkProgress,
) -> Result<Vec<u8>> {
    if use_stub {
        read_flash_region_stub(flasher, offset, size, block_size, max_in_flight, progress)
    } else {
        read_flash_region_rom(flasher, offset, size, block_size, max_in_flight, progress)
    }
}

fn read_flash_region_stub(
    flasher: &mut Flasher,
    offset: u32,
    size: u32,
    block_size: u32,
    max_in_flight: u32,
    progress: &mut BenchmarkProgress,
) -> Result<Vec<u8>> {
    let total_steps = (size as usize).div_ceil(block_size as usize).max(1);
    let connection = flasher.connection();
    let mut data = Vec::with_capacity(size as usize);

    progress.init(offset, total_steps);

    connection.with_timeout(CommandType::ReadFlash.timeout(), |connection| {
        connection.command(Command::ReadFlash {
            offset,
            size,
            block_size,
            max_in_flight,
        })
    })?;

    while data.len() < size as usize {
        let response = connection.read_flash_response()?;
        let chunk: Vec<u8> = if let Some(response) = response {
            response.value.try_into()?
        } else {
            return Err(Error::IncorrectResponse.into());
        };

        data.extend_from_slice(&chunk);
        progress.update(data.len().div_ceil(block_size as usize).min(total_steps));

        if data.len() < size as usize && chunk.len() < block_size as usize {
            return Err(Error::CorruptData(block_size as usize, chunk.len()).into());
        }

        connection.write_raw(data.len() as u32)?;
    }

    if data.len() > size as usize {
        return Err(Error::ReadMoreThanExpected.into());
    }

    let response = connection.read_flash_response()?;
    let digest: Vec<u8> = if let Some(response) = response {
        response.value.try_into()?
    } else {
        return Err(Error::IncorrectResponse.into());
    };

    if digest.len() != 16 {
        return Err(Error::IncorrectDigestLength(digest.len()).into());
    }

    let expected_digest = Md5::digest(&data);
    if digest != expected_digest[..] {
        return Err(Error::DigestMismatch(digest, expected_digest.to_vec()).into());
    }

    progress.finish(false);

    Ok(data)
}

/// Read flash through the ROM's fixed-size `ReadFlashSlow` command.
///
/// Even though the command accepts `block_size` and `max_in_flight`, the ROM
/// protocol is effectively fixed at 64-byte chunks, so progress is tracked in
/// those units to better match the device behavior being measured.
fn read_flash_region_rom(
    flasher: &mut Flasher,
    offset: u32,
    size: u32,
    block_size: u32,
    max_in_flight: u32,
    progress: &mut BenchmarkProgress,
) -> Result<Vec<u8>> {
    let total_steps = (size as usize).div_ceil(ROM_READ_BLOCK_SIZE).max(1);
    let connection = flasher.connection();
    let mut data = Vec::with_capacity(size as usize);

    progress.init(offset, total_steps);

    while data.len() < size as usize {
        let chunk_len = usize::min(ROM_READ_BLOCK_SIZE, size as usize - data.len());
        let chunk_offset = offset + data.len() as u32;
        let response =
            connection.with_timeout(CommandType::ReadFlashSlow.timeout(), |connection| {
                connection.command(Command::ReadFlashSlow {
                    offset: chunk_offset,
                    size: chunk_len as u32,
                    block_size,
                    max_in_flight,
                })
            })?;

        let payload: Vec<u8> = response.try_into()?;
        if payload.len() < chunk_len {
            return Err(Error::CorruptData(chunk_len, payload.len()).into());
        }

        data.extend_from_slice(&payload[..chunk_len]);
        progress.update(data.len().div_ceil(ROM_READ_BLOCK_SIZE).min(total_steps));
    }

    progress.finish(false);

    Ok(data)
}

/// Verify that the benchmark region still matches the expected contents.
fn verify_flash_contents(
    flasher: &mut Flasher,
    address: u32,
    size: u32,
    expected_md5: &[u8],
) -> Result<()> {
    if flasher.checksum_md5(address, size)?.to_be_bytes() == expected_md5 {
        Ok(())
    } else {
        Err(Error::VerifyFailed.into())
    }
}

/// Generate deterministic synthetic test data for scratch-region benchmarks.
///
/// The benchmark uses generated bytes instead of a bundled fixture so every run
/// can materialize the same workload for an arbitrary region size without extra
/// file management. This remains a synthetic workload rather than a firmware
/// image, which is important to remember when comparing stub compression ratios
/// to real application flashes.
fn generate_pattern(size: usize) -> Vec<u8> {
    (0..size)
        .map(|index| {
            let mixed = (index as u32)
                .wrapping_mul(1_664_525)
                .wrapping_add(0xA5A5_5A5A)
                .rotate_left((index % 31) as u32);

            (mixed ^ (mixed >> 8) ^ (mixed >> 16) ^ (mixed >> 24)) as u8
        })
        .collect()
}

fn progress_line(
    label: &str,
    phase: &str,
    iteration: usize,
    total_iterations: usize,
    percent: usize,
    latest_sample_bps: Option<f64>,
    verifying: bool,
) -> String {
    let iteration_width = total_iterations.to_string().len();
    let sample = latest_sample_bps.map(format_rate).unwrap_or_default();
    let status = if verifying { "verifying" } else { "" };

    format!(
        "Benchmarking {label:>10} region  {phase:<5}  ({iteration:>iteration_width$}/{total_iterations:>iteration_width$}, {percent:>3}%)  {sample:>14}  {status:<9}",
    )
}

fn throughput(size: u32, elapsed: Duration) -> f64 {
    if elapsed.is_zero() {
        f64::INFINITY
    } else {
        size as f64 / elapsed.as_secs_f64()
    }
}

fn summarize(samples: &[f64]) -> Result<SampleSummary> {
    if samples.is_empty() {
        return Err(miette!("benchmark produced no samples"));
    }

    let mean = samples.iter().sum::<f64>() / samples.len() as f64;
    let stddev = std_deviation(samples, mean);
    Ok(SampleSummary { mean, stddev })
}

fn std_deviation(samples: &[f64], mean: f64) -> f64 {
    if samples.len() < 2 {
        return 0.0;
    }

    let variance = samples
        .iter()
        .map(|sample| {
            let delta = mean - sample;
            delta * delta
        })
        .sum::<f64>()
        / (samples.len() - 1) as f64;

    variance.sqrt()
}

fn format_size(bytes: f64) -> String {
    format_scaled(bytes, &["B", "KiB", "MiB", "GiB"])
}

fn format_rate(bytes_per_second: f64) -> String {
    format!(
        "{}/s",
        format_scaled(bytes_per_second, &["B", "KiB", "MiB", "GiB"])
    )
}

fn format_duration(seconds: f64) -> String {
    if seconds >= 1.0 {
        format!("{seconds:.2}s")
    } else {
        format!("{:.2}ms", seconds * 1_000.0)
    }
}

fn format_scaled(mut value: f64, units: &[&str]) -> String {
    let mut unit = 0;

    while value >= 1024.0 && unit < units.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }

    format!("{value:.2} {}", units[unit])
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    #[derive(Parser)]
    struct TestParser {
        #[command(flatten)]
        args: BenchmarkArgs,
    }

    fn args() -> BenchmarkArgs {
        TestParser::parse_from(["espflash", "--address", "0x1000", "--size", "0x1000,0x2000"]).args
    }

    #[test]
    fn benchmark_args_require_sector_alignment() {
        let mut parsed = args();
        parsed.address = 1;
        assert!(validate_args(&parsed).is_err());

        let mut parsed = args();
        parsed.sizes = vec![0x1800];
        assert!(validate_args(&parsed).is_err());
    }

    #[test]
    fn benchmark_args_guards_against_overflow() {
        let mut parsed = args();
        // Closest number to u32::MAX aligned to `FLASH_SECTOR_SIZE`
        parsed.sizes = vec![!(FLASH_SECTOR_SIZE as u32 - 1)];
        assert!(validate_args(&parsed).is_err());
    }
}
