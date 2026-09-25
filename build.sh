#!/usr/bin/env bash
set -e

echo "=== Building Charybdis Mini RMK Release Binaries ==="
source ~/.cargo/env 2>/dev/null || true
cargo build --release

echo "=== Converting ELFs to HEX and UF2 ==="
mkdir -p dist

# Convert to HEX
arm-none-eabi-objcopy -O ihex target/thumbv7em-none-eabihf/release/central dist/charybdis_right_central.hex
arm-none-eabi-objcopy -O ihex target/thumbv7em-none-eabihf/release/peripheral dist/charybdis_left_peripheral.hex
arm-none-eabi-objcopy -O ihex target/thumbv7em-none-eabihf/release/dongle dist/charybdis_dongle.hex
arm-none-eabi-objcopy -O ihex target/thumbv7em-none-eabihf/release/settings_reset dist/charybdis_settings_reset.hex

# Convert to UF2 (family 0xADA52840 for nice!nano v2 Adafruit bootloader)
python3 uf2conv.py dist/charybdis_right_central.hex -c -f 0xADA52840 -o dist/charybdis_right_central.uf2
python3 uf2conv.py dist/charybdis_left_peripheral.hex -c -f 0xADA52840 -o dist/charybdis_left_peripheral.uf2
python3 uf2conv.py dist/charybdis_dongle.hex -c -f 0xADA52840 -o dist/charybdis_dongle.uf2
python3 uf2conv.py dist/charybdis_settings_reset.hex -c -f 0xADA52840 -o dist/charybdis_settings_reset.uf2

echo "=== Build Complete! UF2 files ready in dist/ ==="
ls -lh dist/*.uf2
