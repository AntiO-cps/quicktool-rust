//! Read-only native smoke checks; no real clipboard, registry writes or launches.
#![cfg(windows)]
use quicktool_rust::{platform, shell_icons::IconCache, system};
#[test]
fn known_folders_are_native_absolute_paths() {
    let folders = platform::known_folders();
    assert!(
        folders
            .iter()
            .any(|(name, path)| name == "Home" && path.is_absolute())
    );
    assert!(
        folders
            .iter()
            .any(|(name, path)| name == "Desktop" && path.is_absolute())
    );
    assert!(
        folders
            .iter()
            .any(|(name, path)| name == "Downloads" && path.is_absolute())
    );
}
#[test]
fn shell_application_sources_and_targets() {
    let apps = system::app_index_cached(true);
    let counts = apps.iter().fold(
        std::collections::BTreeMap::<String, usize>::new(),
        |mut a, x| {
            *a.entry(x.source.clone()).or_default() += 1;
            a
        },
    );
    eprintln!("native application source counts: {counts:?}");
    assert!(!apps.is_empty());
    assert!(apps.iter().any(|a| a.source == "StartApps"));
    assert!(
        apps.iter()
            .all(|a| !a.name.trim().is_empty() && !a.target.trim().is_empty())
    );
    assert!(
        apps.iter()
            .filter(|a| a.source == "StartApps")
            .all(|a| a.target.starts_with("shell:AppsFolder\\") && !a.target.contains("::{"))
    );
}
#[test]
fn extracts_actual_shell_icon_with_cache() {
    let path =
        std::path::PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("System32/cmd.exe");
    let mut cache = IconCache::default();
    let icon = cache.get(&path).unwrap();
    assert_eq!((icon.width, icon.height, icon.rgba.len()), (32, 32, 4096));
    assert!(icon.rgba.chunks_exact(4).any(|p| p[3] > 0));
    assert_eq!(icon.rgba, cache.get(&path).unwrap().rgba);
}
#[test]
fn invalid_inputs_do_not_execute() {
    assert!(platform::open("", None, None, false).is_err());
    assert!(platform::open("abc\0def", None, None, false).is_err());
    assert!(system::run_shell("cmd", "", Some("Q:\\quicktool-nonexistent-7fc9"), false).is_err());
    assert!(system::copy_flag("../", "svg").is_err());
    assert!(platform::copy_image(0, 0, &[]).is_err());
}

#[test]
fn hover_escape_empty_and_suspended_lifecycle() {
    let mut escape = platform::PinEscape::new(|| {}).unwrap();
    escape.update(Vec::new(), false).unwrap();
    assert_eq!(escape.status(), platform::PinEscapeStatus::default());
    escape.update(vec![(u64::MAX, 0)], true).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(70));
    assert!(!escape.status().registered);
    assert!(escape.status().hovered_id.is_none());
    assert!(escape.events.try_recv().is_err());
    drop(escape);
}

