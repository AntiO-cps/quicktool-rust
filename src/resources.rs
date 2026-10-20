use std::path::PathBuf;
/// Prefer portable resources beside the executable, then the development tree.
pub fn asset(relative: &str) -> PathBuf {
    if let Ok(executable) = std::env::current_exe() {
        if let Some(directory) = executable.parent() {
            let path = directory.join("assets").join(relative);
            if path.is_file() || path.is_dir() {
                return path;
            }
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join(relative)
}
