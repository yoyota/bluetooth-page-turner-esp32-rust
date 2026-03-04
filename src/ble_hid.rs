use esp_idf_sys::*;
use log::info;
use std::ffi::CString;
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU8, Ordering};

static CONNECTED: AtomicBool = AtomicBool::new(false);
static CONN_ID: AtomicU16 = AtomicU16::new(0);
static GATTS_IF: AtomicU8 = AtomicU8::new(ESP_GATT_IF_NONE as u8);
static HID_REPORT_HANDLE: AtomicU16 = AtomicU16::new(0);
static HID_REPORT_MAP_HANDLE: AtomicU16 = AtomicU16::new(0);
static HID_INFO_HANDLE: AtomicU16 = AtomicU16::new(0);

// HID Service UUID: 0x1812
const HID_SERVICE_UUID: u16 = 0x1812;
// HID Report Characteristic UUID: 0x2A4D
const HID_REPORT_CHAR_UUID: u16 = 0x2A4D;
// HID Report Map Characteristic UUID: 0x2A4B
const HID_REPORT_MAP_UUID: u16 = 0x2A4B;
// HID Information Characteristic UUID: 0x2A4A
const HID_INFO_UUID: u16 = 0x2A4A;
// HID Control Point Characteristic UUID: 0x2A4C
const HID_CONTROL_POINT_UUID: u16 = 0x2A4C;
// Client Characteristic Configuration Descriptor UUID: 0x2902
const CCC_DESCRIPTOR_UUID: u16 = 0x2902;

// HID Report Descriptor for a keyboard
const HID_REPORT_MAP: &[u8] = &[
    0x05, 0x01, // Usage Page (Generic Desktop)
    0x09, 0x06, // Usage (Keyboard)
    0xA1, 0x01, // Collection (Application)
    0x85, 0x01, //   Report ID (1)
    0x05, 0x07, //   Usage Page (Keyboard/Keypad)
    0x19, 0xE0, //   Usage Minimum (Left Control)
    0x29, 0xE7, //   Usage Maximum (Right GUI)
    0x15, 0x00, //   Logical Minimum (0)
    0x25, 0x01, //   Logical Maximum (1)
    0x75, 0x01, //   Report Size (1)
    0x95, 0x08, //   Report Count (8)
    0x81, 0x02, //   Input (Data, Variable, Absolute) - Modifier byte
    0x95, 0x01, //   Report Count (1)
    0x75, 0x08, //   Report Size (8)
    0x81, 0x01, //   Input (Constant) - Reserved byte
    0x95, 0x06, //   Report Count (6)
    0x75, 0x08, //   Report Size (8)
    0x15, 0x00, //   Logical Minimum (0)
    0x25, 0x65, //   Logical Maximum (101)
    0x05, 0x07, //   Usage Page (Keyboard/Keypad)
    0x19, 0x00, //   Usage Minimum (0)
    0x29, 0x65, //   Usage Maximum (101)
    0x81, 0x00, //   Input (Data, Array) - Key array (6 keys)
    0xC0, // End Collection
];

// HID Information: bcdHID (1.11), bCountryCode (0), Flags (RemoteWake, NormallyConnectable)
const HID_INFO: &[u8] = &[0x11, 0x01, 0x00, 0x02];

// Application ID for GATT server
const HID_APP_ID: u16 = 0x1234;

pub struct BleKeyboard {
    device_name: CString,
}

impl BleKeyboard {
    pub fn new(name: &str) -> anyhow::Result<Self> {
        let device_name = CString::new(name)?;
        Ok(Self { device_name })
    }

