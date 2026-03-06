pub const TOUCH_THRESHOLD: u16 = 1000;
pub const TOUCH_PAD: u32 = esp_idf_sys::touch_pad_t_TOUCH_PAD_NUM0;

pub fn init() {
    unsafe {
        esp_idf_sys::touch_pad_init();
        esp_idf_sys::touch_pad_set_voltage(
            esp_idf_sys::touch_high_volt_t_TOUCH_HVOLT_2V7,
            esp_idf_sys::touch_low_volt_t_TOUCH_LVOLT_0V5,
            esp_idf_sys::touch_volt_atten_t_TOUCH_HVOLT_ATTEN_1V,
        );
        esp_idf_sys::touch_pad_config(TOUCH_PAD, 0);
    }
}

pub fn read() -> u16 {
    let mut val: u16 = 0;
    unsafe {
        esp_idf_sys::touch_pad_read(TOUCH_PAD, &mut val);
    }
    val
}

pub fn is_touched(val: u16) -> bool {
    val < TOUCH_THRESHOLD
}
