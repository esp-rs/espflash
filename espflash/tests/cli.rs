use assert_cmd::prelude::*; // Add methods on commands
                            // use predicates::prelude::*; // Used for writing assertions
use std::process::Command; // Run programs

#[test]
fn flash() -> Result<(), Box<dyn std::error::Error>> {
    // board-info
    let mut cmd: Command = Command::cargo_bin("espflash")?;

    cmd.arg("board-info");
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("espflash")?;

    // flash
    let image = std::env::var("ESPFLASH_APP").expect("ESPFLASH_APP not set");

    cmd.arg("flash").arg(image);
    cmd.assert().success();

    Ok(())
}
