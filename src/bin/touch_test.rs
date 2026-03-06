use esp_idf_hal::peripherals::Peripherals;
use esp_idf_svc::log::EspLogger;
use esp_idf_sys as _;
use log::info;
use std::{thread, time::Duration};

#[path = "../touch.rs"]
mod touch;

fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    EspLogger::initialize_default();

    Peripherals::take()?;
    touch::init();

    info!("=== TOUCH SENSOR TEST (threshold: {}) ===", touch::TOUCH_THRESHOLD);

    loop {
        let val = touch::read();
        let state = if touch::is_touched(val) { "TOUCHED" } else { "open" };
        info!("touch_val={:4}  {}", val, state);
        thread::sleep(Duration::from_millis(200));
    }
}
