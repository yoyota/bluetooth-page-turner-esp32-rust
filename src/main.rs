use esp_idf_hal::peripherals::Peripherals;
use esp_idf_svc::log::EspLogger;
use esp_idf_sys as _;
use log::{debug, info, warn};
use std::{thread, time::Duration};

mod ble_hid;

use ble_hid::BleKeyboard;

const KEY_PAGE_DOWN: u8 = 0x4E;
const TOUCH_THRESHOLD: u16 = 400;
const TOUCH_PAD: u32 = esp_idf_sys::touch_pad_t_TOUCH_PAD_NUM0;
const LED_GPIO: i32 = 2;

fn init_touch_sensor() {
    unsafe {
        esp_idf_sys::touch_pad_init();
        esp_idf_sys::touch_pad_set_voltage(
            esp_idf_sys::touch_high_volt_t_TOUCH_HVOLT_2V7,
            esp_idf_sys::touch_low_volt_t_TOUCH_LVOLT_0V5,
            esp_idf_sys::touch_volt_atten_t_TOUCH_HVOLT_ATTEN_1V,
        );
        esp_idf_sys::touch_pad_config(TOUCH_PAD, 0);
    }
    info!("Touch sensor initialized on GPIO4");
}

fn read_touch() -> u16 {
    let mut val: u16 = 0;
    unsafe {
        esp_idf_sys::touch_pad_read(TOUCH_PAD, &mut val);
    }
    val
}

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
    init_touch_sensor();

    let keyboard = BleKeyboard::new("BLE PageTurner", on_connect)?;
    info!("BLE Keyboard initialized. Waiting for connection...");

    let mut was_touched = false;

    loop {
        if !keyboard.is_connected() {
            thread::sleep(Duration::from_millis(100));
            continue;
        }

        let touch_val = read_touch();
        debug!("Touch value: {}", touch_val);

        let touched = touch_val < TOUCH_THRESHOLD;
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
