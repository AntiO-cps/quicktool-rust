//! Fixed launcher placement, matching the original work-area anchor.

#[derive(Default)]
pub struct LauncherPlacement {
    #[cfg(windows)]
    monitor: isize,
    #[cfg(windows)]
    owner: isize,
}

#[derive(Debug, PartialEq)]
struct Layout {
    bounds: [i32; 4],
    logical: [f32; 3],
}

fn layout(work: [i32; 4], scale: f64, allocated_height: f32) -> Layout {
    let scale = if scale.is_finite() && scale > 0. {
        scale
    } else {
        1.
    };
    let [left, top, right, bottom] = work;
    let area_width = (right as f64 - left as f64).max(1.);
    let area_height = (bottom as f64 - top as f64).max(1.);
    let width = (area_width / scale - 40.).clamp(1., 812.);
    let physical_width = (width * scale).round().max(1.) as i32;
    let x = left.saturating_add(((area_width - physical_width as f64) / 2.).round() as i32);
    let y = top.saturating_add((area_height * 0.22).round() as i32);
    let maximum = ((bottom as f64 - y as f64) / scale - 120.).max(84.);
    let allocated = if allocated_height.is_finite() {
        allocated_height as f64
    } else {
        84.
    };
    let physical_height = (allocated.clamp(84., maximum) * scale).round().max(1.) as i32;
    Layout {
        bounds: [x, y, physical_width, physical_height],
        logical: [
            (physical_width as f64 / scale) as f32,
            (physical_height as f64 / scale) as f32,
            maximum as f32,
        ],
    }
}

