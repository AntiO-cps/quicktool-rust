//! Optional external file-search providers. Call `search` on a worker thread.
//! `None` requests the local SQLite fallback; `Some([])` is an authoritative empty result.
use serde::{Deserialize, Serialize};
use std::{
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{OnceLock, mpsc},
    time::{Duration, Instant},
};

pub const EVERYTHING_TIMEOUT: Duration = Duration::from_millis(1200);
pub const WINDOWS_TIMEOUT: Duration = Duration::from_millis(4500);
pub const MAX_EVERYTHING_OUTPUT: usize = 4 * 1024 * 1024;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchResult {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub item_type: String,
    pub modified: Option<String>,
    pub size: Option<u64>,
    pub provider: String,
}

/// Resolve an optional provider. Every external operation is bounded; local indexing
/// stays with the caller. Errors from explicitly selected Windows Search do not
/// silently change the provider.
pub fn search(
    provider: &str,
    query: &str,
    limit: usize,
) -> Result<Option<Vec<SearchResult>>, String> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Some(Vec::new()));
    }
    if query.contains('\0') || query.encode_utf16().count() > 30000 {
        return Err("文件搜索内容无效或过长".into());
    }
    let limit = limit.clamp(1, 100);
    match provider {
        "local" => Ok(None),
        "auto" | "everything" | "" => {
            let Some(cli) = find_everything_cli() else {
                return Ok(None);
            };
            let mut command = Command::new(cli);
            command.args(["-n", &limit.to_string(), query]);
            match run_bounded(command, EVERYTHING_TIMEOUT, MAX_EVERYTHING_OUTPUT) {
                Ok(output) => {
                    let rows = parse_everything_output(&output, limit);
                    Ok((!rows.is_empty()).then_some(rows))
                }
                Err(_) => Ok(None),
            }
        }
        "windows" => {
            #[cfg(windows)]
            {
                let query = query.to_owned();
                let (tx, rx) = mpsc::channel();
                std::thread::spawn(move || {
                    let _ = tx.send(windows_search::query(&query, limit));
                });
                rx.recv_timeout(WINDOWS_TIMEOUT)
                    .map_err(|_| "Windows Search 查询超时".to_string())?
                    .map(Some)
            }
            #[cfg(not(windows))]
            {
                Err("Windows Search 需要 Windows".into())
            }
        }
        _ => Err(format!("未知文件搜索提供程序：{provider}")),
    }
}

/// Match the original discovery order, without launching `where.exe`.
pub fn find_everything_cli() -> Option<PathBuf> {
    static PATH: OnceLock<Option<PathBuf>> = OnceLock::new();
    PATH.get_or_init(|| {
        let mut candidates = Vec::new();
        if let Some(root) = std::env::var_os("ProgramFiles") {
            let root = PathBuf::from(root);
            candidates.push(root.join("Everything/es.exe"));
            candidates.push(root.join("Everything 1.5a/es.exe"));
        }
        if let Some(root) = std::env::var_os("LOCALAPPDATA") {
            candidates.push(PathBuf::from(root).join("Everything/es.exe"));
        }
        if let Ok(root) = std::env::current_dir() {
            candidates.push(root.join("es.exe"));
        }
        if let Some(path) = std::env::var_os("PATH") {
            candidates.extend(std::env::split_paths(&path).map(|root| root.join("es.exe")));
        }
        candidates.into_iter().find(|path| path.is_file())
    })
    .clone()
}

/// Everything's CLI emits one complete path per line. Its UTF-8 output contract
/// matches the original `execFile` integration. No shell interprets the query.
pub fn parse_everything_output(output: &[u8], limit: usize) -> Vec<SearchResult> {
    String::from_utf8_lossy(output)
        .trim_start_matches('\u{feff}')
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .take(limit.clamp(1, 100))
        .map(|path| {
            let p = Path::new(path);
            let metadata = p.metadata().ok();
            let name = path
                .rsplit(['\\', '/'])
                .find(|s| !s.is_empty())
                .unwrap_or(path);
            SearchResult {
                name: name.into(),
                path: path.into(),
                is_dir: metadata.as_ref().is_some_and(|m| m.is_dir()),
                item_type: String::new(),
                modified: None,
                size: None,
                provider: "Everything".into(),
            }
        })
        .collect()
}
pub fn windows_query_sql(query: &str, limit: usize) -> String {
    let escaped = query
        .trim()
        .replace('\'', "''")
        .replace('%', "[%]")
        .replace('_', "[_]");
    format!(
        "SELECT TOP {} System.ItemNameDisplay, System.ItemPathDisplay, System.ItemType, System.DateModified, System.Size FROM SYSTEMINDEX WHERE System.ItemNameDisplay LIKE '%{escaped}%' ORDER BY System.Search.Rank DESC, System.DateModified DESC",
        limit.clamp(1, 100)
    )
}

