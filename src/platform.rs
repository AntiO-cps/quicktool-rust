use crate::core::ResultItem;
use std::{path::PathBuf, sync::mpsc};

pub fn known_folders() -> Vec<(String, PathBuf)> {
    #[cfg(windows)]
    {
        native_extra::known_folders()
    }
    #[cfg(not(windows))]
    {
        let home = std::env::var_os("USERPROFILE")
            .map(PathBuf::from)
            .unwrap_or_default();
        ["Desktop", "Documents", "Downloads"]
            .map(|s| (s.to_string(), home.join(s)))
            .to_vec()
    }
}
pub fn app_paths() -> Vec<ResultItem> {
    crate::system::app_index()
        .into_iter()
        .map(|a| ResultItem::new(a.name, a.target))
        .collect()
}
pub struct Hotkey {
    pub events: mpsc::Receiver<()>,
    #[cfg(windows)]
    thread_id: u32,
    #[cfg(windows)]
    join: Option<std::thread::JoinHandle<()>>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PinEscapeStatus {
    pub hovered_id: Option<u64>,
    pub registered: bool,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct PinEscapeState {
    pins: Vec<(u64, isize)>,
    suspended: bool,
}
static PIN_ESCAPE_PAUSED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
/// Temporarily release hover-Escape while a native modal dialog is open.
pub struct PinEscapeSuspend;
impl PinEscapeSuspend {
    pub fn new() -> Self {
        PIN_ESCAPE_PAUSED.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Self
    }
}
impl Default for PinEscapeSuspend {
    fn default() -> Self {
        Self::new()
    }
}
impl Drop for PinEscapeSuspend {
    fn drop(&mut self) {
        PIN_ESCAPE_PAUSED.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }
}
/// Temporarily owns the Escape hotkey only while the pointer is over a listed
/// application-owned Pin window. Events identify the Pin that should close.
pub struct PinEscape {
    pub events: mpsc::Receiver<u64>,
    state: std::sync::Arc<std::sync::Mutex<PinEscapeState>>,
    status: std::sync::Arc<std::sync::Mutex<PinEscapeStatus>>,
    #[cfg(windows)]
    thread_id: u32,
    #[cfg(windows)]
    join: Option<std::thread::JoinHandle<()>>,
}
impl PinEscape {
    pub fn update(&mut self, pins: Vec<(u64, isize)>, suspended: bool) -> Result<(), String> {
        let next = PinEscapeState { pins, suspended };
        let mut state = self.state.lock().map_err(|_| "Pin Escape 状态不可用")?;
        if *state == next {
            return Ok(());
        }
        *state = next;
        drop(state);
        #[cfg(windows)]
        unsafe {
            if windows_sys::Win32::UI::WindowsAndMessaging::PostThreadMessageW(
                self.thread_id,
                0x8000 + 50,
                0,
                0,
            ) == 0
            {
                return Err("Pin Escape 线程已关闭".into());
            }
        }
        Ok(())
    }
    pub fn status(&self) -> PinEscapeStatus {
        self.status.lock().map(|s| s.clone()).unwrap_or_default()
    }
}
#[cfg(not(windows))]
impl PinEscape {
    pub fn new(_: impl Fn() + Send + 'static) -> Result<Self, String> {
        Err("此功能需要 Windows".into())
    }
}

#[cfg(windows)]
mod pin_escape_native {
    use super::*;
    use std::{
        ptr::null_mut,
        sync::{Arc, Mutex},
        time::{Duration, Instant},
    };
    use windows_sys::Win32::{
        Foundation::*,
        System::Threading::{GetCurrentProcessId, GetCurrentThreadId},
        UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
    };
    const ESCAPE_ID: i32 = 0x5154;
    const UPDATE: u32 = WM_APP + 50;
    fn pin_for_window(pins: &[(u64, isize)], window: HWND) -> Option<u64> {
        unsafe {
            if window.is_null() {
                return None;
            }
            let root = GetAncestor(window, GA_ROOT);
            if root.is_null() || IsWindowVisible(root) == 0 || IsIconic(root) != 0 {
                return None;
            }
            let mut pid = 0;
            GetWindowThreadProcessId(root, &mut pid);
            if pid != GetCurrentProcessId() {
                return None;
            }
            pins.iter()
                .rev()
                .find(|(_, hwnd)| *hwnd == root as isize)
                .map(|(id, _)| *id)
        }
    }
    fn hovered(state: &Arc<Mutex<PinEscapeState>>) -> Option<u64> {
        if PIN_ESCAPE_PAUSED.load(std::sync::atomic::Ordering::SeqCst) > 0 {
            return None;
        }
        let state = state.lock().ok()?;
        if state.suspended {
            return None;
        }
        unsafe {
            let mut point = POINT { x: 0, y: 0 };
            if GetCursorPos(&mut point) == 0 {
                return None;
            }
            pin_for_window(&state.pins, WindowFromPoint(point))
        }
    }
    fn forward_escape() {
        unsafe {
            let key = |flags| INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VK_ESCAPE,
                        wScan: 0,
                        dwFlags: flags,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            };
            let inputs = [key(0), key(KEYEVENTF_KEYUP)];
            SendInput(
                inputs.len() as u32,
                inputs.as_ptr(),
                std::mem::size_of::<INPUT>() as i32,
            );
        }
    }
    impl PinEscape {
        pub fn new(wake: impl Fn() + Send + 'static) -> Result<Self, String> {
            let state = Arc::new(Mutex::new(PinEscapeState::default()));
            let shared = state.clone();
            let status = Arc::new(Mutex::new(PinEscapeStatus::default()));
            let worker_status = status.clone();
            let (tx, events) = mpsc::channel();
            let (ready, wait) = mpsc::channel();
            let join = std::thread::spawn(move || unsafe {
                let tid = GetCurrentThreadId();
                let mut message: MSG = std::mem::zeroed();
                PeekMessageW(&mut message, null_mut(), 0, 0, PM_NOREMOVE);
                let timer = SetTimer(null_mut(), 0, 30, None);
                if timer == 0 {
                    let _ = ready.send(Err("Pin Escape 计时器创建失败".to_string()));
                    return;
                }
                let _ = ready.send(Ok(tid));
                let mut registered = false;
                let mut consumed = false;
                let mut last_attempt = Instant::now() - Duration::from_secs(1);
                while GetMessageW(&mut message, null_mut(), 0, 0) > 0 {
                    if message.message == WM_HOTKEY && message.wParam == ESCAPE_ID as usize {
                        if let Some(id) = hovered(&shared) {
                            consumed = true;
                            if tx.send(id).is_err() {
                                break;
                            }
                            wake();
                        } else {
                            if registered {
                                UnregisterHotKey(null_mut(), ESCAPE_ID);
                                registered = false;
                            }
                            consumed = false;
                            forward_escape();
                        }
                    }
                    if message.message == WM_TIMER
                        || message.message == UPDATE
                        || message.message == WM_HOTKEY
                    {
                        if message.message == UPDATE {
                            last_attempt = Instant::now() - Duration::from_secs(1);
                        }
                        let target = hovered(&shared);
                        let down = GetAsyncKeyState(VK_ESCAPE as i32) < 0;
                        if registered && target.is_none() && !(consumed && down) {
                            UnregisterHotKey(null_mut(), ESCAPE_ID);
                            registered = false;
                            consumed = false;
                        }
                        if !registered
                            && target.is_some()
                            && !down
                            && last_attempt.elapsed() >= Duration::from_millis(500)
                        {
                            last_attempt = Instant::now();
                            registered = RegisterHotKey(
                                null_mut(),
                                ESCAPE_ID,
                                MOD_NOREPEAT,
                                VK_ESCAPE as u32,
                            ) != 0;
                        }
                        if let Ok(mut status) = worker_status.lock() {
                            *status = PinEscapeStatus {
                                hovered_id: target,
                                registered,
                            };
                        }
                    } else {
                        TranslateMessage(&message);
                        DispatchMessageW(&message);
                    }
                }
                if registered {
                    UnregisterHotKey(null_mut(), ESCAPE_ID);
                }
                KillTimer(null_mut(), timer);
                if let Ok(mut status) = worker_status.lock() {
                    *status = PinEscapeStatus::default();
                }
            });
            Ok(Self {
                events,
                state,
                status,
                thread_id: wait.recv().map_err(|e| e.to_string())??,
                join: Some(join),
            })
        }
    }
    impl Drop for PinEscape {
        fn drop(&mut self) {
            if let Ok(mut state) = self.state.lock() {
                state.suspended = true;
                state.pins.clear();
            }
            unsafe {
                PostThreadMessageW(self.thread_id, WM_QUIT, 0, 0);
            }
            if let Some(join) = self.join.take() {
                let _ = join.join();
            }
        }
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn hover_policy_only_accepts_live_listed_windows() {
            unsafe {
                let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
                let title: Vec<u16> = "QuickTool hover policy test\0".encode_utf16().collect();
                let pin = CreateWindowExW(
                    WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                    class.as_ptr(),
                    title.as_ptr(),
                    WS_POPUP | WS_VISIBLE,
                    -32500,
                    -32500,
                    10,
                    10,
                    null_mut(),
                    null_mut(),
                    null_mut(),
                    std::ptr::null(),
                );
                let other = CreateWindowExW(
                    WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                    class.as_ptr(),
                    title.as_ptr(),
                    WS_POPUP | WS_VISIBLE,
                    -32520,
                    -32500,
                    10,
                    10,
                    null_mut(),
                    null_mut(),
                    null_mut(),
                    std::ptr::null(),
                );
                assert!(!pin.is_null() && !other.is_null());
                let targets = [(77, pin as isize)];
                assert_eq!(pin_for_window(&targets, pin), Some(77));
                assert_eq!(pin_for_window(&targets, other), None);
                assert_eq!(pin_for_window(&targets, null_mut()), None);
                ShowWindow(pin, SW_HIDE);
                assert_eq!(pin_for_window(&targets, pin), None);
                DestroyWindow(pin);
                DestroyWindow(other);
                assert_eq!(pin_for_window(&targets, pin), None);
            }
        }
    }
}
#[cfg(windows)]
mod windows {
    use super::*;
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::{
        System::{DataExchange::*, Memory::*, Threading::GetCurrentThreadId},
        UI::{Input::KeyboardAndMouse::*, Shell::ShellExecuteW, WindowsAndMessaging::*},
    };
    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }
    pub fn open(
        target: &str,
        parameters: Option<&str>,
        cwd: Option<&str>,
        admin: bool,
    ) -> Result<(), String> {
        if target.trim().is_empty()
            || target.contains('\0')
            || parameters.is_some_and(|s| s.contains('\0'))
            || cwd.is_some_and(|s| s.contains('\0'))
        {
            return Err("路径含无效字符".into());
        }
        let verb = wide(if admin { "runas" } else { "open" });
        let target = wide(target);
        let params = parameters.map(wide);
        let cwd = cwd.map(wide);
        let code = unsafe {
            ShellExecuteW(
                null_mut(),
                verb.as_ptr(),
                target.as_ptr(),
                params.as_ref().map_or(null(), |v| v.as_ptr()),
                cwd.as_ref().map_or(null(), |v| v.as_ptr()),
                SW_SHOWNORMAL,
            )
        } as isize;
        if code > 32 {
            Ok(())
        } else {
            Err(format!("打开失败（Windows 错误 {code}）"))
        }
    }
    pub fn clipboard_text() -> Result<String, String> {
        unsafe {
            if OpenClipboard(null_mut()) == 0 {
                return Err("剪贴板忙".into());
            }
            struct Guard;
            impl Drop for Guard {
                fn drop(&mut self) {
                    unsafe {
                        CloseClipboard();
                    }
                }
            }
            let _guard = Guard;
            let handle = GetClipboardData(13);
            if handle.is_null() {
                return Ok(String::new());
            }
            let size = GlobalSize(handle);
            if size > 192 * 1024 * 1024 {
                return Err("剪贴板数据过大".into());
            }
            let ptr = GlobalLock(handle) as *const u16;
            if ptr.is_null() {
                return Err("无法读取剪贴板".into());
            }
            let slice = std::slice::from_raw_parts(ptr, size / 2);
            let end = slice.iter().position(|c| *c == 0).unwrap_or(slice.len());
            let text = String::from_utf16_lossy(&slice[..end]);
            GlobalUnlock(handle);
            Ok(text)
        }
    }
    pub fn hotkey(shortcut: &str, wake: impl Fn() + Send + 'static) -> Result<Hotkey, String> {
        let (mods, key) = parse_shortcut(shortcut)?;
        let (tx, rx) = mpsc::channel();
        let (ready, wait) = mpsc::channel();
        let join = std::thread::spawn(move || unsafe {
            let tid = GetCurrentThreadId();
            let mut msg: MSG = std::mem::zeroed();
            PeekMessageW(&mut msg, null_mut(), 0, 0, PM_NOREMOVE);
            if RegisterHotKey(null_mut(), 1, mods | MOD_NOREPEAT, key) == 0 {
                let _ = ready.send(Err(format!(
                    "快捷键已被占用：{}",
                    std::io::Error::last_os_error()
                )));
                return;
            }
            let _ = ready.send(Ok(tid));
            while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
                if msg.message == WM_HOTKEY {
                    if tx.send(()).is_err() {
                        break;
                    }
                    wake();
                }
            }
            UnregisterHotKey(null_mut(), 1);
        });
        Ok(Hotkey {
            events: rx,
            thread_id: wait.recv().map_err(|e| e.to_string())??,
            join: Some(join),
        })
    }
    impl Drop for Hotkey {
        fn drop(&mut self) {
            unsafe {
                PostThreadMessageW(self.thread_id, WM_QUIT, 0, 0);
            }
            if let Some(join) = self.join.take() {
                let _ = join.join();
            }
        }
    }
    pub fn parse_shortcut(s: &str) -> Result<(u32, u32), String> {
        let mut mods = 0;
        let mut key = None;
        for token in s.split('+').map(str::trim) {
            let normalized = token.to_ascii_lowercase();
            if !matches!(
                normalized.as_str(),
                "alt" | "ctrl" | "control" | "shift" | "win" | "super" | "meta"
            ) && key.is_some()
            {
                return Err("快捷键只能包含一个主键".into());
            }
            match token.to_ascii_lowercase().as_str() {
                "alt" => mods |= MOD_ALT,
                "ctrl" | "control" => mods |= MOD_CONTROL,
                "shift" => mods |= MOD_SHIFT,
                "win" | "super" | "meta" => mods |= MOD_WIN,
                "space" => key = Some(VK_SPACE as u32),
                "tab" => key = Some(VK_TAB as u32),
                "escape" | "esc" => key = Some(VK_ESCAPE as u32),
                "enter" | "return" => key = Some(VK_RETURN as u32),
                "backspace" => key = Some(VK_BACK as u32),
                "delete" | "del" => key = Some(VK_DELETE as u32),
                "insert" => key = Some(VK_INSERT as u32),
                "home" => key = Some(VK_HOME as u32),
                "end" => key = Some(VK_END as u32),
                "pageup" => key = Some(VK_PRIOR as u32),
                "pagedown" => key = Some(VK_NEXT as u32),
                "left" => key = Some(VK_LEFT as u32),
                "right" => key = Some(VK_RIGHT as u32),
                "up" => key = Some(VK_UP as u32),
                "down" => key = Some(VK_DOWN as u32),
                "plus" => key = Some(VK_OEM_PLUS as u32),
                "minus" | "-" => key = Some(VK_OEM_MINUS as u32),
                s if s.len() == 1 && s.as_bytes()[0].is_ascii_alphanumeric() => {
                    key = Some(s.as_bytes()[0].to_ascii_uppercase() as u32)
                }
                s if s.starts_with('f') => {
                    let n: u32 = s[1..].parse().map_err(|_| "无效快捷键")?;
                    if !(1..=24).contains(&n) {
                        return Err("无效快捷键".into());
                    }
                    key = Some(VK_F1 as u32 + n - 1)
                }
                _ => return Err("无效快捷键".into()),
            }
        }
        Ok((mods, key.ok_or("快捷键缺少主键")?))
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn shortcut_validation() {
            assert!(parse_shortcut("Ctrl+A+B").is_err());
            assert!(parse_shortcut("Ctrl+").is_err());
            assert!(parse_shortcut("Alt+F25").is_err());
            assert!(parse_shortcut("Ctrl+PageDown").is_ok());
            assert_eq!(
                parse_shortcut("Alt+Space").unwrap(),
                (MOD_ALT, VK_SPACE as u32)
            );
        }
    }
}
#[cfg(windows)]
pub use windows::{clipboard_text, hotkey, open, parse_shortcut};
#[cfg(not(windows))]
pub fn open(_: &str, _: Option<&str>, _: Option<&str>, _: bool) -> Result<(), String> {
    Err("此功能需要 Windows".into())
}
#[cfg(not(windows))]
pub fn clipboard_text() -> Result<String, String> {
    Err("此功能需要 Windows".into())
}
#[cfg(not(windows))]
pub fn hotkey(_: &str, _: impl Fn() + Send + 'static) -> Result<Hotkey, String> {
    Err("此功能需要 Windows".into())
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ClipboardEntry {
    pub format: String,
    pub kind: String,
    pub value: String,
    pub preview: String,
    pub bytes: Vec<u8>,
    pub byte_length: usize,
    pub width: u32,
    pub height: u32,
}
#[cfg(windows)]
pub use native_extra::{
    apply_window_style, clipboard_image, clipboard_inspect, configure_glow, configure_popup,
    configure_tooltip, copy_bytes, copy_image, copy_text, find_window, place_companion_window,
    save_file_dialog,
};
#[cfg(not(windows))]
pub fn copy_text(_: &str) -> Result<(), String> {
    Err("此功能需要 Windows".into())
}
#[cfg(not(windows))]
pub fn clipboard_inspect() -> Result<Vec<ClipboardEntry>, String> {
    Err("此功能需要 Windows".into())
}
#[cfg(not(windows))]
pub fn copy_image(_: u32, _: u32, _: &[u8]) -> Result<(), String> {
    Err("此功能需要 Windows".into())
}
#[cfg(not(windows))]
pub fn clipboard_image() -> Result<(u32, u32, Vec<u8>), String> {
    Err("此功能需要 Windows".into())
}

#[cfg(windows)]
mod native_extra {
    use super::*;
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::{
        Foundation::*,
        Graphics::{Dwm::*, Gdi::*},
        System::{Com::CoTaskMemFree, DataExchange::*, Memory::*},
        UI::{Controls::Dialogs::*, Shell::*, WindowsAndMessaging::*},
    };
    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }
    pub fn find_window(title: &str) -> Option<isize> {
        if title.contains('\0') {
            return None;
        }
        let hwnd = unsafe { FindWindowW(null(), wide(title).as_ptr()) };
        (!hwnd.is_null()).then_some(hwnd as isize)
    }
    /// Nonactivating owned popup; preserve the viewport's supplied rectangle.
    pub fn configure_popup(
        title: &str,
        owner: isize,
        visible: bool,
        topmost: bool,
    ) -> Result<(), String> {
        configure_owned_popup(title, owner, visible, topmost, false)
    }
    /// Click-through nonactivating tooltip above its owner; preserve supplied bounds.
    pub fn configure_tooltip(
        title: &str,
        owner: isize,
        visible: bool,
        topmost: bool,
    ) -> Result<(), String> {
        configure_owned_popup(title, owner, visible, topmost, true)
    }
    fn configure_owned_popup(
        title: &str,
        owner: isize,
        visible: bool,
        topmost: bool,
        click_through: bool,
    ) -> Result<(), String> {
        unsafe {
            let popup = find_window(title).ok_or("弹出窗口尚未创建")? as HWND;
            let owner = owner as HWND;
            let (mut a, mut b) = (0, 0);
            GetWindowThreadProcessId(owner, &mut a);
            GetWindowThreadProcessId(popup, &mut b);
            if a != windows_sys::Win32::System::Threading::GetCurrentProcessId()
                || a != b
                || owner == popup
            {
                return Err("弹出窗口必须属于当前应用".into());
            }
            let style = GetWindowLongPtrW(popup, GWL_EXSTYLE) as u32;
            let desired = (style | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW)
                & !(WS_EX_APPWINDOW | WS_EX_TRANSPARENT);
            let desired = if click_through {
                desired | WS_EX_TRANSPARENT
            } else {
                desired
            };
            if style != desired {
                SetWindowLongPtrW(popup, GWL_EXSTYLE, desired as isize);
            }
            SetWindowLongPtrW(popup, GWLP_HWNDPARENT, owner as isize);
            if !visible || IsWindowVisible(owner) == 0 || IsIconic(owner) != 0 {
                ShowWindow(popup, SW_HIDE);
                return Ok(());
            }
            if SetWindowPos(
                popup,
                if topmost {
                    HWND_TOPMOST
                } else {
                    HWND_NOTOPMOST
                },
                0,
                0,
                0,
                0,
                SWP_NOACTIVATE | SWP_NOSIZE | SWP_NOMOVE | SWP_SHOWWINDOW | SWP_NOOWNERZORDER,
            ) == 0
            {
                return Err(std::io::Error::last_os_error().to_string());
            }
            Ok(())
        }
    }
    /// Set passthrough/z-order for the Glow viewport without changing its bounds.
    pub fn configure_glow(
        title: &str,
        owner: isize,
        visible: bool,
        topmost: bool,
    ) -> Result<(), String> {
        unsafe {
            use windows_sys::Win32::System::Threading::GetCurrentProcessId;
            let glow = find_window(title).ok_or("Glow窗口尚未创建")? as HWND;
            let owner = owner as HWND;
            let (mut a, mut b) = (0, 0);
            GetWindowThreadProcessId(owner, &mut a);
            GetWindowThreadProcessId(glow, &mut b);
            if a != GetCurrentProcessId() || b != a || owner == glow {
                return Err("Glow窗口必须属于当前应用".into());
            }
            let style = GetWindowLongPtrW(glow, GWL_EXSTYLE) as u32;
            let desired = (style | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TRANSPARENT)
                & !WS_EX_APPWINDOW;
            if style != desired {
                SetWindowLongPtrW(glow, GWL_EXSTYLE, desired as isize);
            }
            if !visible || IsWindowVisible(owner) == 0 || IsIconic(owner) != 0 {
                if IsWindowVisible(glow) != 0 {
                    ShowWindow(glow, SW_HIDE);
                }
                return Ok(());
            }
            if (style & WS_EX_TOPMOST != 0) != topmost {
                SetWindowPos(
                    glow,
                    if topmost {
                        HWND_TOPMOST
                    } else {
                        HWND_NOTOPMOST
                    },
                    0,
                    0,
                    0,
                    0,
                    SWP_NOACTIVATE | SWP_NOSIZE | SWP_NOMOVE | SWP_NOSENDCHANGING,
                );
            }
            if SetWindowPos(
                glow,
                owner,
                0,
                0,
                0,
                0,
                SWP_NOACTIVATE
                    | SWP_NOSIZE
                    | SWP_NOMOVE
                    | SWP_SHOWWINDOW
                    | SWP_NOSENDCHANGING
                    | SWP_NOOWNERZORDER,
            ) == 0
            {
                return Err(std::io::Error::last_os_error().to_string());
            }
            Ok(())
        }
    }
    /// Place a transparent application-owned companion behind its owner.
    /// Padding is in physical pixels. Click-through companions never activate.
    pub fn place_companion_window(
        owner: isize,
        companion: isize,
        padding: i32,
        visible: bool,
        topmost: bool,
        click_through: bool,
    ) -> Result<(), String> {
        unsafe {
            use windows_sys::Win32::System::Threading::GetCurrentProcessId;
            let (owner, companion) = (owner as HWND, companion as HWND);
            let mut owner_pid = 0;
            let mut companion_pid = 0;
            GetWindowThreadProcessId(owner, &mut owner_pid);
            GetWindowThreadProcessId(companion, &mut companion_pid);
            let pid = GetCurrentProcessId();
            if owner_pid != pid || companion_pid != pid || owner == companion {
                return Err("附属窗口必须属于当前应用".into());
            }
            let style = GetWindowLongPtrW(companion, GWL_EXSTYLE) as u32;
            let desired = (style | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE) & !WS_EX_APPWINDOW;
            let desired = if click_through {
                desired | WS_EX_TRANSPARENT
            } else {
                desired & !WS_EX_TRANSPARENT
            };
            if desired != style {
                SetWindowLongPtrW(companion, GWL_EXSTYLE, desired as isize);
            }
            if !visible || IsWindowVisible(owner) == 0 || IsIconic(owner) != 0 {
                ShowWindow(companion, SW_HIDE);
                return Ok(());
            }
            let mut client: RECT = std::mem::zeroed();
            let mut point = POINT { x: 0, y: 0 };
            if GetClientRect(owner, &mut client) == 0 || ClientToScreen(owner, &mut point) == 0 {
                return Err("无法读取主窗口边界".into());
            }
            let padding = padding.clamp(0, 2048);
            let (x, y, w, h) = (
                point.x - padding,
                point.y - padding,
                client.right + padding * 2,
                client.bottom + padding * 2,
            );
            let mut old: RECT = std::mem::zeroed();
            GetWindowRect(companion, &mut old);
            let is_top = style & WS_EX_TOPMOST != 0;
            let changed = old.left != x
                || old.top != y
                || old.right - old.left != w
                || old.bottom - old.top != h
                || IsWindowVisible(companion) == 0
                || is_top != topmost;
            if changed {
                if is_top != topmost {
                    SetWindowPos(
                        companion,
                        if topmost {
                            HWND_TOPMOST
                        } else {
                            HWND_NOTOPMOST
                        },
                        0,
                        0,
                        0,
                        0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOSENDCHANGING,
                    );
                }
                if SetWindowPos(
                    companion,
                    owner,
                    x,
                    y,
                    w,
                    h,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW | SWP_NOSENDCHANGING | SWP_NOOWNERZORDER,
                ) == 0
                {
                    return Err(std::io::Error::last_os_error().to_string());
                }
            }
            Ok(())
        }
    }
    pub fn known_folders() -> Vec<(String, PathBuf)> {
        let mut out = Vec::new();
        for (name, id) in [
            ("Home", FOLDERID_Profile),
            ("Desktop", FOLDERID_Desktop),
            ("Documents", FOLDERID_Documents),
            ("Downloads", FOLDERID_Downloads),
            ("Pictures", FOLDERID_Pictures),
            ("Music", FOLDERID_Music),
            ("Videos", FOLDERID_Videos),
            ("PublicDesktop", FOLDERID_PublicDesktop),
            ("Programs", FOLDERID_Programs),
            ("CommonPrograms", FOLDERID_CommonPrograms),
        ] {
            let mut p = null_mut();
            unsafe {
                if SHGetKnownFolderPath(&id, 0, null_mut(), &mut p) >= 0 {
                    let mut n = 0;
                    while *p.add(n) != 0 {
                        n += 1
                    }
                    out.push((
                        name.into(),
                        PathBuf::from(String::from_utf16_lossy(std::slice::from_raw_parts(p, n))),
                    ));
                    CoTaskMemFree(p as _);
                }
            }
        }
        out.push(("Temp".into(), std::env::temp_dir()));
        out
    }
    struct Clipboard(HWND);
    impl Clipboard {
        fn open() -> Result<Self, String> {
            let hwnd = unsafe {
                CreateWindowExW(
                    0,
                    wide("STATIC").as_ptr(),
                    wide("QuickTool Clipboard").as_ptr(),
                    0,
                    0,
                    0,
                    0,
                    0,
                    HWND_MESSAGE,
                    null_mut(),
                    null_mut(),
                    null(),
                )
            };
            if hwnd.is_null() {
                return Err("剪贴板所有者窗口创建失败".into());
            }
            for _ in 0..10 {
                if unsafe { OpenClipboard(hwnd) } != 0 {
                    return Ok(Self(hwnd));
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            unsafe {
                DestroyWindow(hwnd);
            }
            Err("剪贴板忙".into())
        }
    }
    impl Drop for Clipboard {
        fn drop(&mut self) {
            unsafe {
                CloseClipboard();
                DestroyWindow(self.0);
            }
        }
    }
    unsafe fn read(format: u32) -> Result<Vec<u8>, String> {
        unsafe {
            let h = GetClipboardData(format);
            if h.is_null() {
                return Err("剪贴板格式不可用".into());
            }
            let size = GlobalSize(h);
            if size > 192 * 1024 * 1024 {
                return Err("剪贴板数据过大".into());
            }
            let p = GlobalLock(h);
            if p.is_null() {
                return Err("无法读取剪贴板".into());
            }
            let data = std::slice::from_raw_parts(p as *const u8, size).to_vec();
            GlobalUnlock(h);
            Ok(data)
        }
    }
    unsafe fn set(format: u32, bytes: &[u8]) -> Result<(), String> {
        unsafe {
            let h = GlobalAlloc(GMEM_MOVEABLE | GMEM_ZEROINIT, bytes.len().max(1));
            if h.is_null() {
                return Err("内存分配失败".into());
            }
            let p = GlobalLock(h);
            if p.is_null() {
                GlobalFree(h);
                return Err("内存锁定失败".into());
            }
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), p as *mut u8, bytes.len());
            GlobalUnlock(h);
            if SetClipboardData(format, h).is_null() {
                GlobalFree(h);
                return Err("剪贴板写入失败".into());
            }
            Ok(())
        }
    }
    pub fn copy_text(text: &str) -> Result<(), String> {
        let _guard = Clipboard::open()?;
        unsafe {
            if EmptyClipboard() == 0 {
                return Err("无法清空剪贴板".into());
            }
            set(
                13,
                &wide(text)
                    .into_iter()
                    .flat_map(u16::to_le_bytes)
                    .collect::<Vec<_>>(),
            )
        }
    }
    pub fn copy_bytes(format: &str, bytes: &[u8]) -> Result<(), String> {
        if format.contains('\0') {
            return Err("格式无效".into());
        }
        let _guard = Clipboard::open()?;
        unsafe {
            let id = RegisterClipboardFormatW(wide(format).as_ptr());
            if id == 0 || EmptyClipboard() == 0 {
                return Err("剪贴板格式注册失败".into());
            }
            set(id, bytes)
        }
    }
    pub fn copy_image(width: u32, height: u32, rgba: &[u8]) -> Result<(), String> {
        if width == 0
            || height == 0
            || width > 32768
            || height > 32768
            || rgba.len() as u64 != u64::from(width) * u64::from(height) * 4
            || rgba.len() > 192 * 1024 * 1024
        {
            return Err("图片尺寸无效".into());
        }
        let mut dib = vec![0u8; 124 + rgba.len()];
        dib[0..4].copy_from_slice(&124u32.to_le_bytes());
        dib[4..8].copy_from_slice(&width.to_le_bytes());
        dib[8..12].copy_from_slice(&(-(height as i32)).to_le_bytes());
        dib[12..14].copy_from_slice(&1u16.to_le_bytes());
        dib[14..16].copy_from_slice(&32u16.to_le_bytes());
        dib[16..20].copy_from_slice(&3u32.to_le_bytes());
        dib[20..24].copy_from_slice(&(rgba.len() as u32).to_le_bytes());
        for (offset, value) in [
            (40, 0x00ff0000u32),
            (44, 0x0000ff00),
            (48, 0x000000ff),
            (52, 0xff000000),
            (56, 0x73524742),
        ] {
            dib[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        for (src, dst) in rgba.chunks_exact(4).zip(dib[124..].chunks_exact_mut(4)) {
            dst.copy_from_slice(&[src[2], src[1], src[0], src[3]]);
        }
        let mut png = std::io::Cursor::new(Vec::new());
        image::write_buffer_with_format(
            &mut png,
            rgba,
            width,
            height,
            image::ColorType::Rgba8,
            image::ImageFormat::Png,
        )
        .map_err(|e| e.to_string())?;
        let _guard = Clipboard::open()?;
        unsafe {
            if EmptyClipboard() == 0 {
                return Err("无法清空剪贴板".into());
            }
            set(17, &dib)?;
            let format = RegisterClipboardFormatW(wide("PNG").as_ptr());
            if format != 0 {
                set(format, png.get_ref())?;
            }
            Ok(())
        }
    }
    pub fn clipboard_image() -> Result<(u32, u32, Vec<u8>), String> {
        let _guard = Clipboard::open()?;
        unsafe {
            let format = RegisterClipboardFormatW(wide("PNG").as_ptr());
            if let Ok(bytes) = read(format) {
                if let Ok(image) = image::load_from_memory(&bytes) {
                    let i = image.to_rgba8();
                    return Ok((i.width(), i.height(), i.into_raw()));
                }
            }
            let bytes = read(17).or_else(|_| read(8))?;
            decode_dib(&bytes)
        }
    }
    fn decode_dib(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
        let u32at = |n| {
            bytes
                .get(n..n + 4)
                .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        };
        let header = u32at(0).ok_or("DIB数据过短")? as usize;
        if header < 40 || header > bytes.len() {
            return Err("DIB头无效".into());
        }
        let width = u32at(4).unwrap() as i32;
        let height = u32at(8).unwrap() as i32;
        let bits = u16::from_le_bytes(bytes[14..16].try_into().unwrap());
        let compression = u32at(16).unwrap();
        if width <= 0 || height == 0 || width > 32768 || height.unsigned_abs() > 32768 {
            return Err("图片格式暂不支持".into());
        }
        if ![24, 32].contains(&bits) || ![0, 3].contains(&compression) {
            return decode_palette_dib(
                bytes,
                width as u32,
                height.unsigned_abs(),
                header,
                bits,
                compression,
            );
        }
        let (w, h) = (width as usize, height.unsigned_abs() as usize);
        let stride = (w * bits as usize).div_ceil(32) * 4;
        let offset = header
            + if header == 40 && compression == 3 {
                12
            } else {
                0
            };
        if stride
            .checked_mul(h)
            .and_then(|n| n.checked_add(offset))
            .is_none_or(|n| n > bytes.len())
            || w * h * 4 > 192 * 1024 * 1024
        {
            return Err("DIB数据无效".into());
        }
        let mut out = vec![0u8; w * h * 4];
        for y in 0..h {
            let src = offset + if height > 0 { h - 1 - y } else { y } * stride;
            for x in 0..w {
                let p = src + x * (bits as usize / 8);
                let d = (y * w + x) * 4;
                out[d..d + 4].copy_from_slice(&[
                    bytes[p + 2],
                    bytes[p + 1],
                    bytes[p],
                    if bits == 32 && header >= 108 {
                        bytes[p + 3]
                    } else {
                        255
                    },
                ]);
            }
        }
        Ok((w as u32, h as u32, out))
    }
    fn decode_palette_dib(
        bytes: &[u8],
        width: u32,
        height: u32,
        header: usize,
        bits: u16,
        compression: u32,
    ) -> Result<(u32, u32, Vec<u8>), String> {
        if u64::from(width) * u64::from(height) * 4 > 192 * 1024 * 1024 {
            return Err("图片数据过大".into());
        }
        let colors = u32::from_le_bytes(bytes[32..36].try_into().unwrap());
        let colors = if colors == 0 && bits <= 8 {
            1u32 << bits
        } else {
            colors
        };
        if colors > 256 {
            return Err("DIB调色板无效".into());
        }
        let offset = 14
            + header
            + colors as usize * 4
            + if header == 40 && compression == 3 {
                12
            } else if header == 40 && compression == 6 {
                16
            } else {
                0
            };
        if offset > bytes.len() + 14 {
            return Err("DIB数据无效".into());
        }
        let mut bmp = vec![0u8; 14];
        bmp[..2].copy_from_slice(b"BM");
        bmp[2..6].copy_from_slice(&((bytes.len() + 14) as u32).to_le_bytes());
        bmp[10..14].copy_from_slice(&(offset as u32).to_le_bytes());
        bmp.extend_from_slice(bytes);
        let image = image::load_from_memory_with_format(&bmp, image::ImageFormat::Bmp)
            .map_err(|e| e.to_string())?
            .to_rgba8();
        Ok((image.width(), image.height(), image.into_raw()))
    }
    pub fn clipboard_inspect() -> Result<Vec<ClipboardEntry>, String> {
        let _guard = Clipboard::open()?;
        let mut out = Vec::new();
        unsafe {
            let mut format = 0;
            loop {
                format = EnumClipboardFormats(format);
                if format == 0 {
                    break;
                }
                let mut name = [0u16; 256];
                let n = GetClipboardFormatNameW(format, name.as_mut_ptr(), name.len() as i32);
                let label = if n > 0 {
                    String::from_utf16_lossy(&name[..n as usize])
                } else {
                    match format {
                        13 => "CF_UNICODETEXT".into(),
                        8 => "CF_DIB".into(),
                        17 => "CF_DIBV5".into(),
                        2 => "CF_BITMAP".into(),
                        _ => format!("CF_{format}"),
                    }
                };
                let Ok(mut bytes) = read(format) else {
                    continue;
                };
                let byte_length = bytes.len();
                let lower = label.to_lowercase();
                let kind = if [1, 7, 13].contains(&format) {
                    "text"
                } else if lower.contains("html") {
                    "html"
                } else if lower.contains("rtf") || lower.contains("rich text") {
                    "rtf"
                } else if [8, 17, 2].contains(&format) || lower.contains("image") || lower == "png"
                {
                    "image"
                } else {
                    "binary"
                };
                let value = if [1, 7, 13].contains(&format) {
                    let unicode = if format == 13 {
                        bytes.clone()
                    } else {
                        read(13).unwrap_or_default()
                    };
                    String::from_utf16_lossy(
                        &unicode
                            .chunks_exact(2)
                            .map(|s| u16::from_le_bytes([s[0], s[1]]))
                            .take_while(|c| *c != 0)
                            .collect::<Vec<_>>(),
                    )
                } else if ["html", "rtf"].contains(&kind) {
                    String::from_utf8_lossy(&bytes)
                        .trim_end_matches('\0')
                        .into()
                } else {
                    String::new()
                };
                if kind == "binary" && bytes.len() > 1024 * 1024 {
                    bytes.clear();
                    bytes.shrink_to_fit();
                }
                let (width, height) = if [8, 17].contains(&format) {
                    decode_dib(&bytes)
                        .map(|(w, h, _)| (w, h))
                        .unwrap_or_default()
                } else if kind == "image" {
                    image::load_from_memory(&bytes)
                        .map(|i| (i.width(), i.height()))
                        .unwrap_or_default()
                } else {
                    (0, 0)
                };
                out.push(ClipboardEntry {
                    format: label,
                    kind: kind.into(),
                    preview: value.chars().take(4000).collect(),
                    value,
                    bytes,
                    byte_length,
                    width,
                    height,
                });
            }
        }
        Ok(out)
    }
    pub fn save_file_dialog(extension: &str) -> Result<Option<PathBuf>, String> {
        if extension.is_empty()
            || extension.len() > 16
            || !extension.chars().all(|c| c.is_ascii_alphanumeric())
        {
            return Err("扩展名无效".into());
        }
        let _escape_pause = PinEscapeSuspend::new();
        let ext = wide(extension);
        let filter = wide(&format!(
            "{} file\0*.{}\0All files\0*.*\0",
            extension.to_uppercase(),
            extension
        ));
        let mut file = vec![0u16; 32768];
        let initial = wide(&format!("QuickTool.{}", extension));
        file[..initial.len()].copy_from_slice(&initial);
        let mut dialog: OPENFILENAMEW = unsafe { std::mem::zeroed() };
        dialog.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
        dialog.lpstrFilter = filter.as_ptr();
        dialog.lpstrFile = file.as_mut_ptr();
        dialog.nMaxFile = file.len() as u32;
        dialog.lpstrDefExt = ext.as_ptr();
        dialog.Flags = OFN_OVERWRITEPROMPT | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR;
        unsafe {
            if GetSaveFileNameW(&mut dialog) != 0 {
                let n = file.iter().position(|c| *c == 0).unwrap_or(file.len());
                Ok(Some(PathBuf::from(String::from_utf16_lossy(&file[..n]))))
            } else {
                let code = CommDlgExtendedError();
                if code == 0 {
                    Ok(None)
                } else {
                    Err(format!("保存对话框错误 {code}"))
                }
            }
        }
    }
    /// Apply DWM acrylic and rounded clipping to the native client window.
    ///
    /// # Safety
    /// `hwnd` must refer to a live window owned by this process. The caller must
    /// keep the window alive for this call and obey its UI thread requirements.
    pub unsafe fn apply_window_style(
        hwnd: isize,
        dark: bool,
        blur: u32,
        radius: i32,
    ) -> Result<(), String> {
        unsafe {
            let hwnd = hwnd as HWND;
            let dark = dark as i32;
            let corner = if radius > 0 { 2i32 } else { 1 };
            let material = if blur == 0 { 1i32 } else { 3 };
            let _ = DwmSetWindowAttribute(hwnd, 20, &dark as *const _ as _, 4);
            let _ = DwmSetWindowAttribute(hwnd, 33, &corner as *const _ as _, 4);
            let result = DwmSetWindowAttribute(hwnd, 38, &material as *const _ as _, 4);
            let mut rect: RECT = std::mem::zeroed();
            if GetClientRect(hwnd, &mut rect) != 0 && radius > 0 {
                let region = CreateRoundRectRgn(
                    0,
                    0,
                    rect.right + 1,
                    rect.bottom + 1,
                    radius * 2,
                    radius * 2,
                );
                if !region.is_null() && SetWindowRgn(hwnd, region, 1) == 0 {
                    DeleteObject(region);
                }
            }
            if result < 0 {
                return Err(format!("DWM材质不可用 0x{result:08x}"));
            }
            Ok(())
        }
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn rejects_truncated_dib() {
            assert!(decode_dib(&[0; 39]).is_err())
        }
        #[test]
        fn reads_bottom_up_bgr() {
            let mut b = vec![0; 44];
            b[..4].copy_from_slice(&40u32.to_le_bytes());
            b[4..8].copy_from_slice(&1u32.to_le_bytes());
            b[8..12].copy_from_slice(&1u32.to_le_bytes());
            b[14..16].copy_from_slice(&24u16.to_le_bytes());
            b[40..43].copy_from_slice(&[3, 2, 1]);
            assert_eq!(decode_dib(&b).unwrap(), (1, 1, vec![1, 2, 3, 255]));
        }
        #[test]
        fn reads_palette_dib() {
            let mut b = vec![0u8; 52];
            b[..4].copy_from_slice(&40u32.to_le_bytes());
            b[4..8].copy_from_slice(&2u32.to_le_bytes());
            b[8..12].copy_from_slice(&1u32.to_le_bytes());
            b[12..14].copy_from_slice(&1u16.to_le_bytes());
            b[14..16].copy_from_slice(&8u16.to_le_bytes());
            b[32..36].copy_from_slice(&2u32.to_le_bytes());
            b[40..48].copy_from_slice(&[0, 0, 255, 0, 0, 255, 0, 0]);
            b[48..50].copy_from_slice(&[0, 1]);
            assert_eq!(
                decode_dib(&b).unwrap(),
                (2, 1, vec![255, 0, 0, 255, 0, 255, 0, 255])
            );
        }
    }
}
