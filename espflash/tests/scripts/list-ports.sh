#!/usr/bin/env bash

result=$(espflash list-ports 2>&1)
echo "$result"
if [[ ! $result =~ "Silicon Labs" ]]; then
    exit 1
fi