    pub fn start(&mut self) -> anyhow::Result<()> {
        unsafe {
            // Initialize NVS
            let ret = nvs_flash_init();
            if ret == ESP_ERR_NVS_NO_FREE_PAGES as i32
                || ret == ESP_ERR_NVS_NEW_VERSION_FOUND as i32
            {
                esp_err_t_to_result(nvs_flash_erase())?;
                esp_err_t_to_result(nvs_flash_init())?;
            }

            // Initialize Bluetooth controller with proper default config
            // These values match BT_CONTROLLER_INIT_CONFIG_DEFAULT() macro for ESP32 (ESP-IDF v5.2.2)
            let mut bt_cfg = esp_bt_controller_config_t {
                controller_task_stack_size: 4096,
                controller_task_prio: 23,
                hci_uart_no: 1,
                hci_uart_baudrate: 921600,
                scan_duplicate_mode: 0,
                scan_duplicate_type: 0,
                normal_adv_size: 20,
                mesh_adv_size: 0,
                send_adv_reserved_size: 1000, // SCAN_SEND_ADV_RESERVED_SIZE
                controller_debug_flag: 0,
                mode: esp_bt_mode_t_ESP_BT_MODE_BLE as u8,
                ble_max_conn: 3,
                bt_max_acl_conn: 0,
                bt_sco_datapath: 0,
                auto_latency: false,
                bt_legacy_auth_vs_evt: false,
                bt_max_sync_conn: 0,
                ble_sca: 0, // ESP_BLE_SCA_500PPM
                pcm_role: 0,
                pcm_polar: 0,
                hli: true, // High Level Interrupt enabled
                dup_list_refresh_period: 0,
                ble_scan_backoff: false,
                magic: ESP_BT_CONTROLLER_CONFIG_MAGIC_VAL,
            };

            esp_err_t_to_result(esp_bt_controller_init(&mut bt_cfg))?;
            esp_err_t_to_result(esp_bt_controller_enable(
                esp_bt_mode_t_ESP_BT_MODE_BLE,
            ))?;
            esp_err_t_to_result(esp_bluedroid_init())?;
            esp_err_t_to_result(esp_bluedroid_enable())?;

            // Clear all bonded devices on startup
            let bond_num = esp_ble_get_bond_device_num();
            if bond_num > 0 {
                info!("Found {} bonded devices, removing...", bond_num);
                let mut dev_list: Vec<esp_ble_bond_dev_t> =
                    vec![Default::default(); bond_num as usize];
                let mut actual_num = bond_num;
                let ret = esp_ble_get_bond_device_list(
                    &mut actual_num,
                    dev_list.as_mut_ptr(),
                );
                if ret == ESP_OK as i32 {
                    for i in 0..actual_num as usize {
                        esp_ble_remove_bond_device(
                            dev_list[i].bd_addr.as_mut_ptr(),
                        );
                    }
                    info!("Cleared {} bonded devices", actual_num);
                }
            }

            // Register callbacks
            esp_err_t_to_result(esp_ble_gatts_register_callback(Some(
                gatts_event_handler,
            )))?;
            esp_err_t_to_result(esp_ble_gap_register_callback(Some(
                gap_event_handler,
            )))?;

            // Register GATT application
            esp_err_t_to_result(esp_ble_gatts_app_register(HID_APP_ID))?;

            // Set device name
            esp_err_t_to_result(esp_ble_gap_set_device_name(
                self.device_name.as_ptr(),
            ))?;

            // Configure security
            // ESP_LE_AUTH_BOND = 0x01
            let auth_req: u8 = 0x01;
            // ESP_IO_CAP_NONE = 3
            let iocap: u8 = 3;
            let init_key: u8 =
                ESP_BLE_ENC_KEY_MASK as u8 | ESP_BLE_ID_KEY_MASK as u8;
            let rsp_key: u8 =
                ESP_BLE_ENC_KEY_MASK as u8 | ESP_BLE_ID_KEY_MASK as u8;
            let key_size: u8 = 16;

            esp_ble_gap_set_security_param(
                esp_ble_sm_param_t_ESP_BLE_SM_AUTHEN_REQ_MODE,
                &auth_req as *const _ as *mut _,
                1,
            );
            esp_ble_gap_set_security_param(
                esp_ble_sm_param_t_ESP_BLE_SM_IOCAP_MODE,
                &iocap as *const _ as *mut _,
                1,
            );
            esp_ble_gap_set_security_param(
                esp_ble_sm_param_t_ESP_BLE_SM_SET_INIT_KEY,
                &init_key as *const _ as *mut _,
                1,
            );
            esp_ble_gap_set_security_param(
                esp_ble_sm_param_t_ESP_BLE_SM_SET_RSP_KEY,
                &rsp_key as *const _ as *mut _,
                1,
            );
            esp_ble_gap_set_security_param(
                esp_ble_sm_param_t_ESP_BLE_SM_MAX_KEY_SIZE,
                &key_size as *const _ as *mut _,
                1,
            );

            info!("BLE HID Keyboard initialized");
        }
        Ok(())
    }

    pub fn is_connected(&self) -> bool {
        CONNECTED.load(Ordering::Relaxed)
    }

