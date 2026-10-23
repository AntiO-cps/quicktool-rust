//! Candidate construction from already-loaded host state; no OS requests here.
use super::*;

fn row(tool: &str, score: f64, item: ResultItem, passive: bool) -> Row {
    Row {
        score,
        tool: tool.into(),
        item,
        command: false,
        country: None,
        launch_app: None,
        passive,
        mode_action: None,
        open_target: None,
    }
}

/// Original appSearch + global launcher or runTool('run') row scoring.
pub(super) fn apps(app: &QuickTool, query: &str, global: bool) -> Vec<Row> {
    let empty = quicktool_rust::core::normalize_search(query).is_empty();
    let mut matches = app
        .apps
        .iter()
        .filter_map(|entry| {
            let score = if empty {
                0.4
            } else {
                quicktool_rust::search::fuzzy(query, &entry.name).score
            };
            (empty || score >= 0.55).then_some((entry, score))
        })
        .collect::<Vec<_>>();
    if !empty {
        matches.sort_by(|(a, a_score), (b, b_score)| {
            b_score
                .total_cmp(a_score)
                .then_with(|| quicktool_rust::countries::locale_compare(&a.name, &b.name))
        });
    }
    // Original appSearch(q, 4) truncates before the global >= .58 filter.
    matches.truncate(if global { 4 } else { 12 });
    matches
        .into_iter()
        .filter(|(_, score)| !global || *score >= 0.58)
        .enumerate()
        .map(|(index, (entry, score))| {
            let source = if entry.source.is_empty() {
                if global { "本机应用" } else { "应用" }
            } else {
                &entry.source
            };
            let mut result = row(
                "run",
                if global {
                    0.64 + score * 0.22 - index as f64 * 0.001
                } else {
                    0.99 - index as f64 * 0.001
                },
                ResultItem::new(
                    &entry.name,
                    if global {
                        format!("启动 · {source}")
                    } else {
                        source.into()
                    },
                ),
                false,
            );
            result.launch_app = Some(entry.clone());
            result
        })
        .collect()
}

fn file_status(title: &str, value: impl Into<String>) -> Vec<Row> {
    vec![row("files", 0.99, ResultItem::new(title, value), true)]
}

/// Original fileSearchItems and global file candidate injection.
pub(super) fn files(app: &QuickTool, query: &str, global: bool) -> Vec<Row> {
    let query = query.trim_matches(quicktool_rust::core::js_space);
    let matching = app
        .file_results
        .as_ref()
        .filter(|(cached_query, _)| cached_query == query)
        .map(|(_, entries)| entries);
    if global {
        if app.store.settings["fileSearchGlobal"] == false || query.is_empty() {
            return vec![];
        }
        return matching
            .into_iter()
            .flatten()
            .take(5)
            .enumerate()
            .map(|(index, entry)| {
                let name = if entry.name.is_empty() {
                    &entry.path
                } else {
                    &entry.name
                };
                let matched = quicktool_rust::search::fuzzy(query, name);
                row(
                    "files",
                    0.56 + if matched.success {
                        matched.score * 0.20
                    } else {
                        0.
                    } - index as f64 * 0.001,
                    ResultItem::new(name, &entry.path),
                    false,
                )
            })
            .collect();
    }
    if query.is_empty() {
        return file_status(
            "文件搜索",
            "输入文件名、目录名或关键词；使用本地持久索引；支持 *.pdf、ext:pdf、folder:",
        );
    }
    if let Some(entries) = matching {
        if !entries.is_empty() {
            return entries
                .iter()
                .enumerate()
                .map(|(index, entry)| {
                    row(
                        "files",
                        0.99 - index as f64 * 0.001,
                        ResultItem::new(
                            if entry.name.is_empty() {
                                &entry.path
                            } else {
                                &entry.name
                            },
                            &entry.path,
                        ),
                        false,
                    )
                })
                .collect();
        }
    } else if app.file_query.as_deref() != Some(query) {
        return file_status("正在搜索文件…", "本地文件索引");
    }
    if app.files_pending {
        return file_status("正在搜索文件…", "本地文件索引");
    }
    if app.file_status.building {
        file_status(
            "正在建立文件索引",
            format!("已发现 {} 个项目", app.file_status.count),
        )
    } else {
        file_status("没有找到文件", query)
    }
}

/// Original registrySearchItems root prefix parsing, shared with host scheduling.
pub(super) fn parse_registry_query(input: &str) -> (String, String) {
    let query = input.trim_matches(quicktool_rust::core::js_space);
    if let Some((head, tail)) = query.split_once(quicktool_rust::core::js_space) {
        let root = head.to_ascii_uppercase();
        let tail = tail.trim_matches(quicktool_rust::core::js_space);
        if matches!(root.as_str(), "HKCU" | "HKLM" | "HKCR")
            && !tail.is_empty()
            && !tail
                .chars()
                .any(|c| matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}'))
        {
            return (root, tail.into());
        }
    }
    ("all".into(), query.into())
}

fn registry_status(title: &str, value: impl Into<String>) -> Vec<Row> {
    vec![row("registry", 0.99, ResultItem::new(title, value), true)]
}

/// Original registrySearchItems over already-loaded state; no host operations.
pub(super) fn registry(app: &QuickTool, input: &str) -> Vec<Row> {
    let (root, query) = parse_registry_query(input);
    if query.is_empty() {
        return registry_status("注册表搜索", "输入关键词；可用 HKCU / HKLM / HKCR 限定根");
    }
    let loading_value = if root == "all" { "HKCU + HKLM" } else { &root };
    if parse_registry_query(&app.registry_query) != (root.clone(), query.clone())
        || ((app.registry_pending || app.registry_due.is_some()) && app.registry_items.is_empty())
    {
        return registry_status("正在搜索注册表…", loading_value);
    }
    if app.registry_items.is_empty() {
        return registry_status("没有找到注册表项", query);
    }
    app.registry_items
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let title = if entry.kind == "key" {
                entry
                    .path
                    .rsplit('\\')
                    .next()
                    .filter(|s| !s.is_empty())
                    .unwrap_or(&entry.path)
            } else if entry.name.is_empty() {
                "(Default)"
            } else {
                &entry.name
            };
            let value = if entry.kind == "value" && !entry.data.is_empty() {
                format!("{} · {}", entry.path, entry.data)
            } else {
                entry.path.clone()
            };
            let mut candidate = row(
                "registry",
                0.99 - index as f64 * 0.001,
                ResultItem::new(title, value),
                false,
            );
            candidate.open_target = Some(entry.path.clone());
            candidate
        })
        .collect()
}
