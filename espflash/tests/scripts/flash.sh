#!/usr/bin/env bash
app="espflash/tests/data/$1"
part_table="espflash/tests/data/partitions.csv"

# espflash should not flash a partition table that is too big
result=$(timeout 15s espflash flash --no-skip --monitor --non-interactive $app --flash-size 2mb --partition-table $part_table 2>&1)
echo "$result"
if [[ $result =~ "Flashing has completed!" ]]; then
    echo "Flashing should have failed!"
    exit 1
fi
if [[ $result =~ "espflash::partition_table::does_not_fit" ]]; then
    echo "Flashing failed as expected!"
else
    echo "Flashing has failed but not with the expected error!"
    exit 1
fi

if [[ "$1" == "esp32c6" ]]; then
    # With manual log-format
    app_defmt="${app}_defmt"
    result=$(timeout 15s espflash flash --no-skip --monitor --non-interactive $app_defmt --log-format defmt 2>&1)
    echo "$result"
    if [[ ! $result =~ "Flashing has completed!" ]]; then
        echo "Flashing failed!"
        exit 1
    fi
    if ! echo "$result" | grep -q "Hello world!"; then
        echo "Monitoring failed!"
        exit 1
    fi

    # With auto-detected log-format
    result=$(timeout 15s espflash flash --no-skip --monitor --non-interactive $app_defmt 2>&1)
    echo "$result"
    if [[ ! $result =~ "Flashing has completed!" ]]; then
        echo "Flashing failed!"
        exit 1
    fi
    if ! echo "$result" | grep -q "Hello world!"; then
        echo "Monitoring failed!"
        exit 1
    fi

    # Backtrace test
    app_backtrace="${app}_backtrace"

    result=$(timeout 10s espflash flash --no-skip --monitor --non-interactive $app_backtrace 2>&1)
    echo "$result"
    if [[ ! $result =~ "Flashing has completed!" ]]; then
        echo "Flashing failed!"
        exit 1
    fi
    expected_strings=(
        "0x420012c8"
        "main"
        "esp32c6_backtrace/src/bin/main.rs:"
        "0x42001280"
        "hal_main"
    )
    for expected in "${expected_strings[@]}"; do
        if ! echo "$result" | grep -q "$expected"; then
            echo "Monitoring failed! Expected '$expected' not found in output."
            exit 1
        fi
    done
fi

result=$(timeout 15s espflash flash --no-skip --monitor --non-interactive $app 2>&1)
echo "$result"
if [[ ! $result =~ "Flashing has completed!" ]]; then
    echo "Flashing failed!"
    exit 1
fi
if ! echo "$result" | grep -q "Hello world!"; then
    echo "Monitoring failed!"
    exit 1
fi

# Test with a higher baud rate
result=$(timeout 15s espflash flash --no-skip --monitor --non-interactive --baud 921600 $app 2>&1 | tr -d '\0')
echo "$result"
if [[ ! $result =~ "Flashing has completed!" ]]; then
    echo "Flashing failed!"
    exit 1
fi
if ! echo "$result" | grep -q "Hello world!"; then
    echo "Monitoring failed!"
    exit 1
fi
