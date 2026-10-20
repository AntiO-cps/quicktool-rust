//! Adjustable host-backdrop Gaussian blur, matching the original compositor.
//! Each owner gets a non-activating, click-through surface immediately behind it.
#[cfg(windows)]
mod native {
    use std::ptr::{null, null_mut};
    use std::{cell::RefCell, sync::mpsc, thread::JoinHandle};
    use windows::{
        Foundation::{Collections::IIterable, IPropertyValue, Numerics::Vector2, PropertyValue},
        Graphics::Effects::{
            IGraphicsEffect, IGraphicsEffect_Impl, IGraphicsEffectSource,
            IGraphicsEffectSource_Impl,
        },
        UI::Composition::*,
        Win32::{
            Foundation::{E_INVALIDARG, HWND as WinHwnd},
            System::WinRT::{Composition::ICompositorDesktopInterop, Graphics::Direct2D::*, *},
        },
        core::{GUID, HSTRING, Interface, PCWSTR, Result as WinResult, implement},
    };
    use windows_sys::Win32::{
        Foundation::*,
        Graphics::{Dwm::*, Gdi::*},
        System::{Com::*, LibraryLoader::GetModuleHandleW, Threading::GetCurrentThreadId},
        UI::WindowsAndMessaging::*,
    };
    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    #[implement(IGraphicsEffect, IGraphicsEffectSource, IGraphicsEffectD2D1Interop)]
    struct BlurDescription {
        name: RefCell<HSTRING>,
        source: IGraphicsEffectSource,
    }
    impl IGraphicsEffect_Impl for BlurDescription_Impl {
        fn Name(&self) -> WinResult<HSTRING> {
            Ok(self.name.borrow().clone())
        }
        fn SetName(&self, name: &HSTRING) -> WinResult<()> {
            *self.name.borrow_mut() = name.clone();
            Ok(())
        }
    }
    impl IGraphicsEffectSource_Impl for BlurDescription_Impl {}
    impl IGraphicsEffectD2D1Interop_Impl for BlurDescription_Impl {
        fn GetEffectId(&self) -> WinResult<GUID> {
            Ok(GUID::from_u128(0x1feb6d69_2fe6_4ac9_8c58_1d7f93e7a6a5))
        }
        fn GetNamedPropertyMapping(
            &self,
            name: &PCWSTR,
            index: *mut u32,
            mapping: *mut GRAPHICS_EFFECT_PROPERTY_MAPPING,
        ) -> WinResult<()> {
            if index.is_null() || mapping.is_null() {
                return Err(E_INVALIDARG.into());
            }
            unsafe {
                *index = 0;
                *mapping = GRAPHICS_EFFECT_PROPERTY_MAPPING_DIRECT;
                if name.to_string()? == "BlurAmount" {
                    Ok(())
                } else {
                    Err(E_INVALIDARG.into())
                }
            }
        }
        fn GetPropertyCount(&self) -> WinResult<u32> {
            Ok(3)
        }
        fn GetProperty(&self, index: u32) -> WinResult<IPropertyValue> {
            match index {
                0 => PropertyValue::CreateSingle(0.0)?.cast(),
                1 => PropertyValue::CreateUInt32(0)?.cast(),
                2 => PropertyValue::CreateUInt32(1)?.cast(),
                _ => Err(E_INVALIDARG.into()),
            }
        }
        fn GetSource(&self, index: u32) -> WinResult<IGraphicsEffectSource> {
            if index == 0 {
                Ok(self.source.clone())
            } else {
                Err(E_INVALIDARG.into())
            }
        }
        fn GetSourceCount(&self) -> WinResult<u32> {
            Ok(1)
        }
    }
    struct Composition {
        compositor: Compositor,
        target: Desktop::DesktopWindowTarget,
        sprite: SpriteVisual,
        effect: CompositionEffectBrush,
        shape: CompositionRoundedRectangleGeometry,
        _queue: windows::System::DispatcherQueueController,
    }
    impl Composition {
        fn new(hwnd: HWND) -> WinResult<Self> {
            unsafe {
                let queue = CreateDispatcherQueueController(DispatcherQueueOptions {
                    dwSize: std::mem::size_of::<DispatcherQueueOptions>() as u32,
                    threadType: DQTYPE_THREAD_CURRENT,
                    apartmentType: DQTAT_COM_STA,
                })?;
                let compositor = Compositor::new()?;
                let desktop: ICompositorDesktopInterop = compositor.cast()?;
                let target = desktop.CreateDesktopWindowTarget(WinHwnd(hwnd), true)?;
                let source = CompositionEffectSourceParameter::Create(&HSTRING::from("Desktop"))?;
                let description: IGraphicsEffect = BlurDescription {
                    name: RefCell::new(HSTRING::from("Blur")),
                    source: source.cast()?,
                }
                .into();
                let properties: IIterable<HSTRING> =
                    vec![HSTRING::from("Blur.BlurAmount")].try_into()?;
                let factory =
                    compositor.CreateEffectFactoryWithProperties(&description, &properties)?;
                let effect = factory.CreateBrush()?;
                effect.SetSourceParameter(
                    &HSTRING::from("Desktop"),
                    &compositor.CreateHostBackdropBrush()?,
                )?;
                let sprite = compositor.CreateSpriteVisual()?;
                sprite.SetBrush(&effect)?;
                let shape = compositor.CreateRoundedRectangleGeometry()?;
                let clip = compositor.CreateGeometricClipWithGeometry(&shape)?;
                sprite.SetClip(&clip)?;
                target.SetRoot(&sprite)?;
                let enabled = 1i32;
                let none = 1i32;
                DwmSetWindowAttribute(hwnd, 17, &enabled as *const _ as _, 4);
                DwmSetWindowAttribute(hwnd, 38, &none as *const _ as _, 4);
                Ok(Self {
                    compositor,
                    target,
                    sprite,
                    effect,
                    shape,
                    _queue: queue,
                })
            }
        }
        fn update(&self, width: i32, height: i32, blur: f32, radius: f32) -> WinResult<()> {
            let size = Vector2 {
                X: width.max(1) as f32,
                Y: height.max(1) as f32,
            };
            self.sprite.SetSize(size)?;
            self.shape.SetSize(size)?;
            self.shape.SetCornerRadius(Vector2 {
                X: radius,
                Y: radius,
            })?;
            self.effect
                .Properties()?
                .InsertScalar(&HSTRING::from("Blur.BlurAmount"), blur.clamp(0.0, 64.0))?;
            self.sprite.SetIsVisible(blur > 0.0)?;
            Ok(())
        }
        fn actual_blur(&self) -> WinResult<f32> {
            let mut actual = 0.0;
            self.effect
                .Properties()?
                .TryGetScalar(&HSTRING::from("Blur.BlurAmount"), &mut actual)?;
            Ok(actual)
        }
    }
    impl Drop for Composition {
        fn drop(&mut self) {
            let _ = self.target.SetRoot(None::<&Visual>);
            let _ = self.effect.Close();
            let _ = self.sprite.Close();
            let _ = self.compositor.Close();
        }
    }

