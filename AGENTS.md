# AGENTS.md — Charybdis Mini RMK Firmware Project Context

> **Project**: Production-grade wireless split keyboard firmware built on [RMK](https://github.com/haobogu/rmk) (Rust) for **Charybdis Mini 3×nice!nano v2** with PMW3610 trackball and dual-dongle bonding support.  
> **Source of Truth**: Migrated 1:1 from ZMK repository [`jools333/charybdis-3-6-dongle-prospector-studio`](https://github.com/jools333/charybdis-3-6-dongle-prospector-studio).  
> **Git Remote**: `git@github.com:jools333/rmk.git` (`main` branch).  
> **Last Updated**: 2026-09-25

---

## 1. Project Overview & Capabilities

This repository contains pure Rust firmware replacing ZMK on a 4-node wireless split setup:
1. **Right Half (Central)**: Primary split controller running split BLE central (connecting to Left half) + BLE peripheral link (connecting to active USB Dongle or Bluetooth hosts) + PMW3610 optical trackball bit-bang SPI driver.
2. **Left Half (Peripheral)**: Split matrix scanner sending switch reports over BLE to Right half.
3. **USB Dongle 1 (Home)**: High-speed 2M PHY BLE receiver relaying keyboard and mouse events to PC over USB HID.
4. **USB Dongle 2 (Work)**: Identical hardware and firmware binary as Dongle 1, coexisting via multi-dongle bond slots.

### Key Features Delivered
- **100% Offline, Self-Contained Repository**: Embedded patched RMK workspace (`rmk-upstream/`) allows instant building on any Linux/Ubuntu or Windows system without external git submodules or network dependencies.
- **Dual-Dongle Support (`dongle_profiles = 2`)**: Zero hardcoded MAC addresses. The same `charybdis_dongle.uf2` binary runs on both Home and Work dongles. Dongles are bonded into separate non-volatile flash slots (Home = Slot 0, Work = Slot 1).
- **Auto-Rotation & Hotkey Switching**: Keyboard searches for the active dongle with a 4-second timeout, automatically rotating between bonded dongles. Manual switching via `User8` / `User9` keys on Layer 8.
- **Trackball Integration**: PMW3610 optical sensor with 800 CPI, `swap_xy = true`, `invert_x = true`, `invert_y = true`, auto-mouse layer activation (Layer 1 on motion with 1000ms idle timeout), Sniper mode (Layer 6, 1/6 speed), and Scroll mode (Layer 7, 1/3 speed with inverted Y).
- **Exact Layout & Behaviors**: 9 layers, 14 combos, 2 macros (`->`, `=>`), home-row mods, USB HID boot protocol support for BIOS/UEFI.

---

## 2. Repository Structure

```
charybdis-rmk/
├── .cargo/
│   └── config.toml             # Target thumbv7em-none-eabihf, rustflags (-C link-arg=-Tlink.x)
├── dist/                       # Ready-to-flash release binaries
│   ├── charybdis_right_central.uf2      # Right half (Central)
│   ├── charybdis_right_central.hex
│   ├── charybdis_left_peripheral.uf2    # Left half (Peripheral)
│   ├── charybdis_left_peripheral.hex
│   ├── charybdis_dongle.uf2             # USB Dongle (Home AND Work)
│   ├── charybdis_dongle.hex
│   ├── charybdis_settings_reset.uf2     # Flash storage wiper utility
│   └── charybdis_settings_reset.hex
├── rmk-upstream/               # Embedded patched RMK workspace crates
│   ├── rmk/                    # Core keyboard firmware engine
│   ├── rmk-config/             # Configuration parsers and constants
│   ├── rmk-macro/              # Procedural macros
│   └── rmk-types/              # Common types and build constants
├── src/
│   ├── central.rs              # Right Half main binary
│   ├── peripheral.rs           # Left Half main binary
│   ├── dongle.rs               # USB Dongle main binary
│   ├── settings_reset.rs       # Flash wiper binary
│   ├── keymap.rs               # 9 layers, actions, user keycodes
│   ├── macros.rs               # Text macros (arrow, fat_arrow)
│   └── pointing_processor_controller.rs # Trackball layer hooks (sniper, scroll)
├── build.rs                    # Linker script search path setup
├── build.sh                    # Automated Ubuntu/Linux release build + UF2 converter
├── build.ps1                   # Windows helper triggering build.sh via WSL
├── Cargo.lock                  # Pinned dependency graph
├── Cargo.toml                  # Package manifest (points rmk to rmk-upstream/rmk)
├── keyboard.toml               # RMK layout, combos, morse, and constants
├── memory.x                    # Flash memory layout (Adafruit bootloader 0x1000)
├── multi_dongle_support.patch  # Standalone patch file for RMK multi-dongle feature
├── uf2conv.py                  # Intel HEX to UF2 converter
└── uf2families.json            # Family definitions (0xADA52840 for nice!nano v2)
```

---

## 3. Hardware Configuration & Pinout

### Microcontroller: 3× nice!nano v2 (Nordic nRF52840)
- **Clock**: 32.768 kHz RC oscillator (`rc_ctiv = 16`, `rc_temp_ctiv = 2`, `accuracy_ppm = 500`).
- **Internal DCDC**: Enabled (`reg0_voltage = 3V3`, `reg0 = true`, `reg1 = true`).
- **Flash Start**: `0x1000` (reserved 4 KB for Adafruit nRF52 bootloader).
- **Storage Offset**: `0xA0000`, 6 sectors (24 KB) for non-volatile keymap and bond storage.
- **Battery Monitoring**: SAADC pin `P0.04` (AIN2) with 2M / 806k voltage divider.

### Matrix Specifications
- **Matrix Dimension**: 4 rows × 12 columns
  - Left Half: Rows 0..3, Columns 0..5 (`ROW_OFFSET = 0`, `COL_OFFSET = 0`)
  - Right Half: Rows 0..3, Columns 6..11 (`ROW_OFFSET = 0`, `COL_OFFSET = 6`)
- **Diode Direction**: `row2col` (RMK setting: `COL2ROW = false`). Rows are outputs; columns are inputs with pull-down resistors.
- **Pin Mapping (Both Halves)**:
  - Rows (Outputs): `P1.15`, `P0.24`, `P0.22`, `P1.06` (Pro Micro pins 18, 5, 4, 9)
  - Columns (Inputs): `P0.02`, `P0.29`, `P0.09`, `P1.00`, `P0.11`, `P1.04` (Pro Micro pins 19, 20, 10, 6, 7, 8)

### Trackball (PMW3610 on Right Half)
- **Protocol**: 3-wire bit-bang SPI (`BitBangSpiBus`).
- **Pins**:
  - SCK: `P0.08`
  - SDIO: `P0.17`
  - CS: `P0.20`
  - MOTION: `P0.06` (Input with pull-up)
- **Transform**:
  - Base CPI: 800 CPI
  - Coordinates: `swap_xy = true`, `invert_x = true`, `invert_y = true`
- **Dynamic Layer Modes**:
  - Auto-Mouse (Layer 1): Activated on trackball motion with 1000ms idle timeout.
  - Sniper Mode (Layer 6): CPI divided by 6 for pixel-precise aiming.
  - Scroll Mode (Layer 7): CPI divided by 3, Y-axis inverted for natural wheel emulation.

---

## 4. Multi-Dongle Architecture & Implementation

### The Problem in Stock RMK / ZMK
Stock RMK supports only 1 hardcoded or single-slot dongle profile (`DONGLE_PROFILE = NUM_BLE_PROFILE`). Pairing a second dongle overwrites the bond of the first dongle. ZMK split dongles typically require hardcoding peripheral MAC addresses or recompiling separate binaries.

### The Solution: Multi-Dongle Bonding System
We enhanced RMK upstream (`feature/multi-dongle-bonds`, embedded in `rmk-upstream/`):
1. **Configurable Profile Count**: Added `dongle_profiles = 2` to `keyboard.toml` and `RmkConstantsConfig`.
2. **Dedicated Bond Storage**:
   - `BOND_SLOTS = NUM_BLE_PROFILE + NUM_DONGLE_PROFILES` (3 host profiles + 2 dongle slots = 5 total bond slots).
   - Dongle Slot 0: Profile 3 (Home Dongle).
   - Dongle Slot 1: Profile 4 (Work Dongle).
3. **Smart Pairing (`Adv::DongleSeeking`)**:
   - Unbonded keyboard broadcasts `Adv::DongleSeeking`.
   - Dongle continuously scans; upon detecting `Adv::DongleSeeking`, it pairs and stores the bond.
   - Once bonded, keyboard communicates via high-speed directed advertisements (`Adv::Directed`).
4. **Auto-Rotation**:
   - If the active dongle is offline (e.g. traveling between Home and Work), the keyboard directs advertisements for 4 seconds, then automatically switches to the next bonded dongle slot.
5. **Manual Layer 8 Controls**:
   - `user!(8)`: Switch to Dongle Slot 0 (Home). Hold to clear slot and re-pair.
   - `user!(9)`: Switch to Dongle Slot 1 (Work). Hold to clear slot and re-pair.
   - `user!(0)`, `user!(1)`, `user!(2)`: Switch to standard Bluetooth host profiles.
   - `kbctrl!(Bootloader)`: Enter bootloader without touching hardware reset buttons.

---

## 5. Keymap & Layer Reference

- **Layer 0 (Base)**: QWERTY + Home-row mod-taps (`mt!`, `gqt!`), layer-taps on thumbs and letters (`lt!(1, E)`, `lt!(6, R)`, `lt!(7, W)`, `lt!(2, Delete)`, `lt!(3, Backspace)`).
- **Layer 1 (Mouse)**: Mouse buttons (MB1, MB2, MB3) on both thumbs and home row, momentary access to Sniper (`mo!(6)`) and Scroll (`mo!(7)`), toggle to Layer 0 (`to!(0)`).
- **Layer 2 (Symbol)**: Shifted symbols (`!@#$%^&*()_+`), braces, brackets, pipe, tilde, and layer toggles (`tg!(4)` Fun, `tg!(5)` Game, `tg!(3)` Num).
- **Layer 3 (Number)**: Numeric row, navigation cluster (Home, End, Up, Down, Left, Right), function keys F1..F12, search shortcuts.
- **Layer 4 (Function)**: Complete F1..F12 matrix, Ctrl+Alt+Del combo, modifier cluster.
- **Layer 5 (Button / Game)**: WASD gaming layout with direct space/alt thumb bindings and 1..5 numbers.
- **Layer 6 (MouseSnip)**: Sniper mode with 1/6 tracking speed divisor.
- **Layer 7 (MouseScroll)**: Scroll wheel simulation with 1/3 speed divisor and inverted Y.
- **Layer 8 (Reset)**: Bootloader access, Dongle 0/1 profile selection, BLE host profiles 0..2.

### Combos (14 Total)
- `Q + P` -> `Win+Shift+P` (VPN Toggle)
- `LGui + '` -> `Win+S` (Web Search)
- `F + J` -> `Win+Space` (Language Switch)
- `Z + /` -> `Win+Shift+Q` (Close Window)
- `V + M` -> `Win+Z` (Editor Action)
- `F + ;` -> `Ctrl+S` (Save)
- `R + U` -> `Win+A` (Messenger)
- `E + I` -> `Win+X` (Command Palette)
- `A + ;` -> `Ctrl+P` (Quick Open)
- `W + O` -> Macro 0 (`->`)
- `C + ,` -> `Alt+Tab` (Window Switch)
- `X + .` -> Macro 1 (`=>`)
- `X + /` -> `Win+Shift+Y` (VPN 2 Toggle)
- `Ctrl+F12` on Num Layer -> Search

---

## 6. Development & Build Workflow

### Prerequisites on Ubuntu / Debian
```bash
sudo apt update
sudo apt install -y gcc-arm-none-eabi libnewlib-arm-none-eabi libclang-dev python3 git
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
rustup target add thumbv7em-none-eabihf
```

### Building Firmware
From repository root (`charybdis-rmk`):
```bash
./build.sh
```
The script automatically builds release binaries for all targets, converts them via `arm-none-eabi-objcopy` and `uf2conv.py`, and outputs ready-to-flash files into `dist/`.

### Flashing Devices
1. Double-click the reset button on the nice!nano v2 (or bridge `RST` + `GND` twice).
2. The controller mounts as a USB mass-storage drive named `NICENANO` (e.g. `/media/$USER/NICENANO/`).
3. Copy the appropriate file:
   - **Right Half**: `cp dist/charybdis_right_central.uf2 /media/$USER/NICENANO/`
   - **Left Half**: `cp dist/charybdis_left_peripheral.uf2 /media/$USER/NICENANO/`
   - **Dongle 1 (Home)**: `cp dist/charybdis_dongle.uf2 /media/$USER/NICENANO/`
   - **Dongle 2 (Work)**: `cp dist/charybdis_dongle.uf2 /media/$USER/NICENANO/` (same binary)
   - **Memory Wipe**: `cp dist/charybdis_settings_reset.uf2 /media/$USER/NICENANO/`
4. The drive automatically unmounts upon write completion and reboots into the new firmware.

---

## 7. Pair Programming Guidelines for Future AI Agents

- **Source Integrity**: `Cargo.toml` points to `rmk-upstream/rmk`. Do not revert to external paths (`../../rmk-upstream`) as that breaks portability on fresh user machines.
- **Unified Dongle Binary**: Do not split `dongle.rs` into home/work binaries. The multi-dongle system is designed to use an identical binary for both dongles.
- **Trackball SPI**: The PMW3610 requires bit-bang 3-wire SPI (`BitBangSpiBus`) with bidirectional SDIO handling. Do not replace with standard hardware SPI unless hardware level-shifter pinout is modified.
- **Memory Map**: Flash start is strictly `0x1000` because the nice!nano v2 bootloader occupies `0x0000..0x1000`. Storage sector base is `0xA0000` with 6 sectors.
- **Commit History**: When making changes, commit cleanly and push to `origin main` using the configured SSH identity.
