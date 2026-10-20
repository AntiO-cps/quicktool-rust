//! Native Windows integrations. All slow discovery runs off the UI thread.
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppEntry {
    pub name: String,
    pub kind: String,
    pub target: String,
    pub args: String,
    pub source: String,
    pub icon_path: String,
    pub icon_resource: String,
}
impl AppEntry {
    fn new(name: impl Into<String>, target: impl Into<String>, source: &str) -> Self {
        Self {
            name: name.into(),
            target: target.into(),
            kind: "exe".into(),
            args: String::new(),
            source: source.into(),
            icon_path: String::new(),
            icon_resource: String::new(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegistryEntry {
    pub kind: String,
    pub path: String,
    pub name: String,
    pub data: String,
    pub value_type: String,
    pub provider: String,
}

pub enum Request {
    Shutdown,
    Apps {
        refresh: bool,
    },
    Registry {
        query: String,
        limit: usize,
        root: String,
    },
}
pub enum Response {
    Apps(Vec<AppEntry>),
    Registry(Vec<RegistryEntry>),
}
pub struct Backend {
    requests: mpsc::Sender<(u64, Request)>,
    pub responses: mpsc::Receiver<(u64, Response)>,
    next: u64,
    registry_revision: Arc<std::sync::atomic::AtomicU64>,
}
impl Backend {
    pub fn new(wake: impl Fn() + Send + 'static) -> Self {
        Self::with_optional_cache(None, wake)
    }
    pub fn with_cache(root: PathBuf, wake: impl Fn() + Send + 'static) -> Self {
        Self::with_optional_cache(Some(root.join("app-index.json")), wake)
    }
    fn with_optional_cache(cache_path: Option<PathBuf>, wake: impl Fn() + Send + 'static) -> Self {
        let (tx, rx) = mpsc::channel::<(u64, Request)>();
        let (out, responses) = mpsc::channel();
        let registry_revision = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let revision = registry_revision.clone();
        let watcher_tx = tx.clone();
        let refresh_pending = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let pending = refresh_pending.clone();
        std::thread::spawn(move || {
            let mut previous_refresh = Instant::now() - Duration::from_secs(1);
            let _watcher = watch_apps(move || {
                if !pending.swap(true, std::sync::atomic::Ordering::Relaxed) {
                    let _ = watcher_tx.send((0, Request::Apps { refresh: true }));
                }
            })
            .ok();
            if let Some(path) = cache_path.as_ref() {
                if let Ok(bytes) = std::fs::read(path) {
                    if bytes.len() <= 16 * 1024 * 1024 {
                        if let Ok(items) = serde_json::from_slice::<Vec<AppEntry>>(&bytes) {
                            let _ = out.send((0, Response::Apps(items)));
                            wake();
                        }
                    }
                }
            }
            while let Ok((id, request)) = rx.recv() {
                let response = match request {
                    Request::Shutdown => break,
                    Request::Apps { refresh } => {
                        if id == 0 {
                            refresh_pending.store(false, std::sync::atomic::Ordering::Relaxed);
                            if previous_refresh.elapsed() < Duration::from_millis(650) {
                                continue;
                            }
                        }
                        previous_refresh = Instant::now();
                        let apps = app_index_cached(refresh);
                        if let Some(path) = cache_path.as_ref() {
                            if let Ok(bytes) = serde_json::to_vec(&apps) {
                                let _ = crate::storage::atomic_write(path, &bytes);
                            }
                        }
                        Response::Apps(apps)
                    }
                    Request::Registry { query, limit, root } => {
                        let cancelled =
                            || revision.load(std::sync::atomic::Ordering::Relaxed) != id;
                        if cancelled() {
                            continue;
                        }
                        #[cfg(windows)]
                        let items =
                            native::search_registry_cancellable(&query, limit, &root, cancelled);
                        #[cfg(not(windows))]
                        let items = search_registry(&query, limit, &root);
                        Response::Registry(items)
                    }
                };
                if out.send((id, response)).is_err() {
                    break;
                }
                wake();
            }
        });
        Self {
            requests: tx,
            responses,
            next: 0,
            registry_revision,
        }
    }
    pub fn request(&mut self, request: Request) -> u64 {
        self.next += 1;
        if matches!(&request, Request::Registry { .. }) {
            self.registry_revision
                .store(self.next, std::sync::atomic::Ordering::Relaxed);
        }
        let _ = self.requests.send((self.next, request));
        self.next
    }
}
impl Drop for Backend {
    fn drop(&mut self) {
        self.registry_revision
            .store(u64::MAX, std::sync::atomic::Ordering::Relaxed);
        let _ = self.requests.send((0, Request::Shutdown));
    }
}

type AppCache = Option<(Instant, Vec<AppEntry>)>;
static APPS: std::sync::OnceLock<Mutex<AppCache>> = std::sync::OnceLock::new();
pub fn app_index() -> Vec<AppEntry> {
    app_index_cached(false)
}
pub fn app_index_cached(refresh: bool) -> Vec<AppEntry> {
    let cache = APPS.get_or_init(|| Mutex::new(None));
    if !refresh {
        if let Ok(c) = cache.lock() {
            if let Some((at, items)) = c.as_ref() {
                if at.elapsed() < Duration::from_secs(30) {
                    return items.clone();
                }
            }
        }
    }
    let result = discover_apps();
    if let Ok(mut c) = cache.lock() {
        *c = Some((Instant::now(), result.clone()));
    }
    result
}
pub fn app_watch_roots() -> Vec<PathBuf> {
    let mut roots = crate::platform::known_folders()
        .into_iter()
        .filter(|(n, _)| {
            matches!(
                n.as_str(),
                "Desktop" | "PublicDesktop" | "Programs" | "CommonPrograms"
            )
        })
        .map(|(_, p)| p)
        .collect::<Vec<_>>();
    if let Some(path) = std::env::var_os("PATH") {
        roots.extend(std::env::split_paths(&path));
    }
    let mut seen = HashSet::new();
    roots.retain(|p| p.is_dir() && seen.insert(p.to_string_lossy().to_lowercase()));
    roots
}
pub fn watch_apps(
    wake: impl Fn() + Send + Sync + 'static,
) -> Result<notify::RecommendedWatcher, String> {
    use notify::Watcher;
    let wake = Arc::new(wake);
    let mut watcher =
        notify::recommended_watcher(move |event: Result<notify::Event, notify::Error>| {
            if event.is_ok() {
                if let Some(c) = APPS.get() {
                    if let Ok(mut c) = c.lock() {
                        *c = None;
                    }
                }
                wake();
            }
        })
        .map_err(|e| e.to_string())?;
    for root in app_watch_roots() {
        let recursive = crate::platform::known_folders().iter().any(|(name, path)| {
            matches!(
                name.as_str(),
                "Desktop" | "PublicDesktop" | "Programs" | "CommonPrograms"
            ) && path == &root
        });
        let _ = watcher.watch(
            &root,
            if recursive {
                notify::RecursiveMode::Recursive
            } else {
                notify::RecursiveMode::NonRecursive
            },
        );
    }
    Ok(watcher)
}
fn discover_apps() -> Vec<AppEntry> {
    let mut out = Vec::new();
    #[cfg(windows)]
    native::registered_apps(&mut out);
    for (name, root) in crate::platform::known_folders() {
        if !matches!(
            name.as_str(),
            "Desktop" | "PublicDesktop" | "Programs" | "CommonPrograms"
        ) {
            continue;
        }
        for entry in walkdir::WalkDir::new(root)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
        {
            if !entry.file_type().is_file() {
                continue;
            }
            let p = entry.path();
            let ext = p
                .extension()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase();
            if !["lnk", "url", "appref-ms", "exe"].contains(&ext.as_str()) {
                continue;
            }
            let mut app = AppEntry::new(
                p.file_stem().unwrap_or_default().to_string_lossy(),
                p.to_string_lossy(),
                "Shortcut",
            );
            app.icon_path = p.to_string_lossy().into();
            if ext == "url" {
                if let Ok(text) = std::fs::read_to_string(p) {
                    if let Some(url) = text.lines().find_map(|s| s.strip_prefix("URL=")) {
                        app.target = url.trim().into();
                        app.kind = "url".into();
                    }
                }
            } else if ext == "appref-ms" {
                app.kind = "file".into();
            }
            out.push(app);
        }
    }
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for p in entries.filter_map(Result::ok).map(|e| e.path()) {
                    if p.is_file()
                        && p.extension().is_some_and(|e| {
                            ["exe", "com", "bat", "cmd", "cpl", "msc"]
                                .iter()
                                .any(|x| e.eq_ignore_ascii_case(x))
                        })
                    {
                        out.push(AppEntry::new(
                            p.file_stem().unwrap_or_default().to_string_lossy(),
                            p.to_string_lossy(),
                            "PATH",
                        ));
                    }
                }
            }
        }
    }
    let mut seen = HashSet::new();
    out.retain(|a| {
        !a.target.is_empty() && seen.insert(format!("{}|{}", a.name, a.target).to_lowercase())
    });
    out
}
pub fn run_shell(kind: &str, command: &str, cwd: Option<&str>, admin: bool) -> Result<(), String> {
    if let Some(cwd) = cwd {
        if !Path::new(cwd).is_dir() {
            return Err("工作目录不存在".into());
        }
    }
    let (exe, args) = match kind {
        "powershell" => (
            "powershell.exe",
            if command.trim().is_empty() {
                "-NoExit".into()
            } else {
                format!("-NoExit -Command {command}")
            },
        ),
        "terminal" => (
            "wt.exe",
            if command.trim().is_empty() {
                cwd.map(|p| format!("-d {}", quote_arg(p)))
                    .unwrap_or_default()
            } else {
                format!("cmd.exe /k {command}")
            },
        ),
        _ => ("cmd.exe", format!("/k {command}")),
    };
    crate::platform::open(exe, Some(&args), cwd, admin)
}
pub fn launch_app(app: &AppEntry, admin: bool) -> Result<(), String> {
    if app.kind == "appx" {
        crate::platform::open("explorer.exe", Some(&quote_arg(&app.target)), None, false)
    } else {
        crate::platform::open(
            &app.target,
            (!app.args.is_empty()).then_some(app.args.as_str()),
            None,
            admin,
        )
    }
}
/// Copy a bundled country flag. No network or shell process is involved.
pub fn copy_flag(code: &str, format: &str) -> Result<(), String> {
    if code.len() != 2 || !code.bytes().all(|c| c.is_ascii_alphabetic()) {
        return Err("国家代码无效".into());
    }
    let code = code.to_ascii_lowercase();
    if format == "emoji" {
        return crate::platform::copy_text(
            &code
                .chars()
                .filter_map(|c| char::from_u32(0x1f1e6 + c as u32 - 'a' as u32))
                .collect::<String>(),
        );
    }
    let ext = match format {
        "svg" => "svg",
        "png" => "png",
        _ => return Err("旗帜格式无效".into()),
    };
    let base = crate::resources::asset("assets/flags");
    let path = base
        .join(if ext == "png" { "png250" } else { "svg" })
        .join(format!("{code}.{ext}"));
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    #[cfg(windows)]
    if ext == "svg" {
        return crate::platform::copy_bytes("image/svg+xml", &bytes);
    }
    let i = image::load_from_memory(&bytes)
        .map_err(|e| e.to_string())?
        .to_rgba8();
    crate::platform::copy_image(i.width(), i.height(), i.as_raw())
}
pub fn quote_arg(s: &str) -> String {
    let mut out = String::from("\"");
    let mut slashes = 0;
    for c in s.chars() {
        if c == '\\' {
            slashes += 1;
        } else {
            if c == '"' {
                out.push_str(&"\\".repeat(slashes * 2 + 1));
            } else {
                out.push_str(&"\\".repeat(slashes));
            }
            out.push(c);
            slashes = 0;
        }
    }
    out.push_str(&"\\".repeat(slashes * 2));
    out.push('"');
    out
}
#[cfg(windows)]
pub use native::{dark_theme, open_registry_at, search_registry};
#[cfg(not(windows))]
pub fn search_registry(_: &str, _: usize, _: &str) -> Vec<RegistryEntry> {
    Vec::new()
}
#[cfg(not(windows))]
pub fn open_registry_at(_: &str) -> Result<(), String> {
    Err("此功能需要 Windows".into())
}
#[cfg(not(windows))]
pub fn dark_theme() -> bool {
    true
}

#[cfg(windows)]
mod native {
    use super::*;
    use std::{
        ffi::c_void,
        ptr::{null, null_mut},
    };
    use windows_sys::Win32::{
        System::{Com::*, Registry::*},
        UI::Shell::*,
    };
    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }
    struct Key(HKEY);
    impl Drop for Key {
        fn drop(&mut self) {
            unsafe {
                RegCloseKey(self.0);
            }
        }
    }
    fn key(root: HKEY, path: &str) -> Option<Key> {
        let mut h = null_mut();
        (unsafe { RegOpenKeyExW(root, wide(path).as_ptr(), 0, KEY_READ, &mut h) } == 0)
            .then_some(Key(h))
    }
    fn names(h: HKEY) -> Vec<String> {
        let mut out = Vec::new();
        for i in 0..100_000 {
            let mut buf = vec![0u16; 32768];
            let mut n = buf.len() as u32;
            let status = unsafe {
                RegEnumKeyExW(
                    h,
                    i,
                    buf.as_mut_ptr(),
                    &mut n,
                    null(),
                    null_mut(),
                    null_mut(),
                    null_mut(),
                )
            };
            if status == 259 {
                break;
            }
            if status == 0 {
                out.push(String::from_utf16_lossy(&buf[..n as usize]));
            }
        }
        out
    }
    fn values(h: HKEY) -> Vec<(String, u32, Vec<u8>)> {
        let mut out = Vec::new();
        for i in 0..100_000 {
            let mut name = vec![0u16; 16384];
            let mut n = name.len() as u32;
            let (mut typ, mut size) = (0, 0);
            let r = unsafe {
                RegEnumValueW(
                    h,
                    i,
                    name.as_mut_ptr(),
                    &mut n,
                    null(),
                    &mut typ,
                    null_mut(),
                    &mut size,
                )
            };
            if r == 259 {
                break;
            }
            if r != 0 && r != 234 {
                continue;
            }
            if size > 16 * 1024 * 1024 {
                continue;
            }
            let mut data = vec![0u8; size as usize];
            n = name.len() as u32;
            if unsafe {
                RegEnumValueW(
                    h,
                    i,
                    name.as_mut_ptr(),
                    &mut n,
                    null(),
                    &mut typ,
                    data.as_mut_ptr(),
                    &mut size,
                )
            } == 0
            {
                data.truncate(size as usize);
                out.push((String::from_utf16_lossy(&name[..n as usize]), typ, data));
            }
        }
        out
    }
    fn display(typ: u32, data: &[u8]) -> String {
        match typ {
            REG_SZ | REG_EXPAND_SZ | REG_MULTI_SZ => String::from_utf16_lossy(
                &data
                    .chunks_exact(2)
                    .map(|c| u16::from_le_bytes([c[0], c[1]]))
                    .collect::<Vec<_>>(),
            )
            .trim_end_matches('\0')
            .replace('\0', "; "),
            REG_DWORD if data.len() >= 4 => {
                format!("0x{:x}", u32::from_le_bytes(data[..4].try_into().unwrap()))
            }
            REG_QWORD if data.len() >= 8 => {
                format!("0x{:x}", u64::from_le_bytes(data[..8].try_into().unwrap()))
            }
            _ => data.iter().take(4096).map(|b| format!("{b:02x}")).collect(),
        }
    }
    fn text(h: HKEY, name: &str) -> Option<String> {
        values(h)
            .into_iter()
            .find(|(n, _, _)| n == name)
            .map(|(_, t, d)| display(t, &d))
    }
    pub fn search_registry(query: &str, limit: usize, root: &str) -> Vec<RegistryEntry> {
        search_registry_cancellable(query, limit, root, || false)
    }
    pub(super) fn search_registry_cancellable(
        query: &str,
        limit: usize,
        root: &str,
        cancelled: impl Fn() -> bool,
    ) -> Vec<RegistryEntry> {
        let query = query.trim().to_lowercase();
        if query.is_empty() {
            return Vec::new();
        }
        let limit = limit.clamp(1, 100);
        let roots = match root.to_ascii_uppercase().as_str() {
            "HKCU" => vec![(HKEY_CURRENT_USER, "HKEY_CURRENT_USER")],
            "HKLM" => vec![(HKEY_LOCAL_MACHINE, "HKEY_LOCAL_MACHINE")],
            "HKCR" => vec![(HKEY_CLASSES_ROOT, "HKEY_CLASSES_ROOT")],
            _ => vec![
                (HKEY_CURRENT_USER, "HKEY_CURRENT_USER"),
                (HKEY_LOCAL_MACHINE, "HKEY_LOCAL_MACHINE"),
            ],
        };
        let mut out = Vec::new();
        let start = Instant::now();
        for (h, label) in roots {
            let mut pending = vec![String::new()];
            while let Some(path) = pending.pop() {
                if cancelled() || start.elapsed() > Duration::from_secs(6) || out.len() >= limit {
                    return out;
                }
                let Some(k) = key(h, &path) else { continue };
                let full = if path.is_empty() {
                    label.into()
                } else {
                    format!("{label}\\{path}")
                };
                if full.to_lowercase().contains(&query) {
                    out.push(RegistryEntry {
                        kind: "key".into(),
                        path: full.clone(),
                        name: String::new(),
                        data: String::new(),
                        value_type: String::new(),
                        provider: "Registry".into(),
                    });
                }
                for (name, t, data) in values(k.0) {
                    if out.len() >= limit {
                        return out;
                    }
                    let data = display(t, &data);
                    if name.to_lowercase().contains(&query) || data.to_lowercase().contains(&query)
                    {
                        out.push(RegistryEntry {
                            kind: "value".into(),
                            path: full.clone(),
                            name,
                            data,
                            value_type: match t {
                                REG_SZ => "REG_SZ",
                                REG_EXPAND_SZ => "REG_EXPAND_SZ",
                                REG_MULTI_SZ => "REG_MULTI_SZ",
                                REG_DWORD => "REG_DWORD",
                                REG_QWORD => "REG_QWORD",
                                _ => "REG_BINARY",
                            }
                            .into(),
                            provider: "Registry".into(),
                        });
                    }
                }
                let mut children = names(k.0);
                children.reverse();
                pending.extend(children.into_iter().map(|n| {
                    if path.is_empty() {
                        n
                    } else {
                        format!("{path}\\{n}")
                    }
                }));
            }
        }
        out
    }
    pub fn open_registry_at(path: &str) -> Result<(), String> {
        let normalized = path.to_ascii_uppercase();
        if ![
            "HKEY_CURRENT_USER",
            "HKEY_LOCAL_MACHINE",
            "HKEY_CLASSES_ROOT",
            "HKEY_USERS",
            "HKEY_CURRENT_CONFIG",
        ]
        .iter()
        .any(|r| normalized == *r || normalized.starts_with(&format!("{r}\\")))
            || path.contains('\0')
        {
            return Err("无效注册表路径".into());
        }
        let mut h = null_mut();
        let r = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                wide("Software\\Microsoft\\Windows\\CurrentVersion\\Applets\\Regedit").as_ptr(),
                0,
                null_mut(),
                0,
                KEY_SET_VALUE,
                null(),
                &mut h,
                null_mut(),
            )
        };
        if r != 0 {
            return Err(format!("注册表错误 {r}"));
        }
        let k = Key(h);
        let value = wide(path);
        let r = unsafe {
            RegSetValueExW(
                k.0,
                wide("LastKey").as_ptr(),
                0,
                REG_SZ,
                value.as_ptr() as *const u8,
                (value.len() * 2) as u32,
            )
        };
        if r != 0 {
            return Err(format!("注册表错误 {r}"));
        }
        crate::platform::open("regedit.exe", None, None, false)
    }
    pub fn dark_theme() -> bool {
        key(
            HKEY_CURRENT_USER,
            "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize",
        )
        .and_then(|k| {
            values(k.0)
                .into_iter()
                .find(|(n, _, _)| n == "AppsUseLightTheme")
        })
        .is_some_and(|(_, _, d)| {
            d.len() >= 4 && u32::from_le_bytes(d[..4].try_into().unwrap()) == 0
        })
    }
    pub fn registered_apps(out: &mut Vec<AppEntry>) {
        shell_apps(out);
        for root in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
            for prefix in [
                "Software\\Microsoft\\Windows\\CurrentVersion",
                "Software\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion",
            ] {
                for (section, source) in [
                    ("App Paths", "AppPaths"),
                    ("Uninstall", "UninstallRegistry"),
                ] {
                    let path = format!("{prefix}\\{section}");
                    let Some(k) = key(root, &path) else { continue };
                    for name in names(k.0) {
                        let Some(child) = key(root, &format!("{path}\\{name}")) else {
                            continue;
                        };
                        if section == "App Paths" {
                            if let Some(target) = text(child.0, "") {
                                out.push(AppEntry::new(
                                    name.trim_end_matches(".exe"),
                                    target.trim_matches('"'),
                                    source,
                                ));
                            }
                        } else if let (Some(name), Some(icon)) =
                            (text(child.0, "DisplayName"), text(child.0, "DisplayIcon"))
                        {
                            let target = icon
                                .rsplit_once(',')
                                .filter(|(_, s)| s.trim().parse::<i32>().is_ok())
                                .map_or(icon.as_str(), |(s, _)| s)
                                .trim()
                                .trim_matches('"');
                            let p = Path::new(target);
                            if p.is_file()
                                && p.extension().is_some_and(|x| x.eq_ignore_ascii_case("exe"))
                                && !p
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .to_lowercase()
                                    .starts_with("unins")
                            {
                                let mut a = AppEntry::new(name, target, source);
                                a.icon_resource = icon;
                                out.push(a);
                            }
                        }
                    }
                }
            }
        }
    }
    // IShellItem/IEnumShellItems COM ABI. Enumerating AppsFolder includes packaged UWP apps and registered StartApps.
    unsafe fn method<T: Copy>(obj: *mut c_void, index: usize) -> T {
        unsafe { std::mem::transmute_copy(&*(*(obj as *mut *mut *mut c_void)).add(index)) }
    }
    unsafe fn release(obj: *mut c_void) {
        if !obj.is_null() {
            let f: unsafe extern "system" fn(*mut c_void) -> u32 = unsafe { method(obj, 2) };
            unsafe {
                f(obj);
            }
        }
    }
    fn shell_apps(out: &mut Vec<AppEntry>) {
        unsafe {
            let hr = CoInitializeEx(null(), COINIT_APARTMENTTHREADED as u32);
            let initialized = hr >= 0;
            let iid = windows_sys::core::GUID::from_u128(0x43826d1e_e718_42ee_bc55_a1e261c37bfe);
            let enum_iid =
                windows_sys::core::GUID::from_u128(0x70629033_e363_4a28_a567_0db78006e6d7);
            let mut folder = null_mut();
            if SHCreateItemFromParsingName(
                wide("shell:AppsFolder").as_ptr(),
                null_mut(),
                &iid,
                &mut folder,
            ) >= 0
            {
                let bind: unsafe extern "system" fn(
                    *mut c_void,
                    *mut c_void,
                    *const windows_sys::core::GUID,
                    *const windows_sys::core::GUID,
                    *mut *mut c_void,
                ) -> i32 = method(folder, 3);
                let mut iter = null_mut();
                if bind(folder, null_mut(), &BHID_EnumItems, &enum_iid, &mut iter) >= 0 {
                    let next: unsafe extern "system" fn(
                        *mut c_void,
                        u32,
                        *mut *mut c_void,
                        *mut u32,
                    ) -> i32 = method(iter, 3);
                    loop {
                        let mut item = null_mut();
                        let mut fetched = 0;
                        if next(iter, 1, &mut item, &mut fetched) != 0 || fetched == 0 {
                            break;
                        }
                        let get: unsafe extern "system" fn(*mut c_void, u32, *mut *mut u16) -> i32 =
                            method(item, 5);
                        let mut name = null_mut();
                        let mut target = null_mut();
                        if get(item, 0, &mut name) >= 0 && get(item, 0x80018001, &mut target) >= 0 {
                            let read = |p: *mut u16| {
                                let mut n = 0;
                                while *p.add(n) != 0 {
                                    n += 1
                                }
                                String::from_utf16_lossy(std::slice::from_raw_parts(p, n))
                            };
                            let target = read(target);
                            let mut a = AppEntry::new(
                                read(name),
                                if target.starts_with("shell:") {
                                    target.clone()
                                } else {
                                    format!("shell:AppsFolder\\{target}")
                                },
                                "StartApps",
                            );
                            a.kind = "appx".into();
                            out.push(a);
                        }
                        CoTaskMemFree(name as _);
                        CoTaskMemFree(target as _);
                        release(item);
                    }
                    release(iter);
                }
                release(folder);
            }
            if initialized {
                CoUninitialize();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quotes_windows_paths() {
        assert_eq!(quote_arg("C:\\A B\\"), "\"C:\\A B\\\\\"");
        assert_eq!(quote_arg("a\"b"), "\"a\\\"b\"");
    }
    #[test]
    fn empty_registry_search_is_instant() {
        assert!(search_registry("  ", 10, "HKCU").is_empty());
    }
    #[test]
    fn invalid_registry_path_is_rejected() {
        assert!(open_registry_at("HKCU;bad").is_err());
    }
}
