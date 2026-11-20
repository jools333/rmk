#[cfg(target_os = "windows")]
use notify_rust::Notification;

use crate::hid::{BatterySnapshot, ChargeState};

#[derive(Debug, Default)]
struct HalfAlertState {
    warned_low: bool,
    warned_critical: bool,
}

#[derive(Debug)]
pub struct LowBatteryNotifier {
    pub threshold: u8,
    pub critical_threshold: u8,
    left: HalfAlertState,
    right: HalfAlertState,
}

impl LowBatteryNotifier {
    pub fn new(threshold: u8) -> Self {
        Self {
            threshold,
            critical_threshold: (threshold / 2).max(5).min(10),
            left: HalfAlertState::default(),
            right: HalfAlertState::default(),
        }
    }

    pub fn set_threshold(&mut self, threshold: u8) {
        self.threshold = threshold;
        self.critical_threshold = (threshold / 2).max(5).min(10);
        self.left = HalfAlertState::default();
        self.right = HalfAlertState::default();
    }

    pub fn check_and_notify(&mut self, snap: &BatterySnapshot) {
        if !snap.connected || !snap.link_connected {
            return;
        }
        Self::check_half(
            "Левая половина",
            snap.left_level,
            snap.left_charge,
            self.threshold,
            self.critical_threshold,
            &mut self.left,
        );
        Self::check_half(
            "Правая половина (трекбол)",
            snap.right_level,
            snap.right_charge,
            self.threshold,
            self.critical_threshold,
            &mut self.right,
        );
    }

    fn check_half(
        name: &str,
        level: Option<u8>,
        charge: ChargeState,
        threshold: u8,
        critical_threshold: u8,
        state: &mut HalfAlertState,
    ) {
        if charge == ChargeState::Charging {
            state.warned_low = false;
            state.warned_critical = false;
            return;
        }
        let Some(pct) = level else {
            return;
        };

        // Hysteresis: reset warning flags once battery recovers at least 5% above threshold
        if pct >= threshold.saturating_add(5) {
            state.warned_low = false;
            state.warned_critical = false;
            return;
        }

        if pct <= critical_threshold && !state.warned_critical {
            state.warned_critical = true;
            state.warned_low = true;
            send_notification(
                &format!("Charybdis Mini: критический заряд ({pct}%)"),
                &format!("{name} почти разряжена ({pct}%). Подключите зарядку!"),
                true,
            );
        } else if pct <= threshold && !state.warned_low {
            state.warned_low = true;
            send_notification(
                &format!("Charybdis Mini: низкий заряд ({pct}%)"),
                &format!("{name}: осталось {pct}% заряда."),
                false,
            );
        }
    }
}

fn send_notification(summary: &str, body: &str, critical: bool) {
    #[cfg(target_os = "windows")]
    {
        let _ = critical;
        let _ = Notification::new()
            .appname("Charybdis Battery")
            .summary(summary)
            .body(body)
            .show();
    }
    #[cfg(target_os = "linux")]
    {
        let urgency = if critical { "critical" } else { "normal" };
        let _ = std::process::Command::new("notify-send")
            .args([
                "--app-name=Charybdis Battery",
                "--icon=battery-low",
                &format!("--urgency={urgency}"),
                summary,
                body,
            ])
            .spawn();
    }
}
