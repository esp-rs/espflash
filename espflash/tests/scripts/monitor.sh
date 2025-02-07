#!/usr/bin/env bash

echo "Monitoring..."
result=$(timeout 5s espflash monitor --non-interactive || true)
echo "$result"
if ! echo "$result" | grep -q "Hello world!"; then
    exit 1
fi
