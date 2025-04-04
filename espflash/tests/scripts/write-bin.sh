#!/usr/bin/env bash
part_table="espflash/tests/data/partitions.csv"

# https://github.com/esp-rs/espflash/issues/622 reproducer
echo -ne "\x01\xa0" >binary_file.bin
result=$(espflash write-bin 0x0 binary_file.bin 2>&1)
echo "$result"
if [[ ! $result =~ "Binary successfully written to flash!" ]]; then
    echo "Failed to write binary"
    exit 1
fi

result=$(espflash read-flash 0 64 flash_content.bin 2>&1)
echo "$result"
if [[ ! $result =~ "Flash content successfully read and written to" ]]; then
    echo "Failed to read flash content"
    exit 1
fi
# Check that the flash_content.bin contains the '01 a0' bytes
if ! grep -q -a -F $'\x01\xa0' flash_content.bin; then
    echo "Failed verifying content"
    exit 1
fi

result=$(espflash write-bin nvs binary_file.bin --partition-table $part_table 2>&1)
echo "$result"
if [[ ! $result =~ "Binary successfully written to flash!" ]]; then
    echo "Failed to write binary to the nvs partition label"
    exit 1
fi

result=$(espflash read-flash 0x9000 64 flash_content.bin 2>&1)
echo "$result"
if [[ ! $result =~ "Flash content successfully read and written to" ]]; then
    echo "Failed to read flash content"
    exit 1
fi
# Check that the flash_content.bin contains the '01 a0' bytes
if ! grep -q -a -F $'\x01\xa0' flash_content.bin; then
    echo "Failed verifying content"
    exit 1
fi
