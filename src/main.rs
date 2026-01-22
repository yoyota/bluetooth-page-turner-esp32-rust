use esp_idf_hal::gpio::{PinDriver, Pull};
use esp_idf_hal::peripherals::Peripherals;
use esp_idf_svc::log::EspLogger;
use esp_idf_sys as _;
use log::{info, warn};
use std::time::Duration;

mod ble_hid;

use ble_hid::BleKeyboard;

// HID Keyboard scan codes for page turning
const KEY_PAGE_UP: u8 = 0x4B;
const KEY_PAGE_DOWN: u8 = 0x4E;
#[allow(dead_code)]
const KEY_LEFT_ARROW: u8 = 0x50;
#[allow(dead_code)]
const KEY_RIGHT_ARROW: u8 = 0x4F;

fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    EspLogger::initialize_default();

    info!("Starting BLE Page Turner Keyboard");

    let peripherals = Peripherals::take()?;

    // Configure GPIO buttons for page navigation
    // GPIO0 - Previous page (built-in BOOT button on most ESP32 boards)
    // GPIO32 - Next page (supports internal pull-up, unlike GPIO35 which is input-only)
    let btn_prev = PinDriver::input(peripherals.pins.gpio0)?;
    let mut btn_next = PinDriver::input(peripherals.pins.gpio32)?;
    btn_next.set_pull(Pull::Up)?;

    // Initialize BLE HID keyboard
    let mut keyboard = BleKeyboard::new("BLE PageTurner")?;
    keyboard.start()?;

    info!("BLE Keyboard initialized. Waiting for connection...");

    let mut prev_pressed = false;
    let mut next_pressed = false;

    loop {
        // Check connection status
        if !keyboard.is_connected() {
            std::thread::sleep(Duration::from_millis(100));
            continue;
        }

        // Previous page button (active low)
        let prev_state = btn_prev.is_low();
        if prev_state && !prev_pressed {
            info!("Previous page pressed");
            if let Err(e) = keyboard.send_key(KEY_PAGE_UP) {
                warn!("Failed to send key: {:?}", e);
            }
        }
        prev_pressed = prev_state;

        // Next page button (active low)
        let next_state = btn_next.is_low();
        if next_state && !next_pressed {
            info!("Next page pressed");
            if let Err(e) = keyboard.send_key(KEY_PAGE_DOWN) {
                warn!("Failed to send key: {:?}", e);
            }
        }
        next_pressed = next_state;
        // if let Err(e) = keyboard.send_key(KEY_PAGE_DOWN) {
        //     warn!("Failed to send key: {:?}", e);
        // }

        // std::thread::sleep(Duration::from_secs(5));

        // if let Err(e) = keyboard.send_key(KEY_PAGE_UP) {
        //     warn!("Failed to send key: {:?}", e);
        // }

        std::thread::sleep(Duration::from_millis(20));
    }
}
