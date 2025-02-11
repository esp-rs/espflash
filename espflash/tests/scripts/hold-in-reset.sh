#!/usr/bin/env bash

result=$(espflash hold-in-reset 2>&1)
echo "$result"
if [[ ! $result =~ "Holding target device in reset" ]]; then
    exit 1
fi
