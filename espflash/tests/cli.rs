use assert_cmd::prelude::*;
use std::process::{exit, Command};
use std::thread::sleep;
use std::time::Duration;

#[test]
fn cli_tests() -> Result<(), Box<dyn std::error::Error>> {
    // board-info
    let mut cmd: Command = Command::cargo_bin("espflash")?;
    cmd.arg("board-info");

    let binding = cmd.assert().success();
    let output = binding.get_output();

    let output_stdout = String::from_utf8_lossy(&output.stdout).to_string();

    assert!(output_stdout.contains("esp32"));
    assert!(output_stdout.contains("revision"));

    // // flash
    let image = std::env::var("ESPFLASH_APP").expect("ESPFLASH_APP not set");

    let mut child = Command::cargo_bin("espflash")?
        .args(&["flash", "--monitor", &image])
        .stdout(std::process::Stdio::piped())
        // .stderr(std::process::Stdio::piped())
        .spawn()?;

    // // Sleep for 10 seconds
    sleep(Duration::from_secs(10));

    // Check if the child process is still running
    match child.try_wait() {
        Ok(Some(_status)) => {
            // The process has terminated
        }
        Ok(None) => {
            // The process is still running, kill it
            child.kill().expect("command wasn't running");
            child.wait().expect("unable to wait on child");
        }
        Err(e) => {
            panic!("Error attempting to wait on child: {}", e);
        }
    }

    let output = child.wait_with_output().expect("Failed to read stdout");
    let output = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(output.contains("Hello world!"));

    // // monitor
    // let mut cmd: Command = Command::cargo_bin("espflash")?;
    // cmd.arg("monitor");

    // let binding = cmd.assert().success();
    // let output = binding.get_output();

    // let output_stdout = String::from_utf8_lossy(&output.stdout).to_string();

    // assert!(output_stdout.contains("Hello world!"));

    Ok(())
}