    #[derive(Clone, Debug, PartialEq)]
    struct Snapshot {
        owner: isize,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        visible: bool,
        topmost: bool,
        dark: bool,
        blur: f32,
        radius: f32,
    }
    pub struct Backdrop {
        native_hwnd: isize,
        thread_id: u32,
        updates: mpsc::Sender<Snapshot>,
        last: Option<Snapshot>,
        join: Option<JoinHandle<()>>,
        pub actual_blur: std::sync::Arc<std::sync::Mutex<f32>>,
        owner: isize,
    }
    #[derive(Clone, Debug)]
    pub struct BackdropStatus {
        pub window: isize,
        pub visible: bool,
        pub topmost: bool,
        pub bounds: [i32; 4],
        pub blur: f32,
    }
    unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
        unsafe {
            match msg {
                WM_NCHITTEST => HTTRANSPARENT as isize,
                WM_MOUSEACTIVATE => MA_NOACTIVATE as isize,
                WM_NCCALCSIZE => 0,
                WM_ERASEBKGND => 1,
                WM_NCACTIVATE => DefWindowProcW(hwnd, msg, 1, l),
                WM_PAINT => {
                    let mut paint: PAINTSTRUCT = std::mem::zeroed();
                    let dc = BeginPaint(hwnd, &mut paint);
                    let mut rect: RECT = std::mem::zeroed();
                    GetClientRect(hwnd, &mut rect);
                    FillRect(dc, &rect, GetStockObject(BLACK_BRUSH) as HBRUSH);
                    EndPaint(hwnd, &paint);
                    0
                }
                WM_CLOSE => {
                    DestroyWindow(hwnd);
                    0
                }
                WM_DESTROY => {
                    PostQuitMessage(0);
                    0
                }
                _ => DefWindowProcW(hwnd, msg, w, l),
            }
        }
    }
    unsafe fn material(hwnd: HWND, dark: bool) {
        unsafe {
            let dark = dark as i32;
            let corner = 2i32;
            let border = 0xfffffffeu32;
            let none = 1i32;
            DwmSetWindowAttribute(hwnd, 20, &dark as *const _ as _, 4);
            DwmSetWindowAttribute(hwnd, 33, &corner as *const _ as _, 4);
            DwmSetWindowAttribute(hwnd, 34, &border as *const _ as _, 4);
            DwmSetWindowAttribute(hwnd, 38, &none as *const _ as _, 4);
            let margins = windows_sys::Win32::UI::Controls::MARGINS {
                cxLeftWidth: -1,
                cxRightWidth: -1,
                cyTopHeight: -1,
                cyBottomHeight: -1,
            };
            DwmExtendFrameIntoClientArea(hwnd, &margins);
            InvalidateRect(hwnd, null(), 0);
        }
    }
    impl Backdrop {
        pub fn new(owner: isize) -> Result<Self, String> {
            if unsafe { IsWindow(owner as HWND) } == 0 {
                return Err("材质所有者窗口无效".into());
            }
            let (updates, rx) = mpsc::channel::<Snapshot>();
            let (ready, wait) = mpsc::channel();
            let actual_blur = std::sync::Arc::new(std::sync::Mutex::new(0.0));
            let actual = actual_blur.clone();
            let join = std::thread::spawn(move || unsafe {
                let initialized = CoInitializeEx(null(), COINIT_APARTMENTTHREADED as u32) >= 0;
                let tid = GetCurrentThreadId();
                let instance = GetModuleHandleW(null());
                let class = wide(&format!("QuickToolRustBackdrop{tid}"));
                let mut wc: WNDCLASSW = std::mem::zeroed();
                wc.lpfnWndProc = Some(wndproc);
                wc.hInstance = instance;
                wc.lpszClassName = class.as_ptr();
                wc.hbrBackground = GetStockObject(BLACK_BRUSH) as HBRUSH;
                if RegisterClassW(&wc) == 0 {
                    let _ = ready.send(Err("材质窗口类注册失败".to_string()));
                    if initialized {
                        CoUninitialize()
                    }
                    return;
                }
                let hwnd = CreateWindowExW(
                    WS_EX_NOACTIVATE
                        | WS_EX_TOOLWINDOW
                        | WS_EX_NOREDIRECTIONBITMAP
                        | WS_EX_TRANSPARENT,
                    class.as_ptr(),
                    wide("QuickTool 材质").as_ptr(),
                    WS_POPUP | WS_THICKFRAME,
                    0,
                    0,
                    1,
                    1,
                    null_mut(),
                    null_mut(),
                    instance,
                    null(),
                );
                if hwnd.is_null() {
                    let _ = ready.send(Err("材质窗口创建失败".to_string()));
                    UnregisterClassW(class.as_ptr(), instance);
                    if initialized {
                        CoUninitialize()
                    }
                    return;
                }
                let composition = match Composition::new(hwnd) {
                    Ok(c) => c,
                    Err(e) => {
                        let _ = ready.send(Err(format!("合成模糊初始化失败：{e}")));
                        DestroyWindow(hwnd);
                        UnregisterClassW(class.as_ptr(), instance);
                        if initialized {
                            CoUninitialize()
                        }
                        return;
                    }
                };
                material(hwnd, true);
                let mut msg: MSG = std::mem::zeroed();
                PeekMessageW(&mut msg, null_mut(), 0, 0, PM_NOREMOVE);
                let _ = ready.send(Ok((tid, hwnd as isize)));
                while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
                    if msg.message == WM_APP + 21 {
                        let Some(s) = rx.try_iter().last() else {
                            continue;
                        };
                        let _ = composition.update(s.width, s.height, s.blur, s.radius);
                        if let Ok(mut value) = actual.lock() {
                            *value = composition.actual_blur().unwrap_or(-1.0);
                        }
                        if !s.visible
                            || IsWindow(s.owner as HWND) == 0
                            || IsWindowVisible(s.owner as HWND) == 0
                            || s.blur <= 0.0
                        {
                            ShowWindow(hwnd, SW_HIDE);
                            continue;
                        }
                        let band = if s.topmost {
                            HWND_TOPMOST
                        } else {
                            HWND_NOTOPMOST
                        };
                        SetWindowPos(
                            hwnd,
                            band,
                            0,
                            0,
                            0,
                            0,
                            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                        );
                        material(hwnd, s.dark);
                        SetWindowPos(
                            hwnd,
                            s.owner as HWND,
                            s.x,
                            s.y,
                            s.width,
                            s.height,
                            SWP_NOACTIVATE
                                | SWP_SHOWWINDOW
                                | SWP_NOSENDCHANGING
                                | SWP_NOOWNERZORDER,
                        );
                    } else {
                        TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                }
                drop(composition);
                if IsWindow(hwnd) != 0 {
                    DestroyWindow(hwnd);
                }
                UnregisterClassW(class.as_ptr(), instance);
                if initialized {
                    CoUninitialize();
                }
            });
            let (thread_id, native_hwnd) = wait.recv().map_err(|e| e.to_string())??;
            Ok(Self {
                native_hwnd,
                thread_id,
                updates,
                last: None,
                join: Some(join),
                actual_blur,
                owner,
            })
        }
        /// Call after owner movement, resizing, visibility, z-band, theme or blur changes.
        pub fn update(
            &mut self,
            visible: bool,
            topmost: bool,
            dark: bool,
            blur: f32,
            inset: i32,
            radius: f32,
        ) -> Result<(), String> {
            unsafe {
                let owner = self.owner as HWND;
                if IsWindow(owner) == 0 {
                    return Err("材质所有者窗口已关闭".into());
                }
                let mut client: RECT = std::mem::zeroed();
                let mut point = POINT { x: 0, y: 0 };
                if GetClientRect(owner, &mut client) == 0 || ClientToScreen(owner, &mut point) == 0
                {
                    return Err("无法读取材质边界".into());
                }
                let inset = inset.max(0);
                let snapshot = Snapshot {
                    owner: self.owner,
                    x: point.x + inset,
                    y: point.y + inset,
                    width: (client.right - inset * 2).max(1),
                    height: (client.bottom - inset * 2).max(1),
                    visible: visible && IsWindowVisible(owner) != 0 && IsIconic(owner) == 0,
                    topmost,
                    dark,
                    blur: if blur.is_finite() {
                        blur.clamp(0.0, 64.0)
                    } else {
                        0.0
                    },
                    radius: if radius.is_finite() {
                        radius.clamp(0.0, 512.0)
                    } else {
                        0.0
                    },
                };
                if self.last.as_ref() == Some(&snapshot) {
                    return Ok(());
                }
                self.updates
                    .send(snapshot.clone())
                    .map_err(|e| e.to_string())?;
                if PostThreadMessageW(self.thread_id, WM_APP + 21, 0, 0) == 0 {
                    return Err("材质线程已关闭".into());
                }
                self.last = Some(snapshot);
                Ok(())
            }
        }
        pub fn invalidate(&mut self) {
            self.last = None;
        }
        pub fn status(&self) -> BackdropStatus {
            unsafe {
                let hwnd = self.native_hwnd as HWND;
                let mut rect: RECT = std::mem::zeroed();
                GetWindowRect(hwnd, &mut rect);
                BackdropStatus {
                    window: self.native_hwnd,
                    visible: IsWindowVisible(hwnd) != 0,
                    topmost: GetWindowLongW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOPMOST != 0,
                    bounds: [
                        rect.left,
                        rect.top,
                        rect.right - rect.left,
                        rect.bottom - rect.top,
                    ],
                    blur: self.actual_blur.lock().map(|v| *v).unwrap_or(-1.0),
                }
            }
        }
    }
    impl Drop for Backdrop {
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
#[cfg(windows)]
pub use native::{Backdrop, BackdropStatus};
