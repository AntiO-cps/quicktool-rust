//! Optional tool modules: a module is installed when
//! `<exe>/assets/modules/<id>/manifest.json` exists. Package payloads always
//! ship under `<exe>/assets/packages/<id>/` so the settings panel can install
//! any module offline. Until `refresh()` runs, every command counts as enabled
//! so tests and probes behave as before.

use serde::Deserialize;
use serde_json::Value;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock, RwLock};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleInfo {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub commands: Vec<String>,
    #[serde(default)]
    pub data_files: Vec<String>,
    #[serde(default)]
    pub size_bytes: u64,
}

#[derive(Debug)]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub commands: Vec<String>,
    pub data_files: Vec<String>,
}

const CATALOG_JSON: &str = include_str!("../assets/module-catalog.json");

fn parse_catalog() -> (Vec<String>, Vec<ModuleInfo>) {
    let value: Value = serde_json::from_str(CATALOG_JSON).expect("module-catalog.json");
    let core = value["core"]
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let modules = serde_json::from_value(value["modules"].clone()).expect("module catalog");
    (core, modules)
}

pub fn catalog() -> &'static [ModuleInfo] {
    static MODULES: OnceLock<Vec<ModuleInfo>> = OnceLock::new();
    MODULES.get_or_init(|| parse_catalog().1)
}

pub fn core_commands() -> &'static [String] {
    static CORE: OnceLock<Vec<String>> = OnceLock::new();
    CORE.get_or_init(|| parse_catalog().0)
}

/// Module owning a command id; `None` means the command is core.
pub fn module_of(command_id: &str) -> Option<&'static ModuleInfo> {
    catalog()
        .iter()
        .find(|info| info.commands.iter().any(|id| id == command_id))
}

pub fn info(id: &str) -> Option<&'static ModuleInfo> {
    catalog().iter().find(|info| info.id == id)
}

