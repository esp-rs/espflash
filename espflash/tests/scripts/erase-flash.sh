#!/usr/bin/env bash

result=$(espflash erase-region 0x1000 0x1000 2>&1)
echo "$result"
if [[ ! $result =~ "Erasing region at" ]]; then
    exit 1
fi
echo "Flash region erased!"
echo "Reading flash content..."
result=$(espflash read-flash 0x1000 0x2000 flash_content.bin 2>&1)
echo "$result"
if [[ ! $result =~ "Flash content successfully read and written to" ]]; then
    exit 1
fi
# Check first 0x1000 bytes are FF
if head -c 4096 flash_content.bin | hexdump -v -e '/1 "%02x"' | grep -qv '^ff*$'; then
    echo "First 0x1000 bytes should be empty (FF)"
    exit 1
fi
# Check next 0x1000 bytes contain some non-FF bytes
if ! tail -c 4096 flash_content.bin | hexdump -v -e '/1 "%02x"' | grep -q '[0-e]'; then
    echo "Next 0x1000 bytes should contain some non-FF bytes"
    exit 1
fi
echo "Flash contents verified!"
