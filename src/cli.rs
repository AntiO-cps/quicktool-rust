use std::{ffi::OsString, path::PathBuf};

pub fn import_options(args: &[OsString]) -> Result<Option<(PathBuf, PathBuf)>, String> {
    if !args
        .iter()
        .any(|arg| arg == "--import-from" || arg == "--import-to")
    {
        return Ok(None);
    }
    let value = |flag: &str| -> Result<PathBuf, String> {
        let positions = args
            .iter()
            .enumerate()
            .filter_map(|(i, arg)| (arg == flag).then_some(i))
            .collect::<Vec<_>>();
        if positions.len() != 1 {
            return Err(format!("导入需要唯一的 {flag} 参数"));
        }
        let value = args
            .get(positions[0] + 1)
            .filter(|arg| !arg.is_empty() && !arg.to_string_lossy().starts_with("--"))
            .ok_or_else(|| format!("{flag} 缺少目录"))?;
        Ok(value.into())
    };
    Ok(Some((value("--import-from")?, value("--import-to")?)))
}

pub fn run_import(args: &[OsString]) -> Option<i32> {
    let options = match import_options(args) {
        Ok(Some(options)) => options,
        Ok(None) => return None,
        Err(error) => {
            eprintln!("{error}\n用法：QuickTool.exe --import-from OLD_DIR --import-to NEW_DIR");
            return Some(2);
        }
    };
    match quicktool_rust::import_legacy::import_from(&options.0, &options.1) {
        Ok(report) => {
            println!(
                "{}",
                serde_json::json!({"imported":report.imported,"images":report.images,"pins":report.pins,"rejected":report.rejected,"errors":report.errors.len(),"report":options.1.join("import-report.json")})
            );
            Some(if report.errors.is_empty() { 0 } else { 3 })
        }
        Err(error) => {
            eprintln!("导入失败：{error}");
            Some(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }
    #[test]
    fn ordinary_launch_does_not_start_import() {
        assert_eq!(
            import_options(&args(&["app", "--query", "CN"])).unwrap(),
            None
        );
    }
    #[test]
    fn import_requires_both_unique_paths() {
        for values in [
            vec!["app", "--import-from"],
            vec!["app", "--import-from", "old"],
            vec!["app", "--import-to", "new"],
            vec![
                "app",
                "--import-from",
                "old",
                "--import-to",
                "new",
                "--import-to",
                "other",
            ],
        ] {
            assert!(import_options(&args(&values)).is_err());
        }
    }
    #[test]
    fn import_keeps_unicode_paths_with_spaces() {
        assert_eq!(
            import_options(&args(&[
                "app",
                "--import-from",
                "旧 数据",
                "--import-to",
                "新 数据"
            ]))
            .unwrap(),
            Some((PathBuf::from("旧 数据"), PathBuf::from("新 数据")))
        );
    }
}
