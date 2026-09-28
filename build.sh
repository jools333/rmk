#!/usr/bin/env bash
set -e

# Load cargo environment if available
[ -f "$HOME/.cargo/env" ] && source "$HOME/.cargo/env"

echo "=========================================================="
echo "  Building Charybdis Mini RMK Firmware (Ubuntu / Linux)   "
echo "=========================================================="

# Check for Rust / Cargo
if ! command -v cargo &> /dev/null; then
    echo "[-] Error: 'cargo' not found in PATH."
    echo "    Install Rust with: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

# Ensure target thumbv7em-none-eabihf is installed
if ! rustup target list --installed | grep -q "thumbv7em-none-eabihf"; then
    echo "[*] Adding Rust target 'thumbv7em-none-eabihf'..."
    rustup target add thumbv7em-none-eabihf
fi

# Check for arm-none-eabi-objcopy
if ! command -v arm-none-eabi-objcopy &> /dev/null; then
    echo "[-] Error: 'arm-none-eabi-objcopy' not found."
    echo "    On Ubuntu/Debian, install with:"
    echo "    sudo apt update && sudo apt install -y gcc-arm-none-eabi libnewlib-arm-none-eabi libclang-dev"
    exit 1
fi

# Check for Python 3
if ! command -v python3 &> /dev/null; then
    echo "[-] Error: 'python3' not found."
    echo "    On Ubuntu/Debian, install with:"
    echo "    sudo apt install -y python3"
    exit 1
fi

echo "[1/3] Compiling firmware release binaries..."
cargo build --release

echo "[2/3] Converting ELF binaries to HEX..."
mkdir -p dist
arm-none-eabi-objcopy -O ihex target/thumbv7em-none-eabihf/release/central dist/charybdis_right_central.hex
arm-none-eabi-objcopy -O ihex target/thumbv7em-none-eabihf/release/peripheral dist/charybdis_left_peripheral.hex
arm-none-eabi-objcopy -O ihex target/thumbv7em-none-eabihf/release/dongle dist/charybdis_dongle.hex
arm-none-eabi-objcopy -O ihex target/thumbv7em-none-eabihf/release/settings_reset dist/charybdis_settings_reset.hex

echo "[3/3] Generating UF2 files for nice!nano v2 (Family: 0xADA52840)..."
python3 uf2conv.py dist/charybdis_right_central.hex -c -f 0xADA52840 -o dist/charybdis_right_central.uf2
python3 uf2conv.py dist/charybdis_left_peripheral.hex -c -f 0xADA52840 -o dist/charybdis_left_peripheral.uf2
python3 uf2conv.py dist/charybdis_dongle.hex -c -f 0xADA52840 -o dist/charybdis_dongle.uf2
python3 uf2conv.py dist/charybdis_settings_reset.hex -c -f 0xADA52840 -o dist/charybdis_settings_reset.uf2

echo "=========================================================="
echo "  Build successful! Firmware files are ready in dist/:"
echo "=========================================================="
ls -lh dist/*.uf2
echo "----------------------------------------------------------"
echo "Flash targets:"
echo "  1. Right half (Central):   dist/charybdis_right_central.uf2"
echo "  2. Left half (Peripheral): dist/charybdis_left_peripheral.uf2"
echo "  3. USB Dongle 1 (Home):    dist/charybdis_dongle.uf2"
echo "  4. USB Dongle 2 (Work):    dist/charybdis_dongle.uf2 (same binary)"
echo "=========================================================="