    pub fn send_key(&mut self, keycode: u8) -> anyhow::Result<()> {
        if !self.is_connected() {
            return Err(anyhow::anyhow!("Not connected"));
        }

        let gatts_if = GATTS_IF.load(Ordering::Relaxed);
        let conn_id = CONN_ID.load(Ordering::Relaxed);
        let handle = HID_REPORT_HANDLE.load(Ordering::Relaxed);

        if handle == 0 {
            return Err(anyhow::anyhow!("HID report handle not set"));
        }

        // HID keyboard report: [report_id, modifier, reserved, key1, key2, key3, key4, key5, key6]
        let mut report = [0u8; 9];
        report[0] = 0x01; // Report ID
        report[3] = keycode; // First key slot

        unsafe {
            // Send key press notification
            esp_ble_gatts_send_indicate(
                gatts_if,
                conn_id,
                handle,
                report.len() as u16,
                report.as_ptr() as *mut u8,
                false, // notification, not indication
            );

            // Small delay
            std::thread::sleep(std::time::Duration::from_millis(20));

            // Send key release (all zeros except report ID)
            let mut release_report = [0u8; 9];
            release_report[0] = 0x01; // Report ID
            esp_ble_gatts_send_indicate(
                gatts_if,
                conn_id,
                handle,
                release_report.len() as u16,
                release_report.as_ptr() as *mut u8,
                false,
            );
        }

        Ok(())
    }
}

fn esp_err_t_to_result(err: esp_err_t) -> anyhow::Result<()> {
    if err == ESP_OK as i32 {
        Ok(())
    } else {
        Err(anyhow::anyhow!("ESP error: {}", err))
    }
}

fn start_advertising() {
    unsafe {
        // Set advertising data
        let mut adv_data = esp_ble_adv_data_t {
            set_scan_rsp: false,
            include_name: true,
            include_txpower: true,
            min_interval: 0x0006,
            max_interval: 0x0010,
            appearance: 0x03C1, // Keyboard appearance
            manufacturer_len: 0,
            p_manufacturer_data: ptr::null_mut(),
            service_data_len: 0,
            p_service_data: ptr::null_mut(),
            service_uuid_len: 0,
            p_service_uuid: ptr::null_mut(),
            flag: (ESP_BLE_ADV_FLAG_GEN_DISC | ESP_BLE_ADV_FLAG_BREDR_NOT_SPT)
                as u8,
        };

        esp_ble_gap_config_adv_data(&mut adv_data);
    }
}

fn start_advertising_now() {
    unsafe {
        let adv_params = esp_ble_adv_params_t {
            adv_int_min: 0x20,
            adv_int_max: 0x40,
            adv_type: esp_ble_adv_type_t_ADV_TYPE_IND,
            own_addr_type: esp_ble_addr_type_t_BLE_ADDR_TYPE_PUBLIC,
            peer_addr: [0; 6],
            peer_addr_type: esp_ble_addr_type_t_BLE_ADDR_TYPE_PUBLIC,
            channel_map: esp_ble_adv_channel_t_ADV_CHNL_ALL,
            adv_filter_policy:
                esp_ble_adv_filter_t_ADV_FILTER_ALLOW_SCAN_ANY_CON_ANY,
        };
        esp_ble_gap_start_advertising(&adv_params as *const _ as *mut _);
    }
}

unsafe extern "C" fn gap_event_handler(
    event: esp_gap_ble_cb_event_t,
    param: *mut esp_ble_gap_cb_param_t,
) {
    match event {
        esp_gap_ble_cb_event_t_ESP_GAP_BLE_ADV_DATA_SET_COMPLETE_EVT => {
            info!("Advertising data set complete");
            start_advertising_now();
        }
        esp_gap_ble_cb_event_t_ESP_GAP_BLE_ADV_START_COMPLETE_EVT => {
            info!("Advertising started");
        }
        esp_gap_ble_cb_event_t_ESP_GAP_BLE_SEC_REQ_EVT => {
            info!("Security request received");
            if !param.is_null() {
                let p = &(*param).ble_security;
                esp_ble_gap_security_rsp(
                    p.ble_req.bd_addr.as_ptr() as *mut u8,
                    true,
                );
            }
        }
        esp_gap_ble_cb_event_t_ESP_GAP_BLE_AUTH_CMPL_EVT => {
            info!("Authentication complete");
        }
        esp_gap_ble_cb_event_t_ESP_GAP_BLE_PASSKEY_NOTIF_EVT => {
            info!("Passkey notification");
        }
        _ => {}
    }
}