#[test]
fn adjustable_backdrop_tracks_owner_and_releases_window() {
    use quicktool_rust::backdrop::Backdrop;
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    fn wait_for(
        surface: &Backdrop,
        check: impl Fn(quicktool_rust::backdrop::BackdropStatus) -> bool,
    ) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        loop {
            if check(surface.status()) {
                return;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "backdrop status timeout: {:?}",
                surface.status()
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
    unsafe {
        let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
        let title: Vec<u16> = "QuickTool isolated offscreen compositor test\0"
            .encode_utf16()
            .collect();
        let hwnd = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            class.as_ptr(),
            title.as_ptr(),
            WS_POPUP | WS_VISIBLE,
            -32000,
            -32000,
            120,
            100,
            null_mut(),
            null_mut(),
            null_mut(),
            null(),
        );
        assert!(!hwnd.is_null());
        struct Owner(windows_sys::Win32::Foundation::HWND);
        impl Drop for Owner {
            fn drop(&mut self) {
                unsafe {
                    DestroyWindow(self.0);
                }
            }
        }
        let _owner = Owner(hwnd);
        let mut surface = Backdrop::new(hwnd as isize).unwrap();
        for blur in [0.0, 12.0, 28.0, 48.0] {
            surface.update(true, false, true, blur, 5, 12.0).unwrap();
            wait_for(&surface, |s| {
                (s.blur - blur).abs() < 0.001 && s.visible == (blur > 0.0)
            });
        }
        wait_for(&surface, |s| s.bounds == [-31995, -31995, 110, 90]);
        SetWindowPos(hwnd, HWND_TOPMOST, -31980, -31990, 160, 130, SWP_NOACTIVATE);
        surface.update(true, true, false, 28.0, 5, 12.0).unwrap();
        wait_for(&surface, |s| {
            s.visible && s.topmost && s.bounds == [-31975, -31985, 150, 120]
        });
        ShowWindow(hwnd, SW_HIDE);
        surface.update(false, true, false, 28.0, 5, 12.0).unwrap();
        wait_for(&surface, |s| !s.visible);
        ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        surface.update(true, true, false, 28.0, 5, 12.0).unwrap();
        wait_for(&surface, |s| s.visible);
        let native = surface.status().window;
        drop(surface);
        assert_eq!(IsWindow(native as _), 0);
        let title_text = format!("QuickTool isolated glow {}", std::process::id());
        let title: Vec<u16> = title_text.encode_utf16().chain(Some(0)).collect();
        let companion = CreateWindowExW(
            0,
            class.as_ptr(),
            title.as_ptr(),
            WS_POPUP,
            -32200,
            -32200,
            200,
            150,
            null_mut(),
            null_mut(),
            null_mut(),
            null(),
        );
        assert!(!companion.is_null());
        let _companion = Owner(companion);
        platform::configure_glow(&title_text, hwnd as isize, true, true).unwrap();
        let mut rect: windows_sys::Win32::Foundation::RECT = std::mem::zeroed();
        GetWindowRect(companion, &mut rect);
        assert_eq!(
            [
                rect.left,
                rect.top,
                rect.right - rect.left,
                rect.bottom - rect.top
            ],
            [-32200, -32200, 200, 150]
        );
        let style = GetWindowLongW(companion, GWL_EXSTYLE) as u32;
        assert_eq!(
            style & (WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TRANSPARENT),
            WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TRANSPARENT
        );
        assert_ne!(IsWindowVisible(companion), 0);
        platform::place_companion_window(hwnd as isize, companion as isize, 10, true, true, true)
            .unwrap();
        GetWindowRect(companion, &mut rect);
        assert_eq!(
            [
                rect.left,
                rect.top,
                rect.right - rect.left,
                rect.bottom - rect.top
            ],
            [-31990, -32000, 180, 150]
        );
        platform::configure_glow(&title_text, hwnd as isize, false, true).unwrap();
        assert_eq!(IsWindowVisible(companion), 0);
        assert!(
            platform::place_companion_window(0, companion as isize, 10, true, true, true).is_err()
        );
    }
}

#[test]
#[ignore = "Requires permission to create a private Windows window station; never uses the user's clipboard"]
fn isolated_clipboard_formats() {
    if std::env::var_os("QUICKTOOL_ISOLATED_CLIPBOARD").is_none() {
        let out = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "isolated_clipboard_formats",
                "--ignored",
                "--nocapture",
            ])
            .env("QUICKTOOL_ISOLATED_CLIPBOARD", "1")
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "isolated clipboard process failed: {} {}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        eprintln!("{}", String::from_utf8_lossy(&out.stdout));
        return;
    }
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::System::{StationsAndDesktops::*, Threading::GetCurrentThreadId};
    unsafe {
        let old_station = GetProcessWindowStation();
        let old_desktop = GetThreadDesktop(GetCurrentThreadId());
        let name: Vec<u16> = format!("QuickToolClipboardTest{}\0", std::process::id())
            .encode_utf16()
            .collect();
        let station = CreateWindowStationW(name.as_ptr(), 0, 0x0000037f, null());
        assert!(
            !station.is_null(),
            "isolated window station: {}",
            std::io::Error::last_os_error()
        );
        assert_ne!(SetProcessWindowStation(station), 0);
        let name: Vec<u16> = "QuickToolIsolatedClipboard\0".encode_utf16().collect();
        let desktop = CreateDesktopW(name.as_ptr(), null(), null_mut(), 0, 0x00000087, null());
        assert!(
            !desktop.is_null(),
            "isolated desktop: {}",
            std::io::Error::last_os_error()
        );
        assert_ne!(SetThreadDesktop(desktop), 0);
        struct Station {
            old_station: HWINSTA,
            old_desktop: HDESK,
            station: HWINSTA,
            desktop: HDESK,
        }
        impl Drop for Station {
            fn drop(&mut self) {
                unsafe {
                    SetProcessWindowStation(self.old_station);
                    SetThreadDesktop(self.old_desktop);
                    CloseDesktop(self.desktop);
                    CloseWindowStation(self.station);
                }
            }
        }
        let _guard = Station {
            old_station,
            old_desktop,
            station,
            desktop,
        };
        let text = "QuickTool 剪贴板 🦀\nUnicode";
        platform::copy_text(text).unwrap();
        assert_eq!(platform::clipboard_text().unwrap(), text);
        assert!(
            platform::clipboard_inspect()
                .unwrap()
                .iter()
                .any(|e| e.kind == "text" && e.value == text)
        );
        let rgba = [
            255, 0, 0, 255, 0, 255, 0, 128, 0, 0, 255, 255, 255, 255, 255, 0,
        ];
        platform::copy_image(2, 2, &rgba).unwrap();
        assert_eq!(platform::clipboard_image().unwrap(), (2, 2, rgba.to_vec()));
        assert!(
            platform::clipboard_inspect()
                .unwrap()
                .iter()
                .any(|e| e.kind == "image" && e.width == 2 && e.height == 2)
        );
        for (format, bytes, kind) in [
            (
                "image/svg+xml",
                b"<svg xmlns='http://www.w3.org/2000/svg'/>".as_slice(),
                "image",
            ),
            ("HTML Format", b"<b>QuickTool</b>\0".as_slice(), "html"),
            (
                "Rich Text Format",
                b"{\\rtf1 QuickTool}\0".as_slice(),
                "rtf",
            ),
            (
                "QuickTool Test Binary",
                [1u8, 0, 255, 2].as_slice(),
                "binary",
            ),
        ] {
            platform::copy_bytes(format, bytes).unwrap();
            let entries = platform::clipboard_inspect().unwrap();
            let entry = entries.iter().find(|e| e.format == format).unwrap();
            assert_eq!(entry.kind, kind);
            assert_eq!(entry.bytes, bytes);
            assert_eq!(entry.byte_length, bytes.len());
        }
        println!("isolated Text/Unicode/PNG/DIB/SVG/HTML/RTF/Binary round-trips passed");
    }
}
