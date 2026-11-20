#[cfg(target_os = "windows")]
use hidapi::HidApi;

pub const CHARYBDIS_VID: u16 = 0x4C4B;
pub const CHARYBDIS_PID_WIRED: u16 = 0x4643;
pub const CHARYBDIS_PID_DONGLE: u16 = 0x4644;
#[cfg(target_os = "windows")]
pub const VIAL_USAGE_PAGE: u16 = 0xFF60;
#[cfg(target_os = "windows")]
pub const VIAL_USAGE: u16 = 0x0061;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportKind {
    Wired,
    Dongle,
}

impl TransportKind {
    pub fn label(self) -> &'static str {
        match self {
            TransportKind::Wired => "Провод",
            TransportKind::Dongle => "Донгл",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChargeState {
    Discharging,
    Charging,
    Unknown,
}

impl From<u8> for ChargeState {
    fn from(v: u8) -> Self {
        match v {
            0 => ChargeState::Discharging,
            1 => ChargeState::Charging,
            _ => ChargeState::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BatterySnapshot {
    pub connected: bool,
    pub transport: Option<TransportKind>,
    pub link_connected: bool,
    pub sleeping: bool,
    pub left_level: Option<u8>,
    pub right_level: Option<u8>,
    pub left_charge: ChargeState,
    pub right_charge: ChargeState,
}

impl Default for BatterySnapshot {
    fn default() -> Self {
        Self {
            connected: false,
            transport: None,
            link_connected: false,
            sleeping: false,
            left_level: None,
            right_level: None,
            left_charge: ChargeState::Unknown,
            right_charge: ChargeState::Unknown,
        }
    }
}

impl BatterySnapshot {
    pub fn format_side(level: Option<u8>, charge: ChargeState) -> String {
        match level {
            Some(pct) => {
                if charge == ChargeState::Charging {
                    format!("{pct}% (зарядка)")
                } else {
                    format!("{pct}%")
                }
            }
            None => "—".to_string(),
        }
    }

    pub fn tooltip(&self) -> String {
        if !self.connected {
            return "Charybdis Mini: не подключена".to_string();
        }
        let via = self.transport.map_or("", |t| t.label());
        if !self.link_connected {
            return format!("Charybdis Mini ({via}): ожидание связи...");
        }
        let sleep_suffix = if self.sleeping { " [сон]" } else { "" };
        format!(
            "Charybdis Mini ({via}{sleep_suffix})\nЛевая: {}\nПравая: {}",
            Self::format_side(self.left_level, self.left_charge),
            Self::format_side(self.right_level, self.right_charge),
        )
    }
}

/// Query split battery status via Vial command `[0x53, 0x04]`.
/// Prioritizes direct wired connection (`0x4643`) over wireless dongle (`0x4644`).
#[cfg(target_os = "windows")]
pub fn poll_battery() -> BatterySnapshot {
    let Ok(api) = HidApi::new() else {
        return BatterySnapshot::default();
    };

    let mut candidates: Vec<_> = api
        .device_list()
        .filter(|d| {
            d.vendor_id() == CHARYBDIS_VID
                && (d.product_id() == CHARYBDIS_PID_WIRED || d.product_id() == CHARYBDIS_PID_DONGLE)
                && d.usage_page() == VIAL_USAGE_PAGE
                && d.usage() == VIAL_USAGE
        })
        .collect();

    if candidates.is_empty() {
        return BatterySnapshot::default();
    }

    // Prefer wired (0x4643) before dongle (0x4644)
    candidates.sort_by_key(|d| if d.product_id() == CHARYBDIS_PID_WIRED { 0 } else { 1 });

    for info in candidates {
        let transport = if info.product_id() == CHARYBDIS_PID_WIRED {
            TransportKind::Wired
        } else {
            TransportKind::Dongle
        };

        let Ok(dev) = info.open_device(&api) else {
            continue;
        };

        let mut req = [0u8; 33];
        req[0] = 0x00; // HID Report ID 0
        req[1] = 0x53;
        req[2] = 0x04;

        if dev.write(&req).is_err() {
            continue;
        }

        let mut resp = [0u8; 32];
        match dev.read_timeout(&mut resp, 200) {
            Ok(n) if n >= 8 && resp[0] == 0x53 && resp[1] == 0x04 => {
                return parse_battery_response(&resp, transport);
            }
            _ => {
                return BatterySnapshot {
                    connected: true,
                    transport: Some(transport),
                    ..Default::default()
                };
            }
        }
    }

    BatterySnapshot::default()
}

/// Pure-Rust Linux hidraw implementation (zero libudev/libusb build dependencies).
#[cfg(target_os = "linux")]
pub fn poll_battery() -> BatterySnapshot {
    use std::fs::{self, OpenOptions};
    use std::io::{Read, Write};
    use std::os::unix::io::AsRawFd;

    const VIAL_USAGE_SIGNATURE: &[u8] = &[0x06, 0x60, 0xFF, 0x09, 0x61];

    let Ok(entries) = fs::read_dir("/sys/class/hidraw") else {
        return BatterySnapshot::default();
    };

    let mut candidates: Vec<(u8, String, TransportKind)> = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let sys_dev = format!("/sys/class/hidraw/{name}/device");
        let Ok(uevent) = fs::read_to_string(format!("{sys_dev}/uevent")) else {
            continue;
        };
        let upper = uevent.to_ascii_uppercase();
        if !upper.contains(&format!(":0000{CHARYBDIS_VID:04X}:")) {
            continue;
        }
        let transport = if upper.contains(&format!(":0000{CHARYBDIS_PID_WIRED:04X}")) {
            TransportKind::Wired
        } else if upper.contains(&format!(":0000{CHARYBDIS_PID_DONGLE:04X}")) {
            TransportKind::Dongle
        } else {
            continue;
        };

        let Ok(desc) = fs::read(format!("{sys_dev}/report_descriptor")) else {
            continue;
        };
        if !desc.windows(VIAL_USAGE_SIGNATURE.len()).any(|w| w == VIAL_USAGE_SIGNATURE) {
            continue;
        }

        let priority = if transport == TransportKind::Wired { 0 } else { 1 };
        candidates.push((priority, format!("/dev/{name}"), transport));
    }

    if candidates.is_empty() {
        return BatterySnapshot::default();
    }
    candidates.sort_by_key(|c| c.0);

    for (_, dev_path, transport) in candidates {
        let Ok(mut file) = OpenOptions::new().read(true).write(true).open(&dev_path) else {
            continue;
        };

        let mut req = [0u8; 33];
        req[0] = 0x00;
        req[1] = 0x53;
        req[2] = 0x04;
        if file.write_all(&req).is_err() {
            continue;
        }

        // Wait up to 200ms using poll() via non-blocking or O_NONBLOCK? Set nonblocking via fcntl or just poll
        let fd = file.as_raw_fd();
        let mut pfd = libc_pollfd {
            fd,
            events: 0x0001, // POLLIN
            revents: 0,
        };
        let ready = unsafe { poll(&mut pfd, 1, 200) };
        if ready > 0 {
            let mut resp = [0u8; 32];
            if let Ok(n) = file.read(&mut resp)
                && n >= 8
                && resp[0] == 0x53
                && resp[1] == 0x04
            {
                return parse_battery_response(&resp, transport);
            }
        }

        return BatterySnapshot {
            connected: true,
            transport: Some(transport),
            ..Default::default()
        };
    }

    BatterySnapshot::default()
}

#[cfg(target_os = "linux")]
#[repr(C)]
struct libc_pollfd {
    fd: i32,
    events: i16,
    revents: i16,
}

#[cfg(target_os = "linux")]
unsafe extern "C" {
    fn poll(fds: *mut libc_pollfd, nfds: usize, timeout: i32) -> i32;
}

fn parse_battery_response(resp: &[u8; 32], transport: TransportKind) -> BatterySnapshot {
    let right_level = if resp[2] <= 100 { Some(resp[2]) } else { None };
    let left_level = if resp[3] <= 100 { Some(resp[3]) } else { None };
    let right_charge = ChargeState::from(resp[4]);
    let left_charge = ChargeState::from(resp[5]);
    let link_connected = resp[6] != 0;
    let sleeping = resp[7] != 0;

    BatterySnapshot {
        connected: true,
        transport: Some(transport),
        link_connected,
        sleeping,
        left_level,
        right_level,
        left_charge,
        right_charge,
    }
}
