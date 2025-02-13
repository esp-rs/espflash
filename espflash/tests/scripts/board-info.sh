#!/usr/bin/env bash

# Run the command and capture output and exit code
result=$(espflash board-info)
exit_code=$?
echo "$result"

# Extract chip type
chip_type=$(awk -F': *' '/Chip type:/ {print $2}' <<< "$result" | awk '{print $1}')

if [[ "$chip_type" == "esp32" ]]; then
    # ESP32 doesn't support get_security_info
    [[ $exit_code -eq 0 && "$result" =~ "Security features: None" ]] || {
        echo "Expected Security features: None"
        exit 1
    }
else
    # Non-ESP32 should contain required info
    [[ $exit_code -eq 0 && "$result" =~ "Security Information:" && "$result" =~ "Flags" ]] || {
        echo "Expected 'Security Information:' and 'Flags' in output but did not find them"
        exit 1
    }
fi
