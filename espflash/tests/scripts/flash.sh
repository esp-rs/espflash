#!/usr/bin/env bash

result=$(espflash flash --no-skip $1 2>&1)
echo "$result"
if [[ ! $result =~ "Flashing has completed!" ]]; then
    exit 1
fi
