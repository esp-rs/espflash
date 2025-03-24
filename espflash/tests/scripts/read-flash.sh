#!/usr/bin/env bash

KNOWN_PATTERN=$'\x01\xa0\x02\xB3\x04\xC4\x08\xD5\x10\xE6\x20\xF7\x40\x88\x50\x99'  
KNOWN_PATTERN+=$'\x60\xAA\x70\xBB\x80\xCC\x90\xDD\xA0\xEE\xB0\xFF\xC0\x11\xD0\x22'  
KNOWN_PATTERN+=$'\xE0\x33\xF0\x44\x05\x55\x15\x66\x25\x77\x35\x88\x45\x99\x55\xAA'  
KNOWN_PATTERN+=$'\x65\xBB\x75\xCC\x85\xDD\x95\xEE\xA5\xFF\xB5\x00\xC5\x11\xD5\x22'  
KNOWN_PATTERN+=$'\xE5\x33\xF5\x44\x06\x55\x16\x66\x26\x77\x36\x88\x46\x99\x56\xAA'  
KNOWN_PATTERN+=$'\x66\xBB\x76\xCC\x86\xDD\x96\xEE\xA6\xFF\xB6\x00\xC6\x11\xD6\x22'

echo -ne "$KNOWN_PATTERN" > pattern.bin
result=$(espflash write-bin 0x0 pattern.bin 2>&1)
echo "$result"
if [[ ! $result =~ "Binary successfully written to flash!" ]]; then
    echo "Failed to write binary to flash"
    exit 1
fi

lengths=(2 5 10 26 44 86)

for len in "${lengths[@]}"; do
    echo "Testing read-flash with length: $len"

    result=$(espflash read-flash 0 "$len" flash_content.bin 2>&1)
    echo "$result"
    if [[ ! $result =~ "Flash content successfully read and written to" ]]; then
        echo "Failed to read $len bytes from flash"
        exit 1
    fi

    EXPECTED=$(echo -ne "$KNOWN_PATTERN" | head -c "$len")

    if ! cmp -s <(echo -ne "$EXPECTED") flash_content.bin; then
        echo "Verification failed: content does not match expected for length"
        exit 1
    fi

    echo "Testing ROM read-flash with length: $len"
    result=$(espflash read-flash --no-stub 0 "$len" flash_content.bin 2>&1)
    echo "$result"

    if ! cmp -s <(echo -ne "$EXPECTED") flash_content.bin; then
        echo "Verification failed: content does not match expected for length"
        exit 1
    fi

    if [[ ! $result =~ "Flash content successfully read and written to" ]]; then
        echo "Failed to read $len bytes from flash"
        exit 1
    fi

done

echo "All read-flash tests passed!"
