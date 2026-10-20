//! Real Explorer icon extraction with bounded, modification-sensitive caching.
use std::{
    collections::HashMap,
    path::Path,
    time::{Duration, Instant, SystemTime},
};
#[derive(Clone)]
pub struct ShellIcon {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}
struct Entry {
    modified: Option<SystemTime>,
    size: u64,
    customized: Option<SystemTime>,
    at: Instant,
    icon: ShellIcon,
}
#[derive(Default)]
pub struct IconCache {
    items: HashMap<String, Entry>,
}
impl IconCache {
    pub fn get_target(&mut self, target: &str) -> Result<ShellIcon, String> {
        if !target.starts_with("shell:") {
            return self.get(Path::new(target));
        }
        let key = target.to_lowercase();
        if let Some(e) = self.items.get(&key) {
            if e.at.elapsed() < Duration::from_secs(30) {
                return Ok(e.icon.clone());
            }
        }
        let icon = extract(Path::new(target))?;
        if self.items.len() >= 256 {
            if let Some(key) = self
                .items
                .iter()
                .min_by_key(|(_, e)| e.at)
                .map(|(k, _)| k.clone())
            {
                self.items.remove(&key);
            }
        }
        self.items.insert(
            key,
            Entry {
                modified: None,
                size: 0,
                customized: None,
                at: Instant::now(),
                icon: icon.clone(),
            },
        );
        Ok(icon)
    }
    pub fn get(&mut self, path: &Path) -> Result<ShellIcon, String> {
        let meta = path.metadata().map_err(|e| e.to_string())?;
        let key = path.to_string_lossy().to_lowercase();
        let modified = meta.modified().ok();
        let customized = if meta.is_dir() {
            path.join("desktop.ini")
                .metadata()
                .and_then(|m| m.modified())
                .ok()
        } else {
            None
        };
        if let Some(e) = self.items.get(&key) {
            if e.modified == modified
                && e.size == meta.len()
                && e.customized == customized
                && e.at.elapsed() < Duration::from_secs(30)
            {
                return Ok(e.icon.clone());
            }
        }
        let icon = extract(path)?;
        if self.items.len() >= 256 {
            if let Some(key) = self
                .items
                .iter()
                .min_by_key(|(_, e)| e.at)
                .map(|(k, _)| k.clone())
            {
                self.items.remove(&key);
            }
        }
        self.items.insert(
            key,
            Entry {
                modified,
                size: meta.len(),
                customized,
                at: Instant::now(),
                icon: icon.clone(),
            },
        );
        Ok(icon)
    }
}
#[cfg(windows)]
fn extract(path: &Path) -> Result<ShellIcon, String> {
    use std::ptr::null_mut;
    use windows_sys::Win32::{
        Graphics::Gdi::*,
        UI::{Shell::*, WindowsAndMessaging::*},
    };
    let path: Vec<u16> = path
        .as_os_str()
        .to_string_lossy()
        .replace('/', "\\")
        .encode_utf16()
        .chain(Some(0))
        .collect();
    unsafe {
        use windows_sys::Win32::System::Com::*;
        struct Apartment(bool);
        impl Drop for Apartment {
            fn drop(&mut self) {
                if self.0 {
                    unsafe {
                        CoUninitialize();
                    }
                }
            }
        }
        let _apartment =
            Apartment(CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32) >= 0);
        let mut info: SHFILEINFOW = std::mem::zeroed();
        let mut pidl = std::ptr::null_mut();
        let parsed = SHParseDisplayName(
            path.as_ptr(),
            std::ptr::null_mut(),
            &mut pidl,
            0,
            std::ptr::null_mut(),
        ) >= 0;
        let result = SHGetFileInfoW(
            if parsed {
                pidl as *const u16
            } else {
                path.as_ptr()
            },
            0,
            &mut info,
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_LARGEICON | if parsed { SHGFI_PIDL } else { 0 },
        );
        if parsed {
            CoTaskMemFree(pidl as _);
        }
        if result == 0 || info.hIcon.is_null() {
            return Err(format!(
                "无法读取 Shell 图标：{}",
                std::io::Error::last_os_error()
            ));
        }
        let dc = CreateCompatibleDC(null_mut());
        if dc.is_null() {
            DestroyIcon(info.hIcon);
            return Err("无法创建图标画布".into());
        }
        let mut bmi: BITMAPINFO = std::mem::zeroed();
        bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = 32;
        bmi.bmiHeader.biHeight = -32;
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = BI_RGB;
        let mut bits = null_mut();
        let bitmap = CreateDIBSection(dc, &bmi, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
        if bitmap.is_null() {
            DeleteDC(dc);
            DestroyIcon(info.hIcon);
            return Err("无法创建图标位图".into());
        }
        let old = SelectObject(dc, bitmap);
        std::ptr::write_bytes(bits, 0, 4096);
        let ok = DrawIconEx(dc, 0, 0, info.hIcon, 32, 32, 0, null_mut(), DI_NORMAL);
        let mut rgba = std::slice::from_raw_parts(bits as *const u8, 4096).to_vec();
        for p in rgba.chunks_exact_mut(4) {
            p.swap(0, 2);
            if p[3] > 0 && p[3] < 255 {
                let a = p[3] as u32;
                for c in &mut p[..3] {
                    *c = ((*c as u32 * 255 / a).min(255)) as u8;
                }
            }
        }
        SelectObject(dc, old);
        DeleteObject(bitmap);
        DeleteDC(dc);
        DestroyIcon(info.hIcon);
        if ok == 0 {
            Err("图标绘制失败".into())
        } else {
            Ok(ShellIcon {
                width: 32,
                height: 32,
                rgba,
            })
        }
    }
}
#[cfg(not(windows))]
fn extract(_: &Path) -> Result<ShellIcon, String> {
    Err("此功能需要 Windows".into())
}
