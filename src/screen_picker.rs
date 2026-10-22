//! Explicit, cancellable screen eyedropper. Nothing starts until `EyeDropper::new` is called.
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EyeDropperPreview {
    pub position: [i32; 2],
    pub desktop_bounds: [i32; 4],
    pub color: [u8; 3],
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}
pub struct EyeDropper {
    pub previews: mpsc::Receiver<EyeDropperPreview>,
    pub results: mpsc::Receiver<Result<Option<[u8; 3]>, String>>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
/// Size the interactive picker overlay in physical virtual-desktop coordinates.
#[cfg(windows)]
pub fn configure_screen_picker(title: &str, bounds: [i32; 4], visible: bool) -> Result<(), String> {
    use windows_sys::Win32::{System::Threading::GetCurrentProcessId, UI::WindowsAndMessaging::*};
    let [x, y, width, height] = bounds;
    if width <= 0
        || height <= 0
        || width as u64 * height as u64 > 64 * 1024 * 1024
        || title.contains('\0')
    {
        return Err("取色覆盖窗口参数无效".into());
    }
    let title: Vec<u16> = title.encode_utf16().chain(Some(0)).collect();
    unsafe {
        let hwnd = FindWindowW(std::ptr::null(), title.as_ptr());
        let mut pid = 0;
        if hwnd.is_null()
            || GetWindowThreadProcessId(hwnd, &mut pid) == 0
            || pid != GetCurrentProcessId()
        {
            return Err("取色覆盖窗口必须属于当前应用".into());
        }
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        let desired =
            (style | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE) & !(WS_EX_APPWINDOW | WS_EX_TRANSPARENT);
        if style != desired {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, desired as isize);
        }
        if !visible {
            ShowWindow(hwnd, SW_HIDE);
            return Ok(());
        }
        if SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            x,
            y,
            width,
            height,
            SWP_NOACTIVATE | SWP_SHOWWINDOW | SWP_NOOWNERZORDER,
        ) == 0
        {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(())
    }
}
#[cfg(not(windows))]
pub fn configure_screen_picker(_: &str, _: [i32; 4], _: bool) -> Result<(), String> {
    Err("屏幕取色需要Windows".into())
}
#[derive(Clone, Copy, Default)]
struct Buttons {
    left: bool,
    right: bool,
    escape: bool,
}
#[derive(Default)]
struct Selection {
    released: bool,
}
#[derive(Debug, PartialEq, Eq)]
enum Step {
    Preview,
    Confirm,
    Cancel,
}
impl Selection {
    fn step(&mut self, buttons: Buttons, stop: bool) -> Step {
        if stop || buttons.right || buttons.escape {
            return Step::Cancel;
        }
        if !buttons.left {
            self.released = true;
        }
        if self.released && buttons.left {
            Step::Confirm
        } else {
            Step::Preview
        }
    }
}
fn magnifier(
    bytes: &[u8],
    bounds: [i32; 4],
    position: [i32; 2],
    side: i32,
) -> Result<Vec<u8>, String> {
    let [left, top, width, height] = bounds;
    if width <= 0
        || height <= 0
        || side <= 0
        || side % 2 == 0
        || side > 63
        || bytes.len() as u64 != width as u64 * height as u64 * 4
    {
        return Err("冻结屏幕像素尺寸无效".into());
    }
    let (x, y) = (
        position[0] as i64 - left as i64,
        position[1] as i64 - top as i64,
    );
    if x < 0 || y < 0 || x >= width as i64 || y >= height as i64 {
        return Err("鼠标坐标已离开冻结桌面范围".into());
    }
    let mut rgba = Vec::with_capacity((side * side * 4) as usize);
    for dy in -side / 2..=side / 2 {
        for dx in -side / 2..=side / 2 {
            let px = (x + dx as i64).clamp(0, width as i64 - 1);
            let py = (y + dy as i64).clamp(0, height as i64 - 1);
            let i = (py as usize * width as usize + px as usize) * 4;
            rgba.extend_from_slice(&[bytes[i + 2], bytes[i + 1], bytes[i], 255]);
        }
    }
    Ok(rgba)
}
trait Device {
    fn buttons(&mut self) -> Buttons;
    fn position(&mut self) -> Result<[i32; 2], String>;
    fn sample(&mut self, position: [i32; 2]) -> Result<EyeDropperPreview, String>;
}
fn run(
    device: &mut impl Device,
    stop: &AtomicBool,
    previews: &mpsc::SyncSender<EyeDropperPreview>,
    wake: &impl Fn(),
    delay: std::time::Duration,
) -> Result<Option<[u8; 3]>, String> {
    let mut state = Selection::default();
    loop {
        match state.step(device.buttons(), stop.load(Ordering::Acquire)) {
            Step::Cancel => return Ok(None),
            Step::Confirm => {
                let position = device.position()?;
                return device.sample(position).map(|p| Some(p.color));
            }
            Step::Preview => {
                let position = device.position()?;
                let preview = device.sample(position)?;
                match previews.try_send(preview) {
                    Ok(()) => wake(),
                    Err(mpsc::TrySendError::Full(_)) => {}
                    Err(mpsc::TrySendError::Disconnected(_)) => return Ok(None),
                }
            }
        }
        std::thread::sleep(delay);
    }
}
impl EyeDropper {
    #[cfg(windows)]
    pub fn new(wake: impl Fn() + Send + 'static) -> Result<Self, String> {
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let (preview_tx, previews) = mpsc::sync_channel(1);
        let (done, results) = mpsc::channel();
        let pause = crate::platform::PinEscapeSuspend::new();
        let thread = std::thread::Builder::new()
            .name("quicktool-eyedropper".into())
            .spawn(move || {
                let _pause = pause;
                let result = native::Screen::new().and_then(|mut device| {
                    run(
                        &mut device,
                        &worker_stop,
                        &preview_tx,
                        &wake,
                        std::time::Duration::from_millis(16),
                    )
                });
                let _ = done.send(result);
                wake();
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            previews,
            results,
            stop,
            thread: Some(thread),
        })
    }
    #[cfg(not(windows))]
    pub fn new(_: impl Fn() + Send + 'static) -> Result<Self, String> {
        Err("屏幕取色需要Windows".into())
    }
    pub fn cancel(&self) {
        self.stop.store(true, Ordering::Release);
    }
}
impl Drop for EyeDropper {
    fn drop(&mut self) {
        self.cancel();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
#[cfg(windows)]
mod native {
    use super::*;
    use std::ptr::null_mut;
    use windows_sys::Win32::{
        Foundation::POINT,
        Graphics::Gdi::*,
        UI::{
            Input::KeyboardAndMouse::{GetAsyncKeyState, VK_ESCAPE, VK_LBUTTON, VK_RBUTTON},
            WindowsAndMessaging::{
                GetCursorPos, GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN,
                SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
            },
        },
    };
    const SIDE: i32 = 15;
    pub(super) struct Screen {
        screen: HDC,
        memory: HDC,
        bitmap: HBITMAP,
        previous: HGDIOBJ,
        pixels: *mut u8,
        bounds: [i32; 4],
    }
    impl Screen {
        pub(super) fn new() -> Result<Self, String> {
            unsafe {
                let bounds = [
                    GetSystemMetrics(SM_XVIRTUALSCREEN),
                    GetSystemMetrics(SM_YVIRTUALSCREEN),
                    GetSystemMetrics(SM_CXVIRTUALSCREEN),
                    GetSystemMetrics(SM_CYVIRTUALSCREEN),
                ];
                if bounds[2] <= 0
                    || bounds[3] <= 0
                    || bounds[2] as u64 * bounds[3] as u64 > 64 * 1024 * 1024
                {
                    return Err("虚拟桌面尺寸无效或截图超过256MB".into());
                }
                let screen = GetDC(null_mut());
                if screen.is_null() {
                    return Err("无法取得屏幕DC".into());
                }
                let mut state = Self {
                    screen,
                    memory: null_mut(),
                    bitmap: null_mut(),
                    previous: null_mut(),
                    pixels: null_mut(),
                    bounds,
                };
                state.memory = CreateCompatibleDC(screen);
                if state.memory.is_null() {
                    return Err("无法创建屏幕采样DC".into());
                }
                let mut info: BITMAPINFO = std::mem::zeroed();
                info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
                info.bmiHeader.biWidth = bounds[2];
                info.bmiHeader.biHeight = -bounds[3];
                info.bmiHeader.biPlanes = 1;
                info.bmiHeader.biBitCount = 32;
                info.bmiHeader.biCompression = BI_RGB;
                let mut pixels = null_mut();
                state.bitmap = CreateDIBSection(
                    state.memory,
                    &info,
                    DIB_RGB_COLORS,
                    &mut pixels,
                    null_mut(),
                    0,
                );
                if state.bitmap.is_null() || pixels.is_null() {
                    return Err("无法创建屏幕采样位图".into());
                }
                state.pixels = pixels.cast();
                state.previous = SelectObject(state.memory, state.bitmap);
                if state.previous.is_null() || state.previous as isize == -1 {
                    return Err("无法选择屏幕采样位图".into());
                }
                if BitBlt(
                    state.memory,
                    0,
                    0,
                    bounds[2],
                    bounds[3],
                    state.screen,
                    bounds[0],
                    bounds[1],
                    SRCCOPY | CAPTUREBLT,
                ) == 0
                {
                    return Err("无法冻结虚拟桌面截图".into());
                }
                GdiFlush();
                // The real desktop is read once, before the UI displays its picker overlay.
                ReleaseDC(null_mut(), state.screen);
                state.screen = null_mut();
                Ok(state)
            }
        }
    }
    impl Device for Screen {
        fn buttons(&mut self) -> Buttons {
            unsafe {
                Buttons {
                    left: GetAsyncKeyState(VK_LBUTTON as i32) < 0,
                    right: GetAsyncKeyState(VK_RBUTTON as i32) < 0,
                    escape: GetAsyncKeyState(VK_ESCAPE as i32) < 0,
                }
            }
        }
        fn position(&mut self) -> Result<[i32; 2], String> {
            unsafe {
                let mut p: POINT = std::mem::zeroed();
                if GetCursorPos(&mut p) == 0 {
                    return Err("无法取得取色坐标".into());
                }
                Ok([p.x, p.y])
            }
        }
        fn sample(&mut self, position: [i32; 2]) -> Result<EyeDropperPreview, String> {
            unsafe {
                let [x, y] = position;
                let [left, top, width, height] = self.bounds;
                let (local_x, local_y) = (x - left, y - top);
                if local_x < 0 || local_y < 0 || local_x >= width || local_y >= height {
                    return Err("鼠标坐标已离开冻结桌面范围".into());
                }
                let color = GetPixel(self.memory, local_x, local_y);
                if color == 0xffff_ffff {
                    return Err("无法读取取色位置的像素".into());
                }
                let color = [color as u8, (color >> 8) as u8, (color >> 16) as u8];
                let bytes =
                    std::slice::from_raw_parts(self.pixels, width as usize * height as usize * 4);
                let rgba = magnifier(bytes, self.bounds, position, SIDE)?;
                Ok(EyeDropperPreview {
                    position,
                    desktop_bounds: self.bounds,
                    color,
                    width: SIDE as u32,
                    height: SIDE as u32,
                    rgba,
                })
            }
        }
    }
    impl Drop for Screen {
        fn drop(&mut self) {
            unsafe {
                if !self.memory.is_null()
                    && !self.previous.is_null()
                    && self.previous as isize != -1
                {
                    SelectObject(self.memory, self.previous);
                }
                if !self.bitmap.is_null() {
                    DeleteObject(self.bitmap);
                }
                if !self.memory.is_null() {
                    DeleteDC(self.memory);
                }
                if !self.screen.is_null() {
                    ReleaseDC(null_mut(), self.screen);
                }
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    struct Mock {
        buttons: Vec<Buttons>,
        index: usize,
        samples: Arc<AtomicUsize>,
        drops: Arc<AtomicUsize>,
        fail: bool,
    }
    impl Drop for Mock {
        fn drop(&mut self) {
            self.drops.fetch_add(1, Ordering::Relaxed);
        }
    }
    impl Device for Mock {
        fn buttons(&mut self) -> Buttons {
            let b = self.buttons.get(self.index).copied().unwrap_or(Buttons {
                escape: true,
                ..Default::default()
            });
            self.index += 1;
            b
        }
        fn position(&mut self) -> Result<[i32; 2], String> {
            Ok([-120, 80])
        }
        fn sample(&mut self, position: [i32; 2]) -> Result<EyeDropperPreview, String> {
            self.samples.fetch_add(1, Ordering::Relaxed);
            if self.fail {
                return Err("sample error".into());
            }
            Ok(EyeDropperPreview {
                position,
                desktop_bounds: [-120, 0, 640, 480],
                color: [1, 2, 3],
                width: 1,
                height: 1,
                rgba: vec![1, 2, 3, 255],
            })
        }
    }
    fn check(
        buttons: Vec<Buttons>,
        stop: bool,
        fail: bool,
    ) -> (Result<Option<[u8; 3]>, String>, usize, usize) {
        let samples = Arc::new(AtomicUsize::new(0));
        let drops = Arc::new(AtomicUsize::new(0));
        let (tx, _rx) = mpsc::sync_channel(1);
        let result = {
            let mut mock = Mock {
                buttons,
                index: 0,
                samples: samples.clone(),
                drops: drops.clone(),
                fail,
            };
            run(
                &mut mock,
                &AtomicBool::new(stop),
                &tx,
                &|| {},
                std::time::Duration::ZERO,
            )
        };
        (
            result,
            samples.load(Ordering::Relaxed),
            drops.load(Ordering::Relaxed),
        )
    }
    #[test]
    fn frozen_bgra_magnifier_handles_negative_desktop_origin_and_clamps_edges() {
        let pixels = [3, 2, 1, 0, 6, 5, 4, 0, 9, 8, 7, 0, 12, 11, 10, 0];
        let rgba = magnifier(&pixels, [-20, -10, 2, 2], [-20, -10], 3).unwrap();
        assert_eq!(&rgba[16..20], &[1, 2, 3, 255]);
        assert_eq!(&rgba[32..36], &[10, 11, 12, 255]);
        assert_eq!(&rgba[..4], &[1, 2, 3, 255]);
        assert!(magnifier(&pixels, [-20, -10, 2, 2], [-21, -10], 3).is_err());
        assert!(magnifier(&pixels[..12], [-20, -10, 2, 2], [-20, -10], 3).is_err());
    }
    #[test]
    fn waits_for_release_before_next_press() {
        let (r, s, d) = check(
            vec![
                Buttons {
                    left: true,
                    ..Default::default()
                },
                Buttons {
                    left: true,
                    ..Default::default()
                },
                Buttons::default(),
                Buttons {
                    left: true,
                    ..Default::default()
                },
            ],
            false,
            false,
        );
        assert_eq!(r.unwrap(), Some([1, 2, 3]));
        assert_eq!((s, d), (4, 1));
    }
    #[test]
    fn escape_right_and_explicit_cancel_do_not_sample() {
        for (b, stop) in [
            (
                Buttons {
                    escape: true,
                    ..Default::default()
                },
                false,
            ),
            (
                Buttons {
                    right: true,
                    ..Default::default()
                },
                false,
            ),
            (Buttons::default(), true),
        ] {
            let (r, s, d) = check(vec![b], stop, false);
            assert_eq!(r.unwrap(), None);
            assert_eq!((s, d), (0, 1));
        }
    }
    #[test]
    fn sampling_error_releases_device() {
        let (r, s, d) = check(vec![Buttons::default()], false, true);
        assert_eq!(r.unwrap_err(), "sample error");
        assert_eq!((s, d), (1, 1));
    }
    #[test]
    fn bounded_preview_queue_does_not_block_confirmation() {
        let (r, _, d) = check(
            vec![
                Buttons::default(),
                Buttons::default(),
                Buttons {
                    left: true,
                    ..Default::default()
                },
            ],
            false,
            false,
        );
        assert_eq!(r.unwrap(), Some([1, 2, 3]));
        assert_eq!(d, 1);
    }
    #[test]
    fn controller_drop_cancels_and_joins_worker() {
        let stop = Arc::new(AtomicBool::new(false));
        let signal = stop.clone();
        let finished = Arc::new(AtomicBool::new(false));
        let done = finished.clone();
        let (_tx, previews) = mpsc::channel();
        let (_done, results) = mpsc::channel();
        let thread = std::thread::spawn(move || {
            while !signal.load(Ordering::Acquire) {
                std::thread::yield_now();
            }
            done.store(true, Ordering::Release);
        });
        drop(EyeDropper {
            previews,
            results,
            stop,
            thread: Some(thread),
        });
        assert!(finished.load(Ordering::Acquire));
    }
}
