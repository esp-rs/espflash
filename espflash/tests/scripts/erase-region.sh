#!/usr/bin/env bash

result=$(espflash erase-region 0x1000 0x1000 2>&1)
echo "$result"
if [[ ! $result =~ "Erasing region at" ]]; then
    exit 1
fi
# TODO: Once https://github.com/esp-rs/espflash/issues/697 is resolved we should look like:
# https://github.com/esp-rs/espflash/pull/754/commits/288eced61e7b21deface52a67e2f023b388ce6ed#diff-083bacee91d55c6adddc9dcd306da31db24e33591d5453e819999552995b85b7R8-R23

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
