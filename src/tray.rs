//! Native notification area icon with summon/exit actions.
use std::sync::mpsc;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayEvent {
    Show,
    Quit,
}
pub struct Tray {
    pub events: mpsc::Receiver<TrayEvent>,
    #[cfg(windows)]
    thread_id: u32,
    #[cfg(windows)]
    join: Option<std::thread::JoinHandle<()>>,
}
#[cfg(windows)]
mod native {
    use super::*;
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::{LibraryLoader::GetModuleHandleW, Threading::GetCurrentThreadId},
        UI::{Shell::*, WindowsAndMessaging::*},
    };
    struct State {
        tx: mpsc::Sender<TrayEvent>,
        wake: Box<dyn Fn() + Send>,
        taskbar_created: u32,
        icon: Option<HICON>,
    }
    impl Drop for State {
        fn drop(&mut self) {
            if let Some(icon) = self.icon.take() {
                unsafe {
                    DestroyIcon(icon);
                }
            }
        }
    }
    fn create_icon() -> Option<HICON> {
        unsafe {
            let image = image::load_from_memory(include_bytes!("../assets/assets/tray.png"))
                .ok()?
                .resize_exact(32, 32, image::imageops::FilterType::Lanczos3)
                .to_rgba8();
            let dc = CreateCompatibleDC(null_mut());
            if dc.is_null() {
                return None;
            }
            let mut bmi: BITMAPINFO = std::mem::zeroed();
            bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            bmi.bmiHeader.biWidth = 32;
            bmi.bmiHeader.biHeight = -32;
            bmi.bmiHeader.biPlanes = 1;
            bmi.bmiHeader.biBitCount = 32;
            let mut bits = null_mut();
            let color = CreateDIBSection(dc, &bmi, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
            DeleteDC(dc);
            if color.is_null() {
                return None;
            }
            let pixels = std::slice::from_raw_parts_mut(bits as *mut u8, 4096);
            for (src, dst) in image
                .as_raw()
                .chunks_exact(4)
                .zip(pixels.chunks_exact_mut(4))
            {
                let a = src[3] as u32;
                dst.copy_from_slice(&[
                    (src[2] as u32 * a / 255) as u8,
                    (src[1] as u32 * a / 255) as u8,
                    (src[0] as u32 * a / 255) as u8,
                    src[3],
                ]);
            }
            let mask = CreateBitmap(32, 32, 1, 1, null());
            if mask.is_null() {
                DeleteObject(color);
                return None;
            }
            let icon_info = ICONINFO {
                fIcon: 1,
                xHotspot: 0,
                yHotspot: 0,
                hbmMask: mask,
                hbmColor: color,
            };
            let icon = CreateIconIndirect(&icon_info);
            DeleteObject(mask);
            DeleteObject(color);
            (!icon.is_null()).then_some(icon)
        }
    }
    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }
    unsafe fn notify(hwnd: HWND, op: u32) {
        unsafe {
            let mut data: NOTIFYICONDATAW = std::mem::zeroed();
            data.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            data.hWnd = hwnd;
            data.uID = 1;
            data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
            data.uCallbackMessage = WM_APP + 1;
            let state = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const State;
            data.hIcon = if state.is_null() {
                LoadIconW(null_mut(), IDI_APPLICATION)
            } else {
                (*state)
                    .icon
                    .unwrap_or_else(|| LoadIconW(null_mut(), IDI_APPLICATION))
            };
            let tip = wide("QuickTool");
            data.szTip[..tip.len()].copy_from_slice(&tip);
            Shell_NotifyIconW(op, &data);
        }
    }
    unsafe extern "system" fn proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
        unsafe {
            if msg == WM_NCCREATE {
                let cs = &*(l as *const CREATESTRUCTW);
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, cs.lpCreateParams as isize);
            }
            let state = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State;
            if !state.is_null() {
                let state = &*state;
                if msg == state.taskbar_created {
                    notify(hwnd, NIM_ADD);
                    return 0;
                }
                if msg == WM_APP + 1 {
                    let event = l as u32;
                    if [WM_LBUTTONUP, WM_LBUTTONDBLCLK].contains(&event) {
                        let _ = state.tx.send(TrayEvent::Show);
                        (state.wake)();
                        return 0;
                    }
                    if event == WM_RBUTTONUP {
                        let menu = CreatePopupMenu();
                        AppendMenuW(menu, MF_STRING, 1, wide("打开 QuickTool").as_ptr());
                        AppendMenuW(menu, MF_SEPARATOR, 0, null());
                        AppendMenuW(menu, MF_STRING, 2, wide("退出").as_ptr());
                        let mut point = POINT { x: 0, y: 0 };
                        GetCursorPos(&mut point);
                        SetForegroundWindow(hwnd);
                        let command = TrackPopupMenu(
                            menu,
                            TPM_RETURNCMD | TPM_NONOTIFY,
                            point.x,
                            point.y,
                            0,
                            hwnd,
                            null(),
                        );
                        DestroyMenu(menu);
                        if command == 1 || command == 2 {
                            let _ = state.tx.send(if command == 1 {
                                TrayEvent::Show
                            } else {
                                TrayEvent::Quit
                            });
                            (state.wake)();
                        }
                        return 0;
                    }
                }
            }
            if msg == WM_CLOSE {
                DestroyWindow(hwnd);
                return 0;
            }
            if msg == WM_DESTROY {
                notify(hwnd, NIM_DELETE);
                PostQuitMessage(0);
                return 0;
            }
            DefWindowProcW(hwnd, msg, w, l)
        }
    }
    impl Tray {
        pub fn new(wake: impl Fn() + Send + 'static) -> Result<Self, String> {
            let (tx, events) = mpsc::channel();
            let (ready, wait) = mpsc::channel();
            let join = std::thread::spawn(move || unsafe {
                let thread_id = GetCurrentThreadId();
                let instance = GetModuleHandleW(null());
                let class = wide(&format!("QuickToolRustTray{thread_id}"));
                let mut wc: WNDCLASSW = std::mem::zeroed();
                wc.lpfnWndProc = Some(proc);
                wc.hInstance = instance;
                wc.lpszClassName = class.as_ptr();
                if RegisterClassW(&wc) == 0 {
                    let _ = ready.send(Err("托盘窗口类注册失败".to_string()));
                    return;
                }
                let mut state = Box::new(State {
                    tx,
                    wake: Box::new(wake),
                    taskbar_created: RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()),
                    icon: create_icon(),
                });
                let hwnd = CreateWindowExW(
                    0,
                    class.as_ptr(),
                    wide("QuickTool").as_ptr(),
                    0,
                    0,
                    0,
                    0,
                    0,
                    null_mut(),
                    null_mut(),
                    instance,
                    &mut *state as *mut State as _,
                );
                if hwnd.is_null() {
                    let _ = ready.send(Err("托盘窗口创建失败".to_string()));
                    UnregisterClassW(class.as_ptr(), instance);
                    return;
                }
                notify(hwnd, NIM_ADD);
                let _ = ready.send(Ok(thread_id));
                let mut msg: MSG = std::mem::zeroed();
                while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
                if IsWindow(hwnd) != 0 {
                    DestroyWindow(hwnd);
                }
                UnregisterClassW(class.as_ptr(), instance);
            });
            Ok(Self {
                events,
                thread_id: wait.recv().map_err(|e| e.to_string())??,
                join: Some(join),
            })
        }
    }
    impl Drop for Tray {
        fn drop(&mut self) {
            unsafe {
                PostThreadMessageW(self.thread_id, WM_QUIT, 0, 0);
            }
            if let Some(join) = self.join.take() {
                let _ = join.join();
            }
        }
    }
}
#[cfg(not(windows))]
impl Tray {
    pub fn new(_: impl Fn() + Send + 'static) -> Result<Self, String> {
        Err("此功能需要 Windows".into())
    }
}
