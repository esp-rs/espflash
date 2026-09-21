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
is submitted. Full HIL runs automatically when a pull request enters the merge
queue; ordinary pull request events do not occupy the hardware runners.
