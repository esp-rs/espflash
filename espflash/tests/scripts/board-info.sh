#!/usr/bin/env bash

# Run the command and capture output and exit code
result=$(espflash board-info)
exit_code=$?
echo "$result"

chip_type=$(echo "$result" | awk -F': *' '/Chip type:/ {print $2}' | awk '{print $1}')


if [[ "$chip_type" == "esp32" ]]; then
    # ESP32 should fail because it doesn't support get_security_info
    if [[ $exit_code -eq 0 ]]; then
        echo "Expected failure for ESP32 but command succeeded"
        exit 1
    fi
else
    # Non-ESP32 should succeed (zero exit code)
    if [[ $exit_code -ne 0 ]]; then
        echo "Expected success for non-ESP32 but command failed"
        exit 1
    fi

    # Ensure Security Information and Flags are present
    if [[ ! "$result" =~ "Security Information:" || ! "$result" =~ "Flags" ]]; then
        echo "Expected 'Security Information:' and 'Flags' in output but did not find them"
        exit 1
    fi
fi
