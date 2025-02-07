#!/usr/bin/env bash

result=$(espflash erase-region 0x1000 0x1000 2>&1)
echo "$result"
if [[ ! $result =~ "Erasing region at" ]]; then
    exit 1
fi
# Check first 0x1000 bytes are FF
result=$(espflash read-flash 0x1000 0x200 flash_content.bin 2>&1)
echo "$result"
if [[ ! $result =~ "Flash content successfully read and written to" ]]; then
    exit 1
fi
if hexdump -v -e '/1 "%02x"' "flash_content.bin" | grep -qv '^ff*$'; then
    exit 1
fi
# Check next 0x1000 bytes contain some non-FF bytes
result=$(espflash read-flash 0x2000 0x200 flash_content.bin 2>&1)
echo "$result"
if [[ ! $result =~ "Flash content successfully read and written to" ]]; then
    echo "This region should be empty (FF)"
    exit 1
fi
if ! hexdump -v -e '/1 "%02x"' "flash_content.bin" | grep -q '[0-e]'; then
    echo "This region should contain some non-FF bytes"
    exit 1
fi
echo "Flash contents verified!"
