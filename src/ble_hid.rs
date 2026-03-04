use esp32_nimble::{
    enums::*, hid::*, utilities::mutex::Mutex, BLEAdvertisementData, BLECharacteristic,
    BLEDevice, BLEHIDDevice, BLEServer,
};
use log::info;
use std::{sync::Arc, thread, time::Duration};

const KEYBOARD_ID: u8 = 0x01;

const HID_REPORT_DESCRIPTOR: &[u8] = hid!(
    (USAGE_PAGE, 0x01),       // Generic Desktop
    (USAGE, 0x06),            // Keyboard
    (COLLECTION, 0x01),       // Application
    (REPORT_ID, KEYBOARD_ID),
    // Modifier keys (8 bits)
    (USAGE_PAGE, 0x07),       // Keyboard/Keypad
    (USAGE_MINIMUM, 0xE0),
    (USAGE_MAXIMUM, 0xE7),
    (LOGICAL_MINIMUM, 0x00),
    (LOGICAL_MAXIMUM, 0x01),
    (REPORT_SIZE, 0x01),
    (REPORT_COUNT, 0x08),
    (HIDINPUT, 0x02),
    // Reserved byte
    (REPORT_COUNT, 0x01),
    (REPORT_SIZE, 0x08),
    (HIDINPUT, 0x01),
    // Key array (6 keys)
    (REPORT_COUNT, 0x06),
    (REPORT_SIZE, 0x08),
    (LOGICAL_MINIMUM, 0x00),
    (LOGICAL_MAXIMUM, 0x65),
    (USAGE_PAGE, 0x07),
    (USAGE_MINIMUM, 0x00),
    (USAGE_MAXIMUM, 0x65),
    (HIDINPUT, 0x00),
    (END_COLLECTION),
);

pub struct BleKeyboard {
    server: &'static mut BLEServer,
    input: Arc<Mutex<BLECharacteristic>>,
}

impl BleKeyboard {
    pub fn new(name: &str, on_connect: impl FnMut(&mut BLEServer, &esp32_nimble::BLEConnDesc) + Send + Sync + 'static) -> anyhow::Result<Self> {
        let device = BLEDevice::take();

        device
            .security()
            .set_auth(AuthReq::Bond)
            .set_io_cap(SecurityIOCap::NoInputNoOutput)
            .resolve_rpa();

        let server = device.get_server();
        server.on_connect(on_connect);
        let mut hid = BLEHIDDevice::new(server);

        let input = hid.input_report(KEYBOARD_ID);

        hid.manufacturer("Espressif");
        hid.pnp(0x02, 0x05ac, 0x820a, 0x0210);
        hid.hid_info(0x00, 0x01);
        hid.report_map(HID_REPORT_DESCRIPTOR);
        hid.set_battery_level(100);

        let ble_advertising = device.get_advertising();
        ble_advertising.lock().scan_response(false).set_data(
            BLEAdvertisementData::new()
                .name(name)
                .appearance(0x03C1)
                .add_service_uuid(hid.hid_service().lock().uuid()),
        )?;
        ble_advertising.lock().start()?;

        info!("BLE HID Keyboard initialized: {}", name);

        Ok(Self {
            server,
            input,
        })
    }

    pub fn is_connected(&self) -> bool {
        self.server.connected_count() > 0
    }

    pub fn send_key(&self, keycode: u8) -> anyhow::Result<()> {
        if !self.is_connected() {
            return Err(anyhow::anyhow!("Not connected"));
        }

        // Key press: [modifier, reserved, key1..key6]
        let report = [0u8, 0, keycode, 0, 0, 0, 0, 0];
        self.input.lock().set_value(&report).notify();

        thread::sleep(Duration::from_millis(20));

        // Key release
        let release = [0u8; 8];
        self.input.lock().set_value(&release).notify();

        Ok(())
    }
}
