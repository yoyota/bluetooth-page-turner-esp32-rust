use esp_idf_hal::gpio::PinDriver;
use esp_idf_hal::peripherals::Peripherals;
use esp_idf_svc::log::EspLogger;
use esp_idf_sys as _;
use log::{debug, info, warn};
use std::time::Duration;

mod ble_hid;

use ble_hid::BleKeyboard;

const KEY_PAGE_DOWN: u8 = 0x4E;

const TOUCH_THRESHOLD: u16 = 400;

fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    EspLogger::initialize_default();

    info!("Starting BLE Page Turner Keyboard");

    let peripherals = Peripherals::take()?;
    let mut led = PinDriver::output(peripherals.pins.gpio2)?;

    unsafe {
        esp_idf_sys::touch_pad_init();
        esp_idf_sys::touch_pad_set_voltage(
            esp_idf_sys::touch_high_volt_t_TOUCH_HVOLT_2V7,
            esp_idf_sys::touch_low_volt_t_TOUCH_LVOLT_0V5,
            esp_idf_sys::touch_volt_atten_t_TOUCH_HVOLT_ATTEN_1V,
        );
        esp_idf_sys::touch_pad_config(
            esp_idf_sys::touch_pad_t_TOUCH_PAD_NUM0,
            0,
        );
    }
    info!("Touch sensor initialized on GPIO4");

    let keyboard = BleKeyboard::new("BLE PageTurner")?;

    info!("BLE Keyboard initialized. Waiting for connection...");

    let mut was_touched = false;
    let mut was_connected = false;

    loop {
        let connected = keyboard.is_connected();
        if !connected {
            was_connected = false;
            std::thread::sleep(Duration::from_millis(100));
            continue;
        }

        // Blink LED 5 times on new connection
        if !was_connected {
            info!("BLE connected, blinking LED");
            for _ in 0..5 {
                led.set_high().ok();
                std::thread::sleep(Duration::from_millis(200));
                led.set_low().ok();
                std::thread::sleep(Duration::from_millis(200));
            }
            was_connected = true;
        }

        // Touch sensor - next page
        let mut touch_val: u16 = 0;
        unsafe {
            esp_idf_sys::touch_pad_read(
                esp_idf_sys::touch_pad_t_TOUCH_PAD_NUM0,
                &mut touch_val,
            );
        }
        debug!("Touch  value! (val: {})", touch_val);

        let touched = touch_val < TOUCH_THRESHOLD;
        if touched && !was_touched {
            info!("Touch detected (val: {})", touch_val);
            if let Err(e) = keyboard.send_key(KEY_PAGE_DOWN) {
                warn!("Failed to send key: {:?}", e);
            }
            std::thread::sleep(Duration::from_millis(2000));
        }
        was_touched = touched;

        std::thread::sleep(Duration::from_millis(200));
    }
}