fn assets_root() -> PathBuf {
    if let Ok(dir) = std::env::var("QUICKTOOL_RUST_ASSETS_DIR") {
        if !dir.trim().is_empty() {
            return PathBuf::from(dir);
        }
    }
    if let Ok(executable) = std::env::current_exe() {
        if let Some(parent) = executable.parent() {
            let candidate = parent.join("assets");
            if candidate.join("packages").is_dir() || candidate.join("modules").is_dir() {
                return candidate;
            }
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets")
}

pub fn modules_dir() -> PathBuf {
    assets_root().join("modules")
}

pub fn packages_dir() -> PathBuf {
    assets_root().join("packages")
}

pub fn module_dir(id: &str) -> PathBuf {
    modules_dir().join(id)
}

fn package_dir(id: &str) -> PathBuf {
    packages_dir().join(id)
}

/// Path of a module data file or directory, when present on disk.
pub fn module_path(id: &str, relative: &str) -> Option<PathBuf> {
    let path = module_dir(id).join(relative);
    (path.exists()).then_some(path)
}

pub fn read_module_file(id: &str, relative: &str) -> Result<Vec<u8>, String> {
    std::fs::read(module_dir(id).join(relative))
        .map_err(|error| format!("读取模块 {id} 的 {relative} 失败：{error}"))
}

fn parse_manifest(path: &Path) -> Result<Manifest, String> {
    let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    let value: Value = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    Ok(Manifest {
        id: value["id"].as_str().unwrap_or_default().to_string(),
        name: value["name"].as_str().unwrap_or_default().to_string(),
        commands: value["commands"]
            .as_array()
            .map(|list| {
                list.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default(),
        data_files: value["dataFiles"]
            .as_array()
            .map(|list| {
                list.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default(),
    })
}

/// Rescans installed modules. Bumping the data version also drops every
/// module-data cache (unicode index, countries, units, system lists).
pub fn refresh() {
    let installed: HashSet<String> = catalog()
        .iter()
        .map(|info| info.id.clone())
        .filter(|id| is_installed(id))
        .collect();
    *state().write().unwrap() = Some(installed);
    DATA_VERSION.fetch_add(1, Ordering::Relaxed);
}

/// Bumped whenever the on-disk module set may have changed; module data caches
/// compare against this and reload lazily.
pub fn data_version() -> u64 {
    DATA_VERSION.load(Ordering::Relaxed)
}

static DATA_VERSION: AtomicU64 = AtomicU64::new(0);

fn state() -> &'static RwLock<Option<HashSet<String>>> {
    static STATE: RwLock<Option<HashSet<String>>> = RwLock::new(None);
    &STATE
}

pub fn is_installed(id: &str) -> bool {
    module_dir(id).join("manifest.json").is_file()
}

/// Whether a command may run: always true before the first `refresh()`
/// (tests, probes), otherwise when its module is installed. Core commands are
/// always enabled.
pub fn is_enabled(command_id: &str) -> bool {
    let snapshot = state().read().unwrap().clone();
    match snapshot {
        None => true,
        Some(installed) => match module_of(command_id) {
            None => true,
            Some(info) => installed.contains(&info.id),
        },
    }
}

pub fn first_disabled(command_ids: impl IntoIterator<Item = impl AsRef<str>>) -> Option<String> {
    command_ids
        .into_iter()
        .map(|id| id.as_ref().to_string())
        .find(|id| !is_enabled(id))
}

/// Copy a directory tree; `std::fs` has no recursive copy.
fn copy_dir_recursive(source: &Path, target: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(target)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let path = entry.path();
        let destination = target.join(entry.file_name());
        if path.is_dir() {
            copy_dir_recursive(&path, &destination)?;
        } else {
            std::fs::copy(&path, &destination)?;
        }
    }
    Ok(())
}

/// Install a module from the package store into the live modules directory.
pub fn install(id: &str) -> Result<(), String> {
    let source = package_dir(id);
    if !source.join("manifest.json").is_file() {
        return Err("安装包中缺少该模块，请重新运行安装程序并勾选后安装".into());
    }
    let target = module_dir(id);
    if target.exists() {
        std::fs::remove_dir_all(&target).map_err(|error| format!("清理旧模块失败：{error}"))?;
    }
    std::fs::create_dir_all(modules_dir()).map_err(|error| error.to_string())?;
    copy_dir_recursive(&source, &target).map_err(|error| format!("复制模块文件失败：{error}"))?;
    let manifest = parse_manifest(&target.join("manifest.json"))
        .map_err(|error| format!("模块清单无效：{error}"))?;
    if manifest.id != id {
        let _ = std::fs::remove_dir_all(&target);
        return Err(format!("模块清单 ID 不匹配：{} ≠ {id}", manifest.id));
    }
    refresh();
    Ok(())
}

/// Uninstall by removing the module instance. User data under %APPDATA% is
/// kept so reinstalling restores previous pins and history.
pub fn uninstall(id: &str) -> Result<(), String> {
    let dir = module_dir(id);
    let manifest = parse_manifest(&dir.join("manifest.json"))
        .map_err(|_| "该目录不是有效的模块实例".to_string())?;
    if manifest.id != id {
        return Err(format!("模块清单 ID 不匹配：{} ≠ {id}", manifest.id));
    }
    std::fs::remove_dir_all(&dir).map_err(|error| format!("删除模块文件失败：{error}"))?;
    refresh();
    Ok(())
}

/// Module data leaked once per generation so callers can hold `&'static`
/// references into it (the binary originally embedded these files). A leaked
/// generation is replaced — not reclaimed — when the module set changes; module
/// installs are rare, so this trades a few MB for zero lifetime plumbing.
pub struct LeakedCache<T: ?Sized + 'static> {
    slot: Mutex<(u64, Option<&'static T>)>,
}

impl<T: ?Sized + 'static> Default for LeakedCache<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: ?Sized + 'static> LeakedCache<T> {
    pub const fn new() -> Self {
        Self {
            // u64::MAX guarantees the first access always loads: until the
            // first refresh(), data_version() is 0.
            slot: Mutex::new((u64::MAX, None)),
        }
    }
    pub fn get_or_load(&self, load: impl FnOnce() -> Option<&'static T>) -> Option<&'static T> {
        let version = data_version();
        let mut slot = self.slot.lock().unwrap();
        if slot.0 != version {
            *slot = (version, load());
        }
        slot.1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_is_complete_and_consistent() {
        let commands = crate::tools::commands();
        let mut covered: HashSet<String> = core_commands().iter().cloned().collect();
        for info in catalog() {
            assert!(!info.name.is_empty(), "module {} missing name", info.id);
            for command in &info.commands {
                assert!(
                    covered.insert(command.clone()),
                    "command {command} assigned twice"
                );
                assert!(
                    commands.iter().any(|c| &c.id == command),
                    "command {command} unknown in commands.json"
                );
            }
            if !info.data_files.is_empty() {
                assert!(
                    info.size_bytes > 0,
                    "module {} ships data without size",
                    info.id
                );
            }
        }
        for command in commands {
            assert!(
                covered.contains(&command.id),
                "command {} not assigned to a module",
                command.id
            );
        }
    }

    #[test]
    fn manifests_match_catalog() {
        for info in catalog() {
            let manifest = parse_manifest(&module_dir(&info.id).join("manifest.json"))
                .unwrap_or_else(|error| panic!("manifest {}: {error}", info.id));
            assert_eq!(manifest.id, info.id);
            assert_eq!(manifest.commands, info.commands);
            assert_eq!(manifest.data_files, info.data_files);
            for file in &manifest.data_files {
                assert!(
                    module_path(&info.id, file).is_some(),
                    "module {} data file {file} missing",
                    info.id
                );
            }
        }
    }
}
