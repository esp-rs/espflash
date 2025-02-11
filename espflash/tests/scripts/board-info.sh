#!/usr/bin/env bash

result=$(espflash board-info)
echo "$result"
if [[ $? -ne 0 || ! "$result" =~ "esp32" ]]; then
    exit 1
fi
