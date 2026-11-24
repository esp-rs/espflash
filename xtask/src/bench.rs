//! Flash performance benchmarks for the HIL runners.
//!
//! Every metric is sampled several times on the device at `ESPFLASH_PORT`, and
//! each metric is printed to stdout as one JSON line holding the median and
//! the 10th and 90th percentiles of its samples.

use std::{
    env,
    path::Path,
    time::{Duration, Instant},
};

use clap::Args;
use espflash::{
    connection::{Connection, ResetAfterOperation, ResetBeforeOperation},
    flasher::Flasher,
    target::DefaultProgressCallback,
};
use serde::Serialize;
use serialport::{FlowControl, SerialPortType};

use crate::{
    Result,
    test_runner::{Console, port_type},
};

/// Start of the flash region the benchmarks overwrite, past the test apps
const OFFSET: u32 = 0x10_0000;
/// Bytes transferred by each sample
const SIZE: u32 = 0x4_0000;

/// Arguments for running the benchmarks
#[derive(Debug, Args)]
pub struct BenchArgs {
    /// Name of the HIL runner the results belong to
    #[arg(long, env = "RUNNER_NAME")]
    pub runner: String,

    /// Baud rate to transfer at after connecting
    #[arg(long, default_value = "115200")]
    pub baud: u32,

    /// Number of samples of each metric. From 7 samples on, the nearest-rank
    /// 10th and 90th percentiles exclude the fastest and slowest sample.
    #[arg(long, default_value = "10")]
    pub samples: usize,

    /// Serial console of the port under test [default: detected from the USB
    /// IDs of `ESPFLASH_PORT`, falling back to uart]
    #[arg(long, value_enum)]
    pub console: Option<Console>,
}

/// One benchmark result, in the record format of the probe-rs dashboard
#[derive(Debug, Serialize)]
struct Record<'a> {
    runner: &'a str,
    chip: &'a str,
    port: Console,
    baud: u32,
    metric: &'a str,
    size: u32,
    unit: &'a str,
    samples: usize,
    median: f64,
    p10: f64,
    p90: f64,
}

/// Runs the benchmarks and prints one JSON line per metric
pub fn run_bench(args: BenchArgs) -> Result<()> {
    if args.samples == 0 {
        return Err("--samples must be at least 1".into());
    }
    let port = env::var("ESPFLASH_PORT").map_err(|_| "ESPFLASH_PORT must name the serial port")?;

    // Reconnecting resets the chip, so each sample includes the reset, the
    // sync, the stub upload and the baud rate change.
    let mut connect = Vec::new();
    let mut flasher = None;
    for _ in 0..args.samples {
        // Close the port before opening it again.
        drop(flasher.take());
        let start = Instant::now();
        flasher = Some(self::connect(&port, args.baud)?);
        connect.push(start.elapsed());
    }
    let mut flasher = flasher.expect("at least one sample was taken");

    // Random data does not compress, so its writes are limited by the link;
    // zeros compress to almost nothing, so their writes are limited by the
    // flash.
    let random = random_bytes(SIZE as usize);
    let zeros = vec![0; SIZE as usize];
    let read_file = env::temp_dir().join("espflash-bench.bin");
    let (mut write_random, mut write_zeros, mut read, mut erase) = (vec![], vec![], vec![], vec![]);
    for _ in 0..args.samples {
        write_random.push(time(|| {
            flasher.write_bin_to_flash(OFFSET, &random, &mut DefaultProgressCallback)
        })?);
        read.push(time(|| {
            flasher.read_flash(OFFSET, SIZE, 0x1000, 64, read_file.clone())
        })?);
        erase.push(time(|| flasher.erase_region(OFFSET, SIZE))?);
        write_zeros.push(time(|| {
            flasher.write_bin_to_flash(OFFSET, &zeros, &mut DefaultProgressCallback)
        })?);
    }

    flasher.connection().reset()?;

    let chip = flasher.chip().to_string();
    let port = args.console.unwrap_or_else(Console::detect);
    let record = |metric, size, unit, values: Vec<f64>| Record {
        runner: &args.runner,
        chip: &chip,
        port,
        baud: args.baud,
        metric,
        size,
        unit,
        samples: values.len(),
        median: percentile(&values, 0.5),
        p10: percentile(&values, 0.1),
        p90: percentile(&values, 0.9),
    };
    let millis = |samples: Vec<Duration>| samples.iter().map(|d| d.as_secs_f64() * 1e3).collect();
    let throughput = |samples: Vec<Duration>| {
        samples
            .iter()
            .map(|d| SIZE as f64 / d.as_secs_f64())
            .collect()
    };

    for record in [
        record("connect", 0, "ms", millis(connect)),
        record("write_random", SIZE, "B/s", throughput(write_random)),
        record("write_zeros", SIZE, "B/s", throughput(write_zeros)),
        record("read", SIZE, "B/s", throughput(read)),
        record("erase", SIZE, "B/s", throughput(erase)),
    ] {
        println!("{}", serde_json::to_string(&record)?);
    }

    Ok(())
}

/// Connects to the device the same way the CLI does by default, except that
/// writes leave the chip in the stub so that the next operation can follow.
fn connect(port: &str, baud: u32) -> Result<Flasher> {
    let Some(SerialPortType::UsbPort(info)) = port_type(Path::new(port)) else {
        return Err(format!("{port} is not a USB serial port").into());
    };
    let serial = serialport::new(port, 115_200)
        .flow_control(FlowControl::None)
        .open_native()?;
    let connection = Connection::new(
        serial,
        info,
        ResetAfterOperation::NoReset,
        ResetBeforeOperation::DefaultReset,
        115_200,
    );

    Ok(Flasher::connect(
        connection,
        true,
        true,
        false,
        None,
        Some(baud),
    )?)
}

fn time(op: impl FnOnce() -> std::result::Result<(), espflash::Error>) -> Result<Duration> {
    let start = Instant::now();
    op()?;
    Ok(start.elapsed())
}

/// Nearest-rank percentile `q` (0 to 1) of `values`.
fn percentile(values: &[f64], q: f64) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    sorted[((sorted.len() - 1) as f64 * q).round() as usize]
}

/// Deterministic xorshift bytes, so every run writes the same data.
fn random_bytes(len: usize) -> Vec<u8> {
    let mut state = 0x2545_f491_4f6c_dd1d_u64;
    (0..len)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state as u8
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentiles_pick_nearest_rank() {
        let values = [10.0, 1.0, 9.0, 2.0, 8.0, 3.0, 7.0, 4.0, 6.0, 5.0];
        assert_eq!(percentile(&values, 0.1), 2.0);
        assert_eq!(percentile(&values, 0.5), 6.0);
        assert_eq!(percentile(&values, 0.9), 9.0);
        assert_eq!(percentile(&[7.0], 0.9), 7.0);
    }
}
