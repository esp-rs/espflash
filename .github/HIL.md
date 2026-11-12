# GitHub workflows

## HIL commands

Hardware-in-the-loop (HIL) tests can be requested by commenting on an open
pull request:

- `/hil quick` — test ESP32-C6 and ESP32-S3.
- `/hil full` — run all HIL tests, including secure download mode (SDM).
- `/hil sdm` — run only the ESP32-C6 SDM tests.
- `/hil <chip1> [chip2...]` — test all configured ports for selected chips.
- `/hil` or `/hil help` — show command usage.

Repository members and owners can run these commands directly. Other
contributors must first be trusted for that pull request by a repository
member:

- `/trust @user` — allow a contributor to run HIL commands.
- `/revoke @user` — remove that permission.

Applying the `trusted-author` label also trusts the pull request author.

Each command tests the exact pull request head commit at the time the command
is submitted. The bot replies with a link to the triggered run and edits that
comment with the result (succeeded, failed, or cancelled) when the run
finishes, and again whenever the run is re-run.

Full HIL runs automatically when a pull request enters the merge queue;
ordinary pull request events do not occupy the hardware runners.

## Benchmarks

After its tests, each HIL job runs `xtask bench`, which measures the connect
time and the write, read and erase throughput of the device. Merge queue runs
publish these results to the `bench-data` branch as `<commit>.jsonl`, together
with the dashboard in `.github/bench/index.html`, which GitHub Pages serves
from that branch. To benchmark a local device with a release build, as HIL
does:

```sh
ESPFLASH_PORT=/dev/ttyACM0 cargo run --release -p xtask -- bench --runner local --baud 921600
```
