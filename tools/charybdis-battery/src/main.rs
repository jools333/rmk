#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]

mod autostart;
mod hid;
mod icon;
mod notify;
#[cfg(target_os = "linux")]
mod tray_linux;
#[cfg(target_os = "windows")]
mod tray_win;

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU8, Ordering},
};
use std::time::{Duration, Instant};

use hid::poll_battery;
use notify::LowBatteryNotifier;

struct CliArgs {
    threshold: u8,
    poll_secs: u64,
    once: bool,
}

fn parse_args() -> CliArgs {
    let mut threshold = 20u8;
    let mut poll_secs = 5u64;
    let mut once = false;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--once" => once = true,
            "--threshold" => {
                if let Some(val) = args.next().and_then(|s| s.parse::<u8>().ok()) {
                    threshold = val.clamp(5, 95);
                }
            }
            "--poll-secs" => {
                if let Some(val) = args.next().and_then(|s| s.parse::<u64>().ok()) {
                    poll_secs = val.clamp(1, 300);
                }
            }
            _ => {}
        }
    }

    CliArgs {
        threshold,
        poll_secs,
        once,
    }
}

fn main() {
    let args = parse_args();

    if args.once {
        let snap = poll_battery();
        println!("{}", snap.tooltip());
        return;
    }

    let initial = poll_battery();
    let shared = Arc::new(Mutex::new(initial));
    let threshold = Arc::new(AtomicU8::new(args.threshold));
    let force_poll = Arc::new(AtomicBool::new(false));
    let notifier_cb: Arc<Mutex<Option<Box<dyn Fn() + Send + Sync>>>> = Arc::new(Mutex::new(None));

    let shared_bg = Arc::clone(&shared);
    let threshold_bg = Arc::clone(&threshold);
    let force_bg = Arc::clone(&force_poll);
    let notify_ui_bg = Arc::clone(&notifier_cb);
    let poll_interval = Duration::from_secs(args.poll_secs);

    std::thread::spawn(move || {
        let mut notifier = LowBatteryNotifier::new(threshold_bg.load(Ordering::Relaxed));
        notifier.check_and_notify(&initial);
        let mut last_poll = Instant::now();

        loop {
            std::thread::sleep(Duration::from_millis(200));

            let forced = force_bg.swap(false, Ordering::Relaxed);
            if !forced && last_poll.elapsed() < poll_interval {
                continue;
            }
            last_poll = Instant::now();

            let cur_thr = threshold_bg.load(Ordering::Relaxed);
            if cur_thr != notifier.threshold {
                notifier.set_threshold(cur_thr);
            }

            let snap = poll_battery();
            notifier.check_and_notify(&snap);

            let changed = {
                let mut guard = shared_bg.lock().unwrap();
                let changed = *guard != snap;
                *guard = snap;
                changed
            };

            if (changed || forced)
                && let Some(ref cb) = *notify_ui_bg.lock().unwrap()
            {
                cb();
            }
        }
    });

    let cb_slot = Arc::clone(&notifier_cb);
    let set_proxy = Box::new(move |cb: Box<dyn Fn() + Send + Sync>| {
        *cb_slot.lock().unwrap() = Some(cb);
    });

    #[cfg(target_os = "windows")]
    tray_win::run_tray(shared, threshold, force_poll, set_proxy);

    #[cfg(target_os = "linux")]
    tray_linux::run_tray(shared, threshold, force_poll, set_proxy);
}
