#!/usr/bin/env bash

result=$(espflash reset 2>&1)
echo "$result"
if [[ ! $result =~ "Resetting target device" ]]; then
    exit 1
fi
