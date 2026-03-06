use esp_idf_hal::peripherals::Peripherals;
use esp_idf_svc::log::EspLogger;
use esp_idf_sys as _;
use log::{debug, info, warn};
use std::{thread, time::Duration};

mod ble_hid;
mod touch;

use ble_hid::BleKeyboard;

const KEY_PAGE_DOWN: u8 = 0x4E;
const LED_GPIO: i32 = 2;

fn init_led() {
    unsafe {
        esp_idf_sys::gpio_reset_pin(LED_GPIO);
        esp_idf_sys::gpio_set_direction(LED_GPIO, esp_idf_sys::gpio_mode_t_GPIO_MODE_OUTPUT);
    }
}

fn blink_led(count: u32) {
    for _ in 0..count {
        unsafe { esp_idf_sys::gpio_set_level(LED_GPIO, 1) };
        thread::sleep(Duration::from_millis(200));
        unsafe { esp_idf_sys::gpio_set_level(LED_GPIO, 0) };
        thread::sleep(Duration::from_millis(200));
    }
}

fn on_connect(_server: &mut esp32_nimble::BLEServer, desc: &esp32_nimble::BLEConnDesc) {
    info!("BLE client connected: {:?}", desc);
    blink_led(5);
}

fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    EspLogger::initialize_default();

    info!("Starting BLE Page Turner Keyboard");

    Peripherals::take()?;

    init_led();
    touch::init();
    info!("Touch sensor initialized on GPIO4");

    let keyboard = BleKeyboard::new("BLE PageTurner", on_connect)?;
    info!("BLE Keyboard initialized. Waiting for connection...");

    let mut was_touched = false;

    loop {
        if !keyboard.is_connected() {
            thread::sleep(Duration::from_millis(100));
            continue;
        }

        let touch_val = touch::read();
        debug!("Touch value: {}", touch_val);

        let touched = touch::is_touched(touch_val);
        if touched && !was_touched {
            info!("Touch detected (val: {})", touch_val);
            if let Err(e) = keyboard.send_key(KEY_PAGE_DOWN) {
                warn!("Failed to send key: {:?}", e);
            }
            thread::sleep(Duration::from_millis(2000));
        }
        was_touched = touched;

        thread::sleep(Duration::from_millis(200));
    }
}

#[cfg(test)]
mod tests {
    use crate::touch::{is_touched, TOUCH_THRESHOLD};

    #[test]
    fn touched_when_value_below_threshold() {
        assert!(is_touched(TOUCH_THRESHOLD - 1));
    }

    #[test]
    fn not_touched_at_threshold() {
        assert!(!is_touched(TOUCH_THRESHOLD));
    }

    #[test]
    fn not_touched_above_threshold() {
        assert!(!is_touched(TOUCH_THRESHOLD + 1));
    }

    #[test]
    fn not_touched_at_max_value() {
        assert!(!is_touched(u16::MAX));
    }

    #[test]
    fn touched_at_zero() {
        assert!(is_touched(0));
    }

    #[test]
    fn debounce_logic_fires_only_on_rising_edge() {
        // Key should only be sent once when transitioning from not-touched to touched.
        let readings: &[u16] = &[500, 500, 100, 100, 100, 500, 100];
        // expected: fire on index 2 and index 6 (rising edges)
        let expected_fires: &[bool] = &[false, false, true, false, false, false, true];

        let mut was_touched = false;
        for (i, &val) in readings.iter().enumerate() {
            let touched = is_touched(val);
            let fires = touched && !was_touched;
            assert_eq!(fires, expected_fires[i], "mismatch at reading index {i}");
            was_touched = touched;
        }
    }
}