fn run_bounded(
    mut command: Command,
    timeout: Duration,
    max_output: usize,
) -> Result<Vec<u8>, String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    let stdout = child.stdout.take().ok_or("无法读取 Everything 输出")?;
    let (tx, rx) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stdout
            .take(max_output as u64 + 1)
            .read_to_end(&mut bytes)
            .map(|_| bytes)
            .map_err(|e| e.to_string());
        let _ = tx.send(result);
    });
    let deadline = Instant::now() + timeout;
    let mut data = None;
    let result = loop {
        if data.is_none() {
            if let Ok(result) = rx.try_recv() {
                match result {
                    Ok(bytes) if bytes.len() <= max_output => data = Some(bytes),
                    Ok(_) => break Err("Everything 输出超过 4 MB".into()),
                    Err(e) => break Err(e),
                }
            }
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    break Err(format!("Everything 返回错误：{status}"));
                }
                if let Some(bytes) = data.take() {
                    break Ok(bytes);
                }
                match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                    Ok(Ok(bytes)) if bytes.len() <= max_output => break Ok(bytes),
                    Ok(Ok(_)) => break Err("Everything 输出超过 4 MB".into()),
                    Ok(Err(e)) => break Err(e),
                    Err(_) => break Err("Everything 查询超时".into()),
                }
            }
            Ok(None) => {}
            Err(e) => break Err(e.to_string()),
        }
        if Instant::now() >= deadline {
            break Err("Everything 查询超时".into());
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    if result.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    // The owned child has exited or was killed, which closes its stdout pipe.
    // Join only after EOF; a descendant retaining a pipe cannot stall the caller.
    if reader.is_finished() {
        let _ = reader.join();
    }
    result
}

