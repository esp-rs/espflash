#!/usr/bin/env bash

result=$(espflash save-image --merge --chip $1 $2 $3 app.bin 2>&1)
echo "$result"
if [[ ! $result =~ "Image successfully saved!" ]]; then
    exit 1
fi
echo "Writting binary"
result=$(espflash write-bin 0x0 app.bin 2>&1)
echo "$result"
if [[ ! $result =~ "Binary successfully written to flash!" ]]; then
    exit 1
fi
