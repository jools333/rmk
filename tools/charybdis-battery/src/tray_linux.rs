use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU8, Ordering},
};

use ksni::blocking::TrayMethods;
use ksni::menu::{RadioGroup, RadioItem, StandardItem};
use ksni::{Icon, MenuItem, ToolTip, Tray};

use crate::hid::BatterySnapshot;
use crate::icon::{ICON_SIZE, render_argb32};

struct CharybdisTray {
    shared: Arc<Mutex<BatterySnapshot>>,
    threshold: Arc<AtomicU8>,
    force_poll: Arc<AtomicBool>,
}

const THRESHOLDS: [u8; 4] = [15, 20, 25, 30];

impl Tray for CharybdisTray {
    fn id(&self) -> String {
        "charybdis-battery".to_string()
    }

    fn title(&self) -> String {
        let snap = *self.shared.lock().unwrap();
        if !snap.connected {
            "Charybdis: отключена".to_string()
        } else if !snap.link_connected {
            "Charybdis: нет связи".to_string()
        } else {
            format!(
                "L:{} R:{}",
                snap.left_level
                    .map_or("—".to_string(), |v| format!("{v}%")),
                snap.right_level
                    .map_or("—".to_string(), |v| format!("{v}%")),
            )
        }
    }

    fn icon_pixmap(&self) -> Vec<Icon> {
        let snap = *self.shared.lock().unwrap();
        vec![Icon {
            width: ICON_SIZE as i32,
            height: ICON_SIZE as i32,
            data: render_argb32(&snap),
        }]
    }

    fn tool_tip(&self) -> ToolTip {
        let snap = *self.shared.lock().unwrap();
        ToolTip {
            title: "Charybdis Mini".to_string(),
            description: snap.tooltip(),
            icon_name: String::new(),
            icon_pixmap: Vec::new(),
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let snap = *self.shared.lock().unwrap();
        let status_label = if !snap.connected {
            "Статус: не подключена".to_string()
        } else if !snap.link_connected {
            format!(
                "Статус: {} (нет связи)",
                snap.transport.map_or("", |t| t.label())
            )
        } else {
            format!(
                "Статус: {} (онлайн)",
                snap.transport.map_or("", |t| t.label())
            )
        };

        let left_label = format!(
            "Левая:  {}",
            BatterySnapshot::format_side(snap.left_level, snap.left_charge)
        );
        let right_label = format!(
            "Правая: {}",
            BatterySnapshot::format_side(snap.right_level, snap.right_charge)
        );

        let cur_thr = self.threshold.load(Ordering::Relaxed);
        let selected_idx = THRESHOLDS
            .iter()
            .position(|&v| v == cur_thr)
            .unwrap_or(1);

        vec![
            StandardItem {
                label: status_label,
                enabled: false,
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: left_label,
                enabled: false,
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: right_label,
                enabled: false,
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Обновить сейчас".to_string(),
                activate: Box::new(|this: &mut Self| {
                    this.force_poll.store(true, Ordering::Relaxed);
                }),
                ..Default::default()
            }
            .into(),
            ksni::menu::SubMenu {
                label: "Порог уведомления".to_string(),
                submenu: vec![
                    RadioGroup {
                        selected: selected_idx,
                        select: Box::new(|this: &mut Self, current| {
                            if let Some(&pct) = THRESHOLDS.get(current) {
                                this.threshold.store(pct, Ordering::Relaxed);
                            }
                        }),
                        options: THRESHOLDS
                            .iter()
                            .map(|pct| RadioItem {
                                label: format!("{pct}%"),
                                ..Default::default()
                            })
                            .collect(),
                    }
                    .into(),
                ],
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Выход".to_string(),
                activate: Box::new(|_| std::process::exit(0)),
                ..Default::default()
            }
            .into(),
        ]
    }
}

pub fn run_tray(
    shared: Arc<Mutex<BatterySnapshot>>,
    threshold: Arc<AtomicU8>,
    force_poll: Arc<AtomicBool>,
    on_proxy: Box<dyn FnOnce(Box<dyn Fn() + Send + Sync>)>,
) {
    let tray = CharybdisTray {
        shared,
        threshold,
        force_poll,
    };
    let handle = tray.spawn().expect("failed to start Linux tray service");
    on_proxy(Box::new(move || {
        handle.update(|_| {});
    }));
    loop {
        std::thread::sleep(std::time::Duration::from_secs(3600));
    }
}