unsafe extern "C" fn gatts_event_handler(
    event: esp_gatts_cb_event_t,
    gatts_if: esp_gatt_if_t,
    param: *mut esp_ble_gatts_cb_param_t,
) {
    match event {
        esp_gatts_cb_event_t_ESP_GATTS_REG_EVT => {
            info!("GATT server registered");
            GATTS_IF.store(gatts_if, Ordering::Relaxed);

            // Create HID service
            let service_uuid = esp_bt_uuid_t {
                len: 2,
                uuid: esp_bt_uuid_t__bindgen_ty_1 {
                    uuid16: HID_SERVICE_UUID,
                },
            };

            let mut srvc_id = esp_gatt_srvc_id_t {
                is_primary: true,
                id: esp_gatt_id_t {
                    uuid: service_uuid,
                    inst_id: 0,
                },
            };

            esp_ble_gatts_create_service(
                gatts_if,
                &mut srvc_id,
                20, // Number of handles
            );

            start_advertising();
        }
        esp_gatts_cb_event_t_ESP_GATTS_CREATE_EVT => {
            if !param.is_null() {
                let p = &(*param).create;
                info!("Service created, handle: {}", p.service_handle);

                // Start service
                esp_ble_gatts_start_service(p.service_handle);

                // Add HID Report Map characteristic
                let mut char_uuid = esp_bt_uuid_t {
                    len: 2,
                    uuid: esp_bt_uuid_t__bindgen_ty_1 {
                        uuid16: HID_REPORT_MAP_UUID,
                    },
                };
                esp_ble_gatts_add_char(
                    p.service_handle,
                    &mut char_uuid,
                    ESP_GATT_PERM_READ as u16,
                    ESP_GATT_CHAR_PROP_BIT_READ as u8,
                    ptr::null_mut(),
                    ptr::null_mut(),
                );
            }
        }
        esp_gatts_cb_event_t_ESP_GATTS_ADD_CHAR_EVT => {
            if !param.is_null() {
                let p = &(*param).add_char;
                info!("Characteristic added, handle: {}", p.attr_handle);

                let char_uuid = p.char_uuid.uuid.uuid16;

                if char_uuid == HID_REPORT_MAP_UUID {
                    // Store Report Map handle
                    HID_REPORT_MAP_HANDLE
                        .store(p.attr_handle, Ordering::Relaxed);
                    info!("HID Report Map handle: {}", p.attr_handle);

                    // Add HID Information characteristic
                    let mut info_uuid = esp_bt_uuid_t {
                        len: 2,
                        uuid: esp_bt_uuid_t__bindgen_ty_1 {
                            uuid16: HID_INFO_UUID,
                        },
                    };
                    esp_ble_gatts_add_char(
                        p.service_handle,
                        &mut info_uuid,
                        ESP_GATT_PERM_READ as u16,
                        ESP_GATT_CHAR_PROP_BIT_READ as u8,
                        ptr::null_mut(),
                        ptr::null_mut(),
                    );
                } else if char_uuid == HID_INFO_UUID {
                    // Store HID Info handle
                    HID_INFO_HANDLE.store(p.attr_handle, Ordering::Relaxed);
                    info!("HID Info handle: {}", p.attr_handle);

                    // Add HID Control Point characteristic
                    let mut ctrl_uuid = esp_bt_uuid_t {
                        len: 2,
                        uuid: esp_bt_uuid_t__bindgen_ty_1 {
                            uuid16: HID_CONTROL_POINT_UUID,
                        },
                    };
                    esp_ble_gatts_add_char(
                        p.service_handle,
                        &mut ctrl_uuid,
                        ESP_GATT_PERM_WRITE as u16,
                        ESP_GATT_CHAR_PROP_BIT_WRITE_NR as u8,
                        ptr::null_mut(),
                        ptr::null_mut(),
                    );
                } else if char_uuid == HID_CONTROL_POINT_UUID {
                    // Add HID Report characteristic (keyboard input)
                    let mut report_uuid = esp_bt_uuid_t {
                        len: 2,
                        uuid: esp_bt_uuid_t__bindgen_ty_1 {
                            uuid16: HID_REPORT_CHAR_UUID,
                        },
                    };
                    esp_ble_gatts_add_char(
                        p.service_handle,
                        &mut report_uuid,
                        (ESP_GATT_PERM_READ | ESP_GATT_PERM_WRITE) as u16,
                        (ESP_GATT_CHAR_PROP_BIT_READ
                            | ESP_GATT_CHAR_PROP_BIT_NOTIFY)
                            as u8,
                        ptr::null_mut(),
                        ptr::null_mut(),
                    );
                } else if char_uuid == HID_REPORT_CHAR_UUID {
                    // Store the report handle for sending data
                    HID_REPORT_HANDLE.store(p.attr_handle, Ordering::Relaxed);
                    info!(
                        "HID Report characteristic handle: {}",
                        p.attr_handle
                    );

                    // Add CCC descriptor for notifications
                    let mut ccc_uuid = esp_bt_uuid_t {
                        len: 2,
                        uuid: esp_bt_uuid_t__bindgen_ty_1 {
                            uuid16: CCC_DESCRIPTOR_UUID,
                        },
                    };
                    esp_ble_gatts_add_char_descr(
                        p.service_handle,
                        &mut ccc_uuid,
                        (ESP_GATT_PERM_READ | ESP_GATT_PERM_WRITE) as u16,
                        ptr::null_mut(),
                        ptr::null_mut(),
                    );
                }
            }
        }
        esp_gatts_cb_event_t_ESP_GATTS_ADD_CHAR_DESCR_EVT => {
            info!("Descriptor added");
        }
        esp_gatts_cb_event_t_ESP_GATTS_START_EVT => {
            info!("Service started");
        }
        esp_gatts_cb_event_t_ESP_GATTS_CONNECT_EVT => {
            if !param.is_null() {
                let p = &(*param).connect;
                info!("Device connected, conn_id: {}", p.conn_id);
                CONN_ID.store(p.conn_id as u16, Ordering::Relaxed);
                CONNECTED.store(true, Ordering::Relaxed);

                // Request security
                esp_ble_set_encryption(
                    p.remote_bda.as_ptr() as *mut u8,
                    esp_ble_sec_act_t_ESP_BLE_SEC_ENCRYPT_MITM,
                );
            }
        }
        esp_gatts_cb_event_t_ESP_GATTS_DISCONNECT_EVT => {
            info!("Device disconnected");
            CONNECTED.store(false, Ordering::Relaxed);
            // Restart advertising
            start_advertising();
        }
        esp_gatts_cb_event_t_ESP_GATTS_READ_EVT => {
            if !param.is_null() {
                let p = &(*param).read;
                let report_map_handle =
                    HID_REPORT_MAP_HANDLE.load(Ordering::Relaxed);
                let info_handle = HID_INFO_HANDLE.load(Ordering::Relaxed);
                let report_handle = HID_REPORT_HANDLE.load(Ordering::Relaxed);

                // Prepare response based on what's being read
                let mut rsp = esp_gatt_rsp_t {
                    attr_value: esp_gatt_value_t {
                        handle: p.handle,
                        offset: 0,
                        len: 0,
                        auth_req: 0,
                        value: [0; 600],
                    },
                };

                // Return appropriate data based on handle
                if p.handle == report_map_handle {
                    // HID Report Map - support long reads with offset
                    let offset = if p.is_long { p.offset as usize } else { 0 };
                    let data = HID_REPORT_MAP;
                    if offset < data.len() {
                        let remaining = &data[offset..];
                        let len = remaining.len().min(22); // BLE MTU limit
                        rsp.attr_value.len = len as u16;
                        rsp.attr_value.value[..len]
                            .copy_from_slice(&remaining[..len]);
                    }
                    info!(
                        "Read Report Map, offset: {}, len: {}",
                        offset, rsp.attr_value.len
                    );
                } else if p.handle == info_handle {
                    // HID Information (4 bytes)
                    rsp.attr_value.len = HID_INFO.len() as u16;
                    rsp.attr_value.value[..HID_INFO.len()]
                        .copy_from_slice(HID_INFO);
                    info!("Read HID Info");
                } else if p.handle == report_handle {
                    // HID Report - return empty report
                    rsp.attr_value.len = 9;
                    rsp.attr_value.value[0] = 0x01; // Report ID
                    info!("Read HID Report");
                } else {
                    info!("Read unknown handle: {}", p.handle);
                }

                esp_ble_gatts_send_response(
                    gatts_if,
                    p.conn_id,
                    p.trans_id,
                    esp_gatt_status_t_ESP_GATT_OK,
                    &mut rsp,
                );
            }
        }
        esp_gatts_cb_event_t_ESP_GATTS_WRITE_EVT => {
            if !param.is_null() {
                let p = &(*param).write;
                info!("Write request, handle: {}", p.handle);

                if p.need_rsp {
                    esp_ble_gatts_send_response(
                        gatts_if,
                        p.conn_id,
                        p.trans_id,
                        esp_gatt_status_t_ESP_GATT_OK,
                        ptr::null_mut(),
                    );
                }
            }
        }
        _ => {}
    }
}
