#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
#[cfg(target_os = "windows")]
const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
#[cfg(target_os = "windows")]
const VALUE_NAME: &str = "CharybdisBattery";

pub fn is_enabled() -> bool {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("reg")
            .args(["query", RUN_KEY, "/v", VALUE_NAME])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    #[cfg(target_os = "linux")]
    {
        desktop_file_path().map(|p| p.exists()).unwrap_or(false)
    }
}

pub fn set_enabled(enable: bool) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let out = if enable {
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            let exe_quoted = format!("\"{}\"", exe.display());
            std::process::Command::new("reg")
                .args([
                    "add",
                    RUN_KEY,
                    "/v",
                    VALUE_NAME,
                    "/t",
                    "REG_SZ",
                    "/d",
                    &exe_quoted,
                    "/f",
                ])
                .creation_flags(CREATE_NO_WINDOW)
                .output()
                .map_err(|e| e.to_string())?
        } else {
            std::process::Command::new("reg")
                .args(["delete", RUN_KEY, "/v", VALUE_NAME, "/f"])
                .creation_flags(CREATE_NO_WINDOW)
                .output()
                .map_err(|e| e.to_string())?
        };
        if out.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&out.stderr).into_owned())
        }
    }

    #[cfg(target_os = "linux")]
    {
        let path = desktop_file_path().ok_or_else(|| "HOME is not set".to_string())?;
        if enable {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            let content = format!(
                "[Desktop Entry]\n\
                 Type=Application\n\
                 Name=Charybdis Battery\n\
                 Comment=System tray battery monitor for Charybdis Mini\n\
                 Exec=\"{}\"\n\
                 Terminal=false\n\
                 X-GNOME-Autostart-enabled=true\n",
                exe.display()
            );
            std::fs::write(&path, content).map_err(|e| e.to_string())
        } else if path.exists() {
            std::fs::remove_file(&path).map_err(|e| e.to_string())
        } else {
            Ok(())
        }
    }
}

#[cfg(target_os = "linux")]
fn desktop_file_path() -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(
        std::path::PathBuf::from(home)
            .join(".config")
            .join("autostart")
            .join("charybdis-battery.desktop"),
    )
}
