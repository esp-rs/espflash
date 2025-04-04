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
