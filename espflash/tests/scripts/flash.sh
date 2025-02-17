#!/usr/bin/env bash
app="espflash/tests/data/$1"

if [[ "$1" == "esp32c6" ]]; then
    app_defmt="${app}_defmt"
    result=$(timeout 8s espflash flash --no-skip --monitor --non-interactive $app_defmt --log-format defmt 2>&1)
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
