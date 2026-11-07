//! End-to-end checks for optional modules: gating, install, uninstall and
//! module-data cache invalidation. Runs in its own process; the assets root is
//! redirected to a scratch directory with a minimal package store.

use quicktool_rust::{modules, tools};
use std::path::Path;
use std::sync::Once;

fn scratch_root() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("quicktool-modules-{}", std::process::id()))
}

fn copy_dir(source: &Path, target: &Path) {
    std::fs::create_dir_all(target).unwrap();
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let destination = target.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &destination);
        } else {
            std::fs::copy(entry.path(), destination).unwrap();
        }
    }
}

fn setup() {
    static SETUP: Once = Once::new();
    SETUP.call_once(|| {
        let root = scratch_root();
        let _ = std::fs::remove_dir_all(&root);
        let packages = root.join("assets/packages");
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/modules");
        // Only two packages ship in this fake installer payload.
        copy_dir(&repo.join("calculator"), &packages.join("calculator"));
        copy_dir(&repo.join("country"), &packages.join("country"));
        // Safe: this test binary is single-threaded at setup time. The
        // variable points at the assets root, like resources::asset().
        unsafe { std::env::set_var("QUICKTOOL_RUST_ASSETS_DIR", root.join("assets")) };
        modules::refresh();
    });
}

#[test]
fn module_lifecycle_gates_commands_and_data() {
    setup();
    // Nothing installed yet: shipped-but-unselected modules are gated, core is not.
    assert!(!modules::is_enabled("calculator"));
    assert!(!modules::is_enabled("country"));
    assert!(modules::is_enabled("settings"));
    assert!(modules::is_enabled("help"));
    assert!(modules::is_enabled("pinwindow"));

    // Suggestions and detection hide uninstalled modules.
    assert!(
        tools::suggestions_all("计算")
            .iter()
            .all(|(c, _)| c.id != "calculator")
    );
    assert!(tools::suggestions_all("计算").is_empty());
    assert!(
        tools::detect_scored("中国")
            .iter()
            .all(|(id, _, _)| id != "country")
    );

    // An exact command query still resolves so the caller can surface an
    // install hint; unrelated matches (such as the PING app for "pin") must
    // not become the best answer. Partial queries stay hidden.
    assert_eq!(
        tools::disabled_exact("pin").map(|c| c.id.as_str()),
        Some("pin")
    );
    assert_eq!(
        tools::disabled_exact("dz").map(|c| c.id.as_str()),
        Some("pin")
    );
    assert_eq!(
        tools::disabled_exact("ping").map(|c| c.id.as_str()),
        Some("system:ping")
    );
    assert_eq!(tools::disabled_exact("pinw").map(|c| c.id.as_str()), None);
    assert_eq!(
        tools::disabled_exact("settings").map(|c| c.id.as_str()),
        None
    );
    assert_eq!(tools::disabled_exact("").map(|c| c.id.as_str()), None);

    // Execution reports the module as missing instead of running half-alive.
    let error = tools::execute("calculator", "1+1").unwrap_err();
    assert!(error.contains("未安装"), "unexpected error: {error}");
    assert!(
        tools::execute("country", "中国")
            .unwrap_err()
            .contains("未安装")
    );

    // Install from the package store: command and its data come alive at once.
    modules::install("calculator").unwrap();
    modules::install("country").unwrap();
    assert!(modules::is_enabled("calculator"));
    assert_eq!(
        tools::disabled_exact("calculator").map(|c| c.id.as_str()),
        None
    );
    assert!(
        tools::suggestions_all("计算")
            .iter()
            .any(|(c, _)| c.id == "calculator")
    );
    let items = tools::execute("calculator", "1+1").unwrap();
    assert!(!items.is_empty());
    let country = tools::execute("country", "中国").unwrap();
    assert!(
        !country.is_empty(),
        "country data should load after install"
    );

    // Uninstall removes the command again; user data semantics are unchanged.
    modules::uninstall("calculator").unwrap();
    assert!(!modules::is_enabled("calculator"));
    assert!(
        tools::execute("calculator", "1+1")
            .unwrap_err()
            .contains("未安装")
    );

    // Package-less modules refuse to install with a helpful message.
    let error = modules::install("unicode").unwrap_err();
    assert!(error.contains("安装包"), "unexpected error: {error}");

    unsafe { std::env::remove_var("QUICKTOOL_RUST_ASSETS_DIR") };
    let _ = std::fs::remove_dir_all(scratch_root());
}
