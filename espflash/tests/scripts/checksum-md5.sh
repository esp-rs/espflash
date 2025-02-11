#!/usr/bin/env bash

result=$(espflash erase-flash 2>&1)
echo "$result"
if [[ ! $result =~ "Flash has been erased!" ]]; then
    exit 1
fi
result=$(espflash checksum-md5 --address 0x1000 --length 0x100 2>&1)
echo "$result"
if [[ ! $result =~ "0x827f263ef9fb63d05499d14fcef32f60" ]]; then
    exit 1
fi
