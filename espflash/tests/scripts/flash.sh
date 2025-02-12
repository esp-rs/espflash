#!/usr/bin/env bash

result=$(timeout 8s espflash flash --no-skip --monitor --non-interactive $1 2>&1)
echo "$result"
if [[ ! $result =~ "Flashing has completed!" ]]; then
    echo "Flashing failed!"
    exit 1
fi
if ! echo "$result" | grep -q "Hello world!"; then
    echo "Monitoring failed!"
    exit 1
fi