#[cfg(windows)]
mod native {
    use super::*;
    use windows_sys::Win32::{
        Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
        Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint},
        System::Threading::{GetCurrentProcessId, GetCurrentThreadId},
        UI::{
            HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI},
            Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
            WindowsAndMessaging::{
                GetCursorPos, GetWindowRect, GetWindowThreadProcessId, HTCAPTION, HTCLIENT,
                IsWindow, SC_MOVE, SWP_NOACTIVATE, SWP_NOZORDER, SetWindowPos, WM_NCDESTROY,
                WM_NCHITTEST, WM_SYSCOMMAND,
            },
        },
    };

    const SUBCLASS_ID: usize = 0x51544c50;

    unsafe extern "system" fn fixed_launcher(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        id: usize,
        _reference: usize,
    ) -> LRESULT {
        unsafe {
            match message {
                WM_SYSCOMMAND if wparam as u32 & 0xfff0 == SC_MOVE => 0,
                WM_NCHITTEST => {
                    let hit = DefSubclassProc(hwnd, message, wparam, lparam);
                    if hit == HTCAPTION as isize {
                        HTCLIENT as isize
                    } else {
                        hit
                    }
                }
                WM_NCDESTROY => {
                    RemoveWindowSubclass(hwnd, Some(fixed_launcher), id);
                    DefSubclassProc(hwnd, message, wparam, lparam)
                }
                _ => DefSubclassProc(hwnd, message, wparam, lparam),
            }
        }
    }

    fn cursor_monitor() -> Result<isize, String> {
        unsafe {
            let mut cursor = POINT { x: 0, y: 0 };
            if GetCursorPos(&mut cursor) == 0 {
                return Err("无法读取启动器目标显示器".into());
            }
            let monitor = MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST);
            if monitor.is_null() {
                Err("无法定位启动器显示器".into())
            } else {
                Ok(monitor as isize)
            }
        }
    }

    impl LauncherPlacement {
        /// Re-select the cursor's display only on an explicit summon or when the
        /// previous monitor disappears. Coordinates and HWND sizes are physical.
        pub fn update(
            &mut self,
            owner: isize,
            select_display: bool,
            allocated_height: f32,
        ) -> Result<[f32; 3], String> {
            unsafe {
                let hwnd = owner as HWND;
                let mut process = 0;
                let thread = GetWindowThreadProcessId(hwnd, &mut process);
                if IsWindow(hwnd) == 0 || process != GetCurrentProcessId() {
                    return Err("启动器必须是当前应用的有效窗口".into());
                }
                if thread != GetCurrentThreadId() {
                    return Err("启动器定位必须在其窗口线程执行".into());
                }
                if self.owner != owner {
                    if SetWindowSubclass(hwnd, Some(fixed_launcher), SUBCLASS_ID, 0) == 0 {
                        return Err("无法固定启动器窗口位置".into());
                    }
                    self.owner = owner;
                }
                if select_display || self.monitor == 0 {
                    self.monitor = cursor_monitor()?;
                }
                let mut monitor: MONITORINFO = std::mem::zeroed();
                monitor.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
                if GetMonitorInfoW(self.monitor as _, &mut monitor) == 0 {
                    self.monitor = cursor_monitor()?;
                    if GetMonitorInfoW(self.monitor as _, &mut monitor) == 0 {
                        return Err("无法读取启动器显示器工作区".into());
                    }
                }
                let mut dpi_x = 0;
                let mut dpi_y = 0;
                if GetDpiForMonitor(self.monitor as _, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y)
                    < 0
                    || dpi_x == 0
                {
                    return Err("无法读取启动器显示器缩放".into());
                }
                let work = monitor.rcWork;
                let placement = layout(
                    [work.left, work.top, work.right, work.bottom],
                    dpi_x as f64 / 96.,
                    allocated_height,
                );
                let mut previous: RECT = std::mem::zeroed();
                if GetWindowRect(hwnd, &mut previous) == 0 {
                    return Err("无法读取启动器窗口边界".into());
                }
                let actual = [
                    previous.left,
                    previous.top,
                    previous.right - previous.left,
                    previous.bottom - previous.top,
                ];
                if actual != placement.bounds {
                    let [x, y, width, height] = placement.bounds;
                    if SetWindowPos(
                        hwnd,
                        std::ptr::null_mut(),
                        x,
                        y,
                        width,
                        height,
                        SWP_NOACTIVATE | SWP_NOZORDER,
                    ) == 0
                    {
                        return Err(format!(
                            "启动器定位失败：{}",
                            std::io::Error::last_os_error()
                        ));
                    }
                }
                Ok(placement.logical)
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::{cell::Cell, ptr::null_mut};
        use windows_sys::Win32::{
            Foundation::HINSTANCE,
            System::LibraryLoader::GetModuleHandleW,
            UI::{
                HiDpi::{
                    DPI_AWARENESS_CONTEXT, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
                    SetThreadDpiAwarenessContext,
                },
                WindowsAndMessaging::{
                    CreateWindowExW, DefWindowProcW, DestroyWindow, RegisterClassW, SendMessageW,
                    UnregisterClassW, WM_WINDOWPOSCHANGING, WNDCLASSW, WS_EX_NOACTIVATE,
                    WS_EX_TOOLWINDOW, WS_POPUP,
                },
            },
        };

        thread_local! {
            static SYSTEM_MOVES: Cell<usize> = const { Cell::new(0) };
            static POSITION_CHANGES: Cell<usize> = const { Cell::new(0) };
        }

        unsafe extern "system" fn test_window(
            hwnd: HWND,
            message: u32,
            wparam: WPARAM,
            lparam: LPARAM,
        ) -> LRESULT {
            match message {
                WM_SYSCOMMAND if wparam as u32 & 0xfff0 == SC_MOVE => {
                    SYSTEM_MOVES.with(|count| count.set(count.get() + 1));
                    // Never enter the system move loop, even if the production
                    // subclass regresses and forwards the command here.
                    0
                }
                WM_NCHITTEST => HTCAPTION as isize,
                WM_WINDOWPOSCHANGING => {
                    POSITION_CHANGES.with(|count| count.set(count.get() + 1));
                    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
                }
                _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
            }
        }

        struct TestWindow {
            hwnd: HWND,
            class: Vec<u16>,
            instance: HINSTANCE,
            previous_dpi: DPI_AWARENESS_CONTEXT,
        }

        impl TestWindow {
            fn new() -> Self {
                unsafe {
                    let previous_dpi =
                        SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
                    assert!(!previous_dpi.is_null(), "set fixture thread DPI awareness");
                    let mut fixture = Self {
                        hwnd: null_mut(),
                        class: format!(
                            "QuickToolLauncherNativeTest_{}_{}",
                            GetCurrentProcessId(),
                            GetCurrentThreadId()
                        )
                        .encode_utf16()
                        .chain(Some(0))
                        .collect(),
                        instance: GetModuleHandleW(std::ptr::null()),
                        previous_dpi,
                    };
                    let class = WNDCLASSW {
                        lpfnWndProc: Some(test_window),
                        hInstance: fixture.instance,
                        lpszClassName: fixture.class.as_ptr(),
                        ..std::mem::zeroed()
                    };
                    assert_ne!(RegisterClassW(&class), 0, "register fixture window class");
                    fixture.hwnd = CreateWindowExW(
                        WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
                        fixture.class.as_ptr(),
                        fixture.class.as_ptr(),
                        WS_POPUP,
                        0,
                        0,
                        1,
                        1,
                        null_mut(),
                        null_mut(),
                        fixture.instance,
                        std::ptr::null(),
                    );
                    assert!(!fixture.hwnd.is_null(), "create hidden fixture window");
                    fixture
                }
            }
            fn bounds(&self) -> [i32; 4] {
                unsafe {
                    let mut rect = std::mem::zeroed::<RECT>();
                    assert_ne!(GetWindowRect(self.hwnd, &mut rect), 0);
                    [
                        rect.left,
                        rect.top,
                        rect.right - rect.left,
                        rect.bottom - rect.top,
                    ]
                }
            }
        }

        impl Drop for TestWindow {
            fn drop(&mut self) {
                unsafe {
                    if !self.hwnd.is_null() {
                        DestroyWindow(self.hwnd);
                    }
                    UnregisterClassW(self.class.as_ptr(), self.instance);
                    SetThreadDpiAwarenessContext(self.previous_dpi);
                }
            }
        }

        #[test]
        fn native_anchor_resizes_without_moving_and_blocks_system_move() {
            let mut fixture = TestWindow::new();
            let mut placement = LauncherPlacement::default();
            placement.update(fixture.hwnd as isize, true, 84.).unwrap();
            unsafe {
                // The fixture's own procedure returns HTCAPTION. Conversion to
                // HTCLIENT proves the production subclass is installed before
                // testing SC_MOVE. Its fallback also never enters a move loop.
                assert_eq!(
                    SendMessageW(fixture.hwnd, WM_NCHITTEST, 0, 0),
                    HTCLIENT as isize,
                    "production subclass must exist before sending any move command"
                );
                SYSTEM_MOVES.with(|count| count.set(0));
                SendMessageW(fixture.hwnd, WM_SYSCOMMAND, SC_MOVE as usize, 0);
                SendMessageW(fixture.hwnd, WM_SYSCOMMAND, (SC_MOVE | 2) as usize, 0);
                assert_eq!(SYSTEM_MOVES.with(Cell::get), 0);

                let mut monitor: MONITORINFO = std::mem::zeroed();
                monitor.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
                assert_ne!(GetMonitorInfoW(placement.monitor as _, &mut monitor), 0);
                let mut dpi_x = 0;
                let mut dpi_y = 0;
                assert!(
                    GetDpiForMonitor(
                        placement.monitor as _,
                        MDT_EFFECTIVE_DPI,
                        &mut dpi_x,
                        &mut dpi_y
                    ) >= 0
                );
                let scale = dpi_x as f64 / 96.;
                let work = monitor.rcWork;
                let area_width = (work.right - work.left) as f64;
                let area_height = (work.bottom - work.top) as f64;
                let expected_width = ((area_width / scale - 40.).min(812.) * scale).round() as i32;
                let expected_x =
                    work.left + ((area_width - expected_width as f64) / 2.).round() as i32;
                let expected_y = work.top + (area_height * 0.22).round() as i32;
                let maximum = ((work.bottom - expected_y) as f64 / scale - 120.).max(84.);

                for height in [642., 220., 84., 642., 84.] {
                    let returned = placement
                        .update(fixture.hwnd as isize, false, height)
                        .unwrap();
                    let actual = fixture.bounds();
                    assert_eq!(actual[0], expected_x, "horizontal anchor changed on resize");
                    assert_eq!(actual[1], expected_y, "vertical anchor changed on resize");
                    assert_eq!(actual[2], expected_width);
                    assert_eq!(
                        actual[3],
                        ((height as f64).clamp(84., maximum) * scale).round() as i32
                    );
                    assert!((returned[0] as f64 - actual[2] as f64 / scale).abs() < 0.001);
                    assert!((returned[1] as f64 - actual[3] as f64 / scale).abs() < 0.001);
                    assert!((returned[2] as f64 - maximum).abs() < 0.001);

                    let changes = POSITION_CHANGES.with(Cell::get);
                    placement
                        .update(fixture.hwnd as isize, false, height)
                        .unwrap();
                    assert_eq!(
                        POSITION_CHANGES.with(Cell::get),
                        changes,
                        "unchanged bounds were reapplied"
                    );
                }

                // Destroying the fixture runs the production WM_NCDESTROY path.
                assert_ne!(DestroyWindow(fixture.hwnd), 0);
                assert_eq!(IsWindow(fixture.hwnd), 0);
                fixture.hwnd = null_mut();
            }
        }
    }
}

#[cfg(not(windows))]
impl LauncherPlacement {
    pub fn update(
        &mut self,
        _owner: isize,
        _select_display: bool,
        allocated_height: f32,
    ) -> Result<[f32; 3], String> {
        Ok(layout([0, 0, 1920, 1080], 1., allocated_height).logical)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_anchor_and_margin_match_original_layout() {
        assert_eq!(
            layout([0, 0, 1920, 1080], 1., 84.).bounds,
            [554, 238, 812, 84]
        );
        let expanded = layout([0, 0, 1920, 1080], 1., 642.);
        assert_eq!(expanded.bounds, [554, 238, 812, 642]);
        assert_eq!(expanded.logical, [812., 642., 722.]);
        let limited = layout([0, 0, 1280, 720], 1., 642.);
        assert_eq!(limited.bounds, [234, 158, 812, 442]);
        assert_eq!(limited.logical, [812., 442., 442.]);
    }

    #[test]
    fn monitor_origin_and_dpi_preserve_logical_gutters() {
        let scaled = layout([-1920, -120, 0, 960], 1.25, 642.);
        assert_eq!(scaled.bounds, [-1467, 118, 1015, 692]);
        assert!((scaled.logical[1] - 553.6).abs() < 0.001);
        assert!((scaled.logical[2] - 553.6).abs() < 0.001);
        let narrow = layout([2000, 0, 2900, 1000], 1.5, 84.);
        assert_eq!(narrow.bounds, [2030, 220, 840, 126]);
        assert_eq!(narrow.logical[0], 560.);
    }

    #[test]
    fn small_work_areas_and_invalid_heights_keep_valid_windows() {
        for height in [-1., 0., 1., f32::NAN, f32::INFINITY] {
            let placement = layout([0, 0, 30, 50], 1., height);
            assert_eq!(placement.bounds[2], 1);
            assert_eq!(placement.logical[1], 84.);
            assert_eq!(placement.logical[2], 84.);
        }
    }
}
