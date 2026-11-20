use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU8, Ordering},
};

use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{Icon, TrayIconBuilder};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::WindowId;

use crate::autostart;
use crate::hid::BatterySnapshot;
use crate::icon::{ICON_SIZE, render_rgba};

#[derive(Debug, Clone)]
pub enum UserEvent {
    SnapshotUpdated,
}

struct TrayApp {
    shared: Arc<Mutex<BatterySnapshot>>,
    threshold: Arc<AtomicU8>,
    force_poll: Arc<AtomicBool>,
    tray: Option<tray_icon::TrayIcon>,
    status_item: MenuItem,
    left_item: MenuItem,
    right_item: MenuItem,
    refresh_item: MenuItem,
    autostart_item: CheckMenuItem,
    quit_item: MenuItem,
    threshold_items: Vec<(u8, CheckMenuItem)>,
}

impl TrayApp {
    fn build_icon(snap: &BatterySnapshot) -> Icon {
        let rgba = render_rgba(snap);
        Icon::from_rgba(rgba, ICON_SIZE, ICON_SIZE).expect("valid 32x32 RGBA icon")
    }

    fn update_ui(&self) {
        let snap = *self.shared.lock().unwrap();
        let status_text = if !snap.connected {
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

        self.status_item.set_text(status_text);
        self.left_item.set_text(format!(
            "Левая:  {}",
            BatterySnapshot::format_side(snap.left_level, snap.left_charge)
        ));
        self.right_item.set_text(format!(
            "Правая: {}",
            BatterySnapshot::format_side(snap.right_level, snap.right_charge)
        ));

        if let Some(ref tray) = self.tray {
            let _ = tray.set_tooltip(Some(snap.tooltip()));
            let _ = tray.set_icon(Some(Self::build_icon(&snap)));
        }
    }
}

impl ApplicationHandler<UserEvent> for TrayApp {
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {
        if self.tray.is_some() {
            return;
        }
        let menu = Menu::new();
        let threshold_menu = Submenu::new("Порог уведомления", true);
        for (_, item) in &self.threshold_items {
            let _ = threshold_menu.append(item);
        }

        let _ = menu.append(&self.status_item);
        let _ = menu.append(&self.left_item);
        let _ = menu.append(&self.right_item);
        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&self.refresh_item);
        let _ = menu.append(&threshold_menu);
        let _ = menu.append(&self.autostart_item);
        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&self.quit_item);

        let snap = *self.shared.lock().unwrap();
        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip(snap.tooltip())
            .with_icon(Self::build_icon(&snap))
            .build()
            .expect("failed to create Windows tray icon");
        self.tray = Some(tray);
        self.update_ui();
    }

    fn window_event(&mut self, _event_loop: &ActiveEventLoop, _id: WindowId, _event: WindowEvent) {}

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::SnapshotUpdated => self.update_ui(),
        }

        while let Ok(ev) = MenuEvent::receiver().try_recv() {
            if ev.id == self.quit_item.id() {
                event_loop.exit();
            } else if ev.id == self.refresh_item.id() {
                self.force_poll.store(true, Ordering::Relaxed);
            } else if ev.id == self.autostart_item.id() {
                let checked = self.autostart_item.is_checked();
                if autostart::set_enabled(checked).is_err() {
                    self.autostart_item.set_checked(!checked);
                }
            } else {
                for (pct, item) in &self.threshold_items {
                    if ev.id == item.id() {
                        self.threshold.store(*pct, Ordering::Relaxed);
                        for (other_pct, other_item) in &self.threshold_items {
                            other_item.set_checked(*other_pct == *pct);
                        }
                    }
                }
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        while let Ok(ev) = MenuEvent::receiver().try_recv() {
            if ev.id == self.quit_item.id() {
                event_loop.exit();
            } else if ev.id == self.refresh_item.id() {
                self.force_poll.store(true, Ordering::Relaxed);
            } else if ev.id == self.autostart_item.id() {
                let checked = self.autostart_item.is_checked();
                if autostart::set_enabled(checked).is_err() {
                    self.autostart_item.set_checked(!checked);
                }
            } else {
                for (pct, item) in &self.threshold_items {
                    if ev.id == item.id() {
                        self.threshold.store(*pct, Ordering::Relaxed);
                        for (other_pct, other_item) in &self.threshold_items {
                            other_item.set_checked(*other_pct == *pct);
                        }
                    }
                }
            }
        }
    }
}

pub fn run_tray(
    shared: Arc<Mutex<BatterySnapshot>>,
    threshold: Arc<AtomicU8>,
    force_poll: Arc<AtomicBool>,
    on_proxy: Box<dyn FnOnce(Box<dyn Fn() + Send + Sync>)>,
) {
    let event_loop = EventLoop::<UserEvent>::with_user_event()
        .build()
        .expect("failed to create winit event loop");
    let proxy = event_loop.create_proxy();
    on_proxy(Box::new(move || {
        let _ = proxy.send_event(UserEvent::SnapshotUpdated);
    }));

    let current_thr = threshold.load(Ordering::Relaxed);
    let threshold_items = [15u8, 20, 25, 30]
        .into_iter()
        .map(|pct| {
            (
                pct,
                CheckMenuItem::new(format!("{pct}%"), true, pct == current_thr, None),
            )
        })
        .collect();

    let mut app = TrayApp {
        shared,
        threshold,
        force_poll,
        tray: None,
        status_item: MenuItem::new("Статус: проверка...", false, None),
        left_item: MenuItem::new("Левая:  —", false, None),
        right_item: MenuItem::new("Правая: —", false, None),
        refresh_item: MenuItem::new("Обновить сейчас", true, None),
        autostart_item: CheckMenuItem::new("Автозапуск с системой", true, autostart::is_enabled(), None),
        quit_item: MenuItem::new("Выход", true, None),
        threshold_items,
    };

    let _ = event_loop.run_app(&mut app);
}
