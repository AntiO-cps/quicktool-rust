use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HistoryItem {
    pub id: String,
    pub input: String,
    pub tool: String,
    pub title: String,
    pub value: String,
    pub pinned: bool,
    pub created: u64,
    #[serde(default, rename = "pinState", skip_serializing_if = "Option::is_none")]
    pub pin_state: Option<crate::pins::PinRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<HistoryMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_asset: Option<String>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryMode {
    pub tool_id: String,
    #[serde(default)]
    pub args: Vec<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub submode: Option<String>,
}
pub struct Store {
    pub root: PathBuf,
    pub settings: Value,
    pub history: Vec<HistoryItem>,
    pub load_errors: Vec<String>,
    pub usage: std::collections::BTreeMap<String, Usage>,
    usage_blocked: bool,
    settings_blocked: bool,
    history_blocked: bool,
    pin_worker: Option<crate::pin_storage::Worker>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Usage {
    pub count: u64,
    pub last: u64,
}
fn read<T: serde::de::DeserializeOwned>(path: &Path) -> io::Result<Option<T>> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(io::Error::other),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}
pub fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("Missing parent"))?;
    fs::create_dir_all(parent)?;
    let mut random = [0u8; 8];
    getrandom::fill(&mut random).map_err(|e| io::Error::other(e.to_string()))?;
    let tmp = parent.join(format!(".quicktool-{:x}.tmp", u64::from_le_bytes(random)));
    let result = (|| {
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            let src: Vec<u16> = tmp.as_os_str().encode_wide().chain(Some(0)).collect();
            let dst: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
            unsafe extern "system" {
                fn MoveFileExW(a: *const u16, b: *const u16, flags: u32) -> i32;
            }
            if unsafe { MoveFileExW(src.as_ptr(), dst.as_ptr(), 0x1 | 0x8) } == 0 {
                return Err(io::Error::last_os_error());
            }
        }
        #[cfg(not(windows))]
        fs::rename(&tmp, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(tmp);
    }
    result
}
impl Store {
    pub fn open(root: PathBuf) -> Self {
        let mut settings: Value =
            serde_json::from_str(include_str!("../assets/defaults.json")).unwrap();
        let mut load_errors = Vec::new();
        let mut settings_blocked = false;
        let mut history_blocked = false;
        match read(&root.join("settings.json")) {
            Ok(Some(Value::Object(old))) => settings.as_object_mut().unwrap().extend(old),
            Ok(None) => {}
            Ok(Some(_)) => {
                settings_blocked = true;
                load_errors.push("settings.json 格式无效，已保留原文件并禁止覆盖".into());
            }
            Err(e) => {
                settings_blocked = true;
                load_errors.push(format!("settings.json 读取失败，已禁止覆盖：{e}"));
            }
        }
        let history = match read(&root.join("history.json")) {
            Ok(history) => history.unwrap_or_default(),
            Err(e) => {
                history_blocked = true;
                load_errors.push(format!("history.json 读取失败，已禁止覆盖：{e}"));
                vec![]
            }
        };
        let (usage, usage_blocked) = match read(&root.join("usage.json")) {
            Ok(value) => (value.unwrap_or_default(), false),
            Err(error) => {
                load_errors.push(format!("usage.json 读取失败，已禁止覆盖：{error}"));
                (std::collections::BTreeMap::new(), true)
            }
        };
        Self {
            root,
            settings,
            history,
            load_errors,
            usage,
            usage_blocked,
            settings_blocked,
            history_blocked,
            pin_worker: None,
        }
    }
    pub fn default_root() -> PathBuf {
        let mut args = std::env::args_os();
        while let Some(arg) = args.next() {
            if arg == "--data-dir" {
                if let Some(directory) = args.next().filter(|value| !value.is_empty()) {
                    return PathBuf::from(directory);
                }
            }
        }
        std::env::var_os("QUICKTOOL_RUST_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                std::env::var_os("APPDATA")
                    .map(PathBuf::from)
                    .unwrap_or_else(std::env::temp_dir)
                    .join("QuickTool-Rust")
            })
    }
    pub fn usage_boost(&self, id: &str) -> f64 {
        let Some(u) = self.usage.get(id) else {
            return 0.;
        };
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let frequency = ((u.count as f64 + 1.).log2() / 6.).min(1.);
        let age = (now as f64 - u.last as f64) / 86400000.;
        frequency * 0.18
            + if u.last > 0 {
                (1. - age / 30.).max(0.) * 0.08
            } else {
                0.
            }
    }
    pub fn mark_used(&mut self, id: &str) -> io::Result<()> {
        if self.usage_blocked {
            return Err(io::Error::other("使用记录源文件读取失败，禁止覆盖"));
        }
        let previous = self.usage.clone();
        let item = self.usage.entry(id.into()).or_default();
        item.count = item.count.saturating_add(1);
        item.last = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        if let Err(error) = atomic_write(
            &self.root.join("usage.json"),
            &serde_json::to_vec(&self.usage)?,
        ) {
            self.usage = previous;
            return Err(error);
        }
        Ok(())
    }
    pub fn save_settings(&self) -> io::Result<()> {
        if self.settings_blocked {
            return Err(io::Error::other("设置源文件读取失败，禁止覆盖"));
        }
        atomic_write(
            &self.root.join("settings.json"),
            &serde_json::to_vec_pretty(&self.settings)?,
        )
    }
    pub fn save_history(&self) -> io::Result<()> {
        if self.history_blocked {
            return Err(io::Error::other("历史源文件读取失败，禁止覆盖"));
        }
        atomic_write(
            &self.root.join("history.json"),
            &serde_json::to_vec(&self.history)?,
        )
    }
    pub fn add(&mut self, input: &str, tool: &str, title: &str, value: &str) -> io::Result<()> {
        self.add_with_mode(input, tool, title, value, None)
    }
    pub fn add_with_mode(
        &mut self,
        input: &str,
        tool: &str,
        title: &str,
        value: &str,
        mode: Option<HistoryMode>,
    ) -> io::Result<()> {
        if self.history_blocked {
            return Err(io::Error::other("历史源文件读取失败，禁止覆盖"));
        }
        let previous = self.history.clone();
        let created = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let mut random = [0u8; 8];
        getrandom::fill(&mut random).map_err(|e| io::Error::other(e.to_string()))?;
        self.history.insert(
            0,
            HistoryItem {
                id: format!("{created}-{:x}", u64::from_le_bytes(random)),
                input: input.into(),
                tool: tool.into(),
                title: title.into(),
                value: value.into(),
                pinned: false,
                created,
                pin_state: None,
                thumbnail: None,
                mode,
                image_asset: None,
            },
        );
        let limit = self.settings["historyLimit"]
            .as_f64()
            .unwrap_or(500.)
            .round()
            .clamp(0., 10000.) as usize;
        let mut count = 0;
        self.history.retain(|h| {
            if h.pinned {
                true
            } else {
                count += 1;
                count <= limit
            }
        });
        if let Err(e) = self.save_history() {
            self.history = previous;
            return Err(e);
        }
        Ok(())
    }
    pub fn upsert_pin(&mut self, update: &crate::pins::PinHistoryUpdate) -> io::Result<()> {
        if self.history_blocked {
            return Err(io::Error::other("历史源文件读取失败，禁止覆盖"));
        }
        let existing = self
            .history
            .iter()
            .find(|h| h.id == update.record.history_id)
            .and_then(|h| h.pin_state.as_ref())
            .and_then(|p| p.asset.clone());
        let prepared = crate::pin_storage::prepare(&self.root, update, existing)?;
        self.apply_pin(prepared)
    }
    pub fn enqueue_pin(
        &mut self,
        update: crate::pins::PinHistoryUpdate,
        wake: impl Fn() + Send + 'static,
    ) -> io::Result<()> {
        if self.history_blocked {
            return Err(io::Error::other("历史源文件读取失败，禁止覆盖"));
        }
        if self.pin_worker.is_none() {
            let existing = self
                .history
                .iter()
                .filter_map(|h| h.pin_state.as_ref())
                .filter_map(|p| p.asset.as_ref().map(|a| (p.history_id.clone(), a.clone())))
                .collect();
            self.pin_worker = Some(crate::pin_storage::Worker::start(
                self.root.clone(),
                existing,
                wake,
            ));
        }
        self.pin_worker.as_ref().unwrap().queue(update)
    }
    /// Encode and write assets on the shared background worker.
    #[allow(clippy::too_many_arguments)]
    pub fn enqueue_image_with_mode(
        &mut self,
        input: &str,
        tool: &str,
        title: &str,
        source: std::sync::Arc<image::RgbaImage>,
        mode: Option<HistoryMode>,
        wake: impl Fn() + Send + 'static,
    ) -> io::Result<String> {
        if self.history_blocked {
            return Err(io::Error::other("历史源文件读取失败，禁止覆盖"));
        }
        let created = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let mut random = [0u8; 8];
        getrandom::fill(&mut random).map_err(|e| io::Error::other(e.to_string()))?;
        let id = format!("{created}-{:x}", u64::from_le_bytes(random));
        let item = HistoryItem {
            id: id.clone(),
            input: input.into(),
            tool: tool.into(),
            title: title.into(),
            value: format!("图片 {} × {}", source.width(), source.height()),
            pinned: false,
            created,
            pin_state: None,
            thumbnail: None,
            mode,
            image_asset: None,
        };
        if self.pin_worker.is_none() {
            let existing = self
                .history
                .iter()
                .filter_map(|h| h.pin_state.as_ref())
                .filter_map(|p| p.asset.as_ref().map(|a| (p.history_id.clone(), a.clone())))
                .collect();
            self.pin_worker = Some(crate::pin_storage::Worker::start(
                self.root.clone(),
                existing,
                wake,
            ));
        }
        self.pin_worker
            .as_ref()
            .unwrap()
            .queue_image(item, source)?;
        Ok(id)
    }
    fn apply_image(&mut self, item: HistoryItem) -> io::Result<()> {
        let previous = self.history.clone();
        let position = self
            .history
            .iter()
            .position(|h| h.created <= item.created)
            .unwrap_or(self.history.len());
        self.history.insert(position, item);
        let limit = self.settings["historyLimit"]
            .as_f64()
            .unwrap_or(500.)
            .round()
            .clamp(0., 10000.) as usize;
        let mut count = 0;
        self.history.retain(|h| {
            if h.pinned {
                true
            } else {
                count += 1;
                count <= limit
            }
        });
        if let Err(e) = self.save_history() {
            self.history = previous;
            return Err(e);
        }
        Ok(())
    }
    pub fn load_history_image(&self, id: &str) -> io::Result<image::RgbaImage> {
        let asset = self
            .history
            .iter()
            .find(|h| h.id == id)
            .and_then(|h| h.image_asset.as_deref())
            .ok_or_else(|| io::Error::other("图片历史原图不存在"))?;
        crate::pin_storage::load_image(&self.root, asset)
    }
    pub fn poll_pin_updates(&mut self) -> Vec<String> {
        let prepared = self
            .pin_worker
            .as_ref()
            .map(|worker| worker.responses.try_iter().collect::<Vec<_>>())
            .unwrap_or_default();
        let mut errors = Vec::new();
        for result in prepared {
            match result {
                Ok(prepared) => {
                    let applied = match prepared {
                        crate::pin_storage::Prepared::Pin(pin) => self.apply_pin(*pin),
                        crate::pin_storage::Prepared::Image(item) => self.apply_image(*item),
                    };
                    if let Err(e) = applied {
                        errors.push(e.to_string());
                    }
                }
                Err(e) => errors.push(e.to_string()),
            }
        }
        errors
    }
    pub fn flush_pin_updates(&mut self) -> Vec<String> {
        let mut errors = Vec::new();
        if let Some(worker) = &self.pin_worker {
            if let Err(e) = worker.flush() {
                errors.push(e.to_string());
            }
        }
        errors.extend(self.poll_pin_updates());
        errors
    }
    pub(crate) fn apply_pin(
        &mut self,
        prepared: crate::pin_storage::PreparedPin,
    ) -> io::Result<()> {
        let previous = self.history.clone();
        let pin = prepared.record;
        let pinned = self
            .history
            .iter()
            .find(|h| h.id == pin.history_id)
            .is_some_and(|h| h.pinned);
        let position = self.history.iter().position(|h| h.id == pin.history_id);
        let title = if pin.kind == "image" {
            format!("图片 Pin · {} × {}", pin.width.round(), pin.height.round())
        } else {
            "文字 Pin".into()
        };
        let value = if pin.kind == "image" {
            format!("图片 {} × {}", pin.width.round(), pin.height.round())
        } else {
            pin.text.clone()
        };
        let item = HistoryItem {
            id: pin.history_id.clone(),
            input: if pin.kind == "image" {
                String::new()
            } else {
                pin.text.clone()
            },
            tool: "pin".into(),
            title,
            value,
            pinned,
            created: pin.created,
            image_asset: None,
            pin_state: Some(pin),
            thumbnail: prepared.thumbnail,
            mode: Some(HistoryMode {
                tool_id: "pin".into(),
                ..Default::default()
            }),
        };
        if let Some(position) = position {
            self.history[position] = item;
        } else {
            self.history.insert(0, item);
        }
        let limit = self.settings["historyLimit"]
            .as_f64()
            .unwrap_or(500.)
            .round()
            .clamp(0., 10000.) as usize;
        let mut count = 0;
        self.history.retain(|h| {
            if h.pinned {
                true
            } else {
                count += 1;
                count <= limit
            }
        });
        if let Err(e) = self.save_history() {
            self.history = previous;
            return Err(e);
        }
        Ok(())
    }
    pub fn load_pin(
        &self,
        id: &str,
    ) -> io::Result<(crate::pins::PinRecord, Option<image::RgbaImage>)> {
        let record = self
            .history
            .iter()
            .find(|h| h.id == id)
            .and_then(|h| h.pin_state.clone())
            .ok_or_else(|| io::Error::other("Pin 历史不存在"))?;
        let source = if record.kind == "image" {
            let asset = record
                .asset
                .as_deref()
                .ok_or_else(|| io::Error::other("Pin 原图资源不存在"))?;
            Some(crate::pin_storage::load_image(&self.root, asset)?)
        } else {
            None
        };
        Ok((record, source))
    }
    pub fn thumbnail_path(&self, id: &str) -> Option<PathBuf> {
        let asset = self
            .history
            .iter()
            .find(|h| h.id == id)?
            .thumbnail
            .as_deref()?;
        crate::pin_storage::asset_path(&self.root, asset, true).ok()
    }
}
