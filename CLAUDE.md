# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a Rust-based ESP32 BLE HID Keyboard firmware project that creates a wireless Bluetooth page-turning remote control. The device advertises as "BLE PageTurner" and sends page navigation keycodes (PAGE_UP, PAGE_DOWN, arrow keys) to connected devices.

## Build Commands

```bash
# Build (requires ESP Rust toolchain via espup)
cargo build --release

# Flash to device and monitor serial output
cargo run --release

# Clean build artifacts
cargo clean
```

**Prerequisites:** ESP Rust toolchain (install via `espup`), espflash tool for flashing. ESP-IDF v5.2.2 is automatically downloaded by embuild during first build.

## Architecture

**Target:** `xtensa-esp32-espidf` (ESP32 with ESP-IDF framework)

**Two main modules:**

- `src/main.rs` - Application entry point with GPIO button polling loop. GPIO0 (BOOT button) triggers "previous page", GPIO35 triggers "next page". Uses 20ms debounce interval.

- `src/ble_hid.rs` - BLE HID keyboard implementation using Bluedroid stack. Manages BLE advertising, connection state (via atomic bool), and sends standard 8-byte USB HID keyboard reports.

**Key HID keycodes used:** 0x4B (PAGE_UP), 0x4E (PAGE_DOWN), 0x50 (LEFT_ARROW), 0x4F (RIGHT_ARROW)

## Configuration Files

- `sdkconfig.defaults` - ESP-IDF SDK settings: enables Bluetooth/BLE/HID, sets device name, configures stack sizes
- `.cargo/config.toml` - Cargo settings: target architecture, espflash runner, ESP-IDF version (v5.2.2)
- `rust-toolchain.toml` - Uses "esp" channel for Xtensa architecture support
