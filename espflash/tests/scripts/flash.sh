#!/usr/bin/env bash
app="espflash/tests/data/$1"
result=$(timeout 8s espflash flash --no-skip --monitor --non-interactive $app 2>&1)
echo "$result"
if [[ ! $result =~ "Flashing has completed!" ]]; then
    echo "Flashing failed!"
    exit 1
fi
if ! echo "$result" | grep -q "Hello world!"; then
    echo "Monitoring failed!"
    exit 1
fi

if [[ "$1" == "esp32c6" ]]; then
    app="${app}_defmt"
    result=$(DEFMT_LOG=info timeout 8s espflash flash --no-skip --monitor --non-interactive $app --log-format defmt 2>&1)
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
