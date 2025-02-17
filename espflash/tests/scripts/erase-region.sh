#!/usr/bin/env bash

# Function to check expected failure for unaligned erase arguments
check_unaligned_erase() {
    local address=$1
    local size=$2
    result=$(espflash erase-region "$address" "$size" 2>&1)
    echo "$result"
    
    if [[ $result =~ "Invalid `address`" ]]; then
        echo "Unaligned erase correctly rejected: address=$address, size=$size"
    else
        echo "Test failed: unaligned erase was not rejected!"
        exit 1
    fi
}

# Unaligned address (not a multiple of 4096)
check_unaligned_erase 0x1001 0x1000

# Unaligned size (not a multiple of 4096)
check_unaligned_erase 0x1000 0x1001

# Both address and size unaligned
check_unaligned_erase 0x1003 0x1005

# Valid erase - should succeed
result=$(espflash erase-region 0x1000 0x1000 2>&1)
echo "$result"
if [[ ! $result =~ "Erasing region at" ]]; then
    exit 1
fi
result=$(espflash read-flash 0x1000 0x2000 flash_content.bin 2>&1)
echo "$result"
if [[ ! $result =~ "Flash content successfully read and written to" ]]; then
    echo "Failed to read flash contents"
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