#[cfg(windows)]
mod windows_search {
    use super::*;
    use windows::{
        Win32::System::Com::*,
        core::{BSTR, GUID, IUnknown, Interface, PCWSTR, VARIANT},
    };
    fn dispatch(value: &VARIANT) -> Result<IDispatch, String> {
        unsafe {
            let raw = value.as_raw();
            if raw.Anonymous.Anonymous.vt == 9 {
                let ptr = raw.Anonymous.Anonymous.Anonymous.pdispVal;
                IDispatch::from_raw_borrowed(&ptr)
                    .cloned()
                    .ok_or("Windows Search 未返回记录集".into())
            } else {
                IUnknown::try_from(value)
                    .and_then(|u| u.cast())
                    .map_err(|e| e.to_string())
            }
        }
    }
    fn call(
        object: &IDispatch,
        name: &str,
        flags: DISPATCH_FLAGS,
        mut args: Vec<VARIANT>,
    ) -> Result<VARIANT, String> {
        unsafe {
            let method_name = name.to_owned();
            let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
            let name = PCWSTR(wide.as_ptr());
            let mut id = 0;
            object
                .GetIDsOfNames(&GUID::zeroed(), &name, 1, 0, &mut id)
                .map_err(|e| e.to_string())?;
            args.reverse();
            let mut named = -3i32;
            let params = DISPPARAMS {
                rgvarg: if args.is_empty() {
                    std::ptr::null_mut()
                } else {
                    args.as_mut_ptr()
                },
                rgdispidNamedArgs: if flags == DISPATCH_PROPERTYPUT {
                    &mut named
                } else {
                    std::ptr::null_mut()
                },
                cArgs: args.len() as u32,
                cNamedArgs: if flags == DISPATCH_PROPERTYPUT { 1 } else { 0 },
            };
            let mut result = VARIANT::new();
            let mut exception = EXCEPINFO::default();
            let outcome = object.Invoke(
                id,
                &GUID::zeroed(),
                0,
                flags,
                &params,
                Some(&mut result),
                Some(&mut exception),
                None,
            );
            if outcome.is_err() {
                if let Some(fill) = exception.pfnDeferredFillIn {
                    let _ = fill(&mut exception);
                }
            }
            let mut description = exception.bstrDescription.to_string();
            if outcome.is_err() && description.is_empty() {
                if let Ok(info) = GetErrorInfo(0) {
                    description = info
                        .GetDescription()
                        .map(|s| s.to_string())
                        .unwrap_or_default();
                }
            }
            let provider_code = exception.scode;
            std::mem::ManuallyDrop::drop(&mut exception.bstrSource);
            std::mem::ManuallyDrop::drop(&mut exception.bstrDescription);
            std::mem::ManuallyDrop::drop(&mut exception.bstrHelpFile);
            outcome.map_err(|e| {
                if provider_code as u32 == 0x80041607 {
                    return "Windows Search 查询超时（3 秒）".to_string();
                }
                format!(
                    "Windows Search {method_name}：{}",
                    if description.is_empty() {
                        if provider_code != 0 {
                            format!("{e} (provider 0x{:08X})", provider_code as u32)
                        } else {
                            e.to_string()
                        }
                    } else {
                        description
                    }
                )
            })?;
            Ok(result)
        }
    }
    struct Close(IDispatch);
    impl Drop for Close {
        fn drop(&mut self) {
            let _ = call(&self.0, "Close", DISPATCH_METHOD, vec![]);
        }
    }
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
    fn string(v: &VARIANT) -> String {
        BSTR::try_from(v).map(|s| s.to_string()).unwrap_or_default()
    }
    fn field(fields: &IDispatch, index: i32) -> Result<VARIANT, String> {
        let f = dispatch(&call(
            fields,
            "Item",
            DISPATCH_PROPERTYGET,
            vec![index.into()],
        )?)?;
        call(&f, "Value", DISPATCH_PROPERTYGET, vec![])
    }
    pub fn query(query: &str, limit: usize) -> Result<Vec<SearchResult>, String> {
        unsafe {
            let _apartment = Apartment(CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok());
            let clsid = GUID::from_u128(0x00000514_0000_0010_8000_00aa006d2ea4);
            let connection: IDispatch = CoCreateInstance(&clsid, None, CLSCTX_INPROC_SERVER)
                .map_err(|e| format!("Windows Search ADO 不可用：{e}"))?;
            call(
                &connection,
                "ConnectionTimeout",
                DISPATCH_PROPERTYPUT,
                vec![3i32.into()],
            )?;
            call(
                &connection,
                "CommandTimeout",
                DISPATCH_PROPERTYPUT,
                vec![3i32.into()],
            )?;
            call(
                &connection,
                "Open",
                DISPATCH_METHOD,
                vec![
                    "Provider=Search.CollatorDSO;Extended Properties='Application=Windows';".into(),
                ],
            )?;
            let connection = Close(connection);
            let recordset = Close(dispatch(&call(
                &connection.0,
                "Execute",
                DISPATCH_METHOD,
                vec![windows_query_sql(query, limit).as_str().into()],
            )?)?);
            let fields = dispatch(&call(&recordset.0, "Fields", DISPATCH_PROPERTYGET, vec![])?)?;
            let mut rows = Vec::new();
            while rows.len() < limit.clamp(1, 100)
                && !bool::try_from(&call(&recordset.0, "EOF", DISPATCH_PROPERTYGET, vec![])?)
                    .map_err(|e| e.to_string())?
            {
                let name = string(&field(&fields, 0)?);
                let path = string(&field(&fields, 1)?);
                if !path.trim().is_empty() {
                    let item_type = string(&field(&fields, 2)?);
                    let date = field(&fields, 3)?;
                    let modified = if date.as_raw().Anonymous.Anonymous.vt == 7 {
                        let days = date.as_raw().Anonymous.Anonymous.Anonymous.date;
                        time::OffsetDateTime::from_unix_timestamp(
                            ((days - 25569.0) * 86400.0) as i64,
                        )
                        .ok()
                        .and_then(|d| {
                            d.format(&time::format_description::well_known::Rfc3339)
                                .ok()
                        })
                    } else {
                        None
                    };
                    let size = u64::try_from(&field(&fields, 4)?).ok();
                    let is_dir = Path::new(&path).is_dir();
                    rows.push(SearchResult {
                        name,
                        path,
                        is_dir,
                        item_type,
                        modified,
                        size,
                        provider: "Windows Search".into(),
                    });
                }
                call(&recordset.0, "MoveNext", DISPATCH_METHOD, vec![])?;
            }
            Ok(rows)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn output_capture_is_bounded() {
        let mut cmd = Command::new(std::env::current_exe().unwrap());
        cmd.args(["--list"]);
        let output = run_bounded(cmd, Duration::from_secs(2), 128 * 1024).unwrap();
        assert!(!output.is_empty());
    }
    #[test]
    fn oversized_output_is_rejected() {
        let mut cmd = Command::new(std::env::current_exe().unwrap());
        cmd.args(["--list"]);
        assert!(run_bounded(cmd, Duration::from_secs(2), 1).is_err());
    }
    #[test]
    #[ignore = "Subprocess fixture used by bounded_command_timeout"]
    fn command_sleep_fixture() {
        std::thread::sleep(Duration::from_secs(5));
    }
    #[test]
    fn bounded_command_timeout() {
        let mut cmd = Command::new(std::env::current_exe().unwrap());
        cmd.args([
            "--exact",
            "file_providers::tests::command_sleep_fixture",
            "--ignored",
            "--nocapture",
        ]);
        let start = Instant::now();
        assert!(run_bounded(cmd, Duration::from_millis(40), 4096).is_err());
        assert!(start.elapsed() < Duration::from_secs(2));
    }
}
