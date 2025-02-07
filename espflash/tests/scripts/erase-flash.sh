#!/usr/bin/env bash

result=$(espflash erase-flash 2>&1)
echo "$result"
if [[ ! $result =~ "Flash has been erased!" ]]; then
    exit 1
fi
result=$(espflash read-flash 0 0x200 flash_content.bin 2>&1)
echo "$result"
if [[ ! $result =~ "Flash content successfully read and written to" ]]; then
    exit 1
fi
echo "Checking if flash is empty"
if hexdump -v -e '/1 "%02x"' "flash_content.bin" | grep -qv '^ff*$'; then
    exit 1
fi
echo "Flash is empty!"
