//! Differential candidate tests against the original renderer pipeline.
//! Host fixtures are explicit; unsupported fixtures are never counted as passes.
use super::*;

pub(super) struct CandidatesProbe {
    path: std::path::PathBuf,
    oracle: Value,
    next: usize,
    checks: Vec<Value>,
    settings: Value,
    unverified_time_values: bool,
}

impl CandidatesProbe {
    pub(super) fn from_args() -> Option<Self> {
        Some(Self {
            path: std::env::args_os()
                .skip_while(|a| a != "--verify-candidates")
                .nth(1)?
                .into(),
            oracle: serde_json::from_str(include_str!("../tests/candidates-oracle.json")).unwrap(),
            next: 0,
            checks: vec![],
            settings: Value::Null,
            unverified_time_values: std::env::args()
                .any(|a| a == "--candidates-unverified-time-values"),
        })
    }
}

fn expected_command(row: &Value) -> bool {
    [
        "commandSuggestion",
        "modeAction",
        "launcherAction",
        "helpEntry",
    ]
    .iter()
    .any(|key| !row[*key].is_null())
}

fn expected_row(row: &Value) -> Value {
    serde_json::json!({
        "tool":row["tool"], "title":row["title"], "value":row["value"],
        "score":row["score"], "command":expected_command(row),
        "country":row["country"], "countryCard":row["countryCard"].as_bool().unwrap_or(false),
        "countryNavigate":row["countryNavigate"],
        "category":row["category"],
        "helpEntryId":row["helpEntry"]["id"],
    })
}

fn actual_row(app: &QuickTool, row: &Row) -> Value {
    let country = row.country.as_ref().filter(|(_, card)| *card);
    let navigate = row.country.as_ref().filter(|(_, card)| !*card);
    let help_entry = app.showing_help() && row.command;
    let tool = if help_entry {
        "help"
    } else if let Some((parent, _)) = &row.mode_action {
        parent
    } else if row.tool.starts_with('@') {
        app.mode.as_deref().unwrap_or(&row.tool)
    } else {
        &row.tool
    };
    serde_json::json!({
        "tool":tool, "title":row.item.title, "value":row.item.value,
        "internalTool":row.tool,
        "modeAction":row.mode_action,
        "toolNormalization":if help_entry {Some("Original renderer stores the help parent tool plus helpEntry.id; Rust stores that entry id directly.")}else if row.mode_action.is_some() {Some("The mode action explicitly identifies its parent tool and optional submode.")}else if row.tool.starts_with('@') {Some("Original renderer stores the current parent tool plus modeAction.submode; Rust stores @submode.")}else{None},
        "helpEntryId":if help_entry {Some(row.tool.as_str())}else{None},
        "score":row.score, "command":row.command,
        "country":country.map(|(country,_)|country["alpha2"].clone()),
        "countryCard":!row.command && country.is_some(),
        "countryNavigate":navigate.map(|(country,_)|country["alpha2"].clone()),
        "category":if app.row_is_launch(row) {"run"}else{"tools"},
    })
}

fn difference(expected: &[Value], actual: &[Value], skip_time_values: bool) -> Vec<Value> {
    let mut differences = vec![];
    if expected.len() != actual.len() {
        differences.push(
            serde_json::json!({"field":"length","expected":expected.len(),"actual":actual.len()}),
        );
    }
    for (index, (expected, actual)) in expected.iter().zip(actual).enumerate() {
        for (field, value) in expected.as_object().unwrap() {
            if skip_time_values && field == "value" && expected["tool"] == "time" {
                continue;
            }
            let equal = if field == "score" {
                value
                    .as_f64()
                    .zip(actual[field].as_f64())
                    .is_some_and(|(a, b)| (a - b).abs() < 1e-8)
            } else {
                value == &actual[field]
            };
            if !equal {
                differences.push(serde_json::json!({"index":index,"field":field,"expected":value,"actual":actual[field]}));
            }
        }
    }
    differences
}

fn normalize_validated_random(
    expected: &mut [Value],
    actual: &mut [Value],
    section: &str,
) -> Vec<Value> {
    let mut checks = vec![];
    for index in 0..expected.len() {
        let tool = expected[index]["tool"].as_str().unwrap_or("").to_string();
        if !matches!(tool.as_str(), "uuid" | "random") || expected[index]["command"] != false {
            continue;
        }
        let value = actual
            .get(index)
            .and_then(|row| row["value"].as_str())
            .unwrap_or("")
            .to_string();
        let bytes = value.as_bytes();
        let shape_valid = if tool == "uuid" {
            bytes.len() == 36
                && bytes.iter().enumerate().all(|(i, b)| {
                    if [8, 13, 18, 23].contains(&i) {
                        *b == b'-'
                    } else {
                        b.is_ascii_digit() || (b'a'..=b'f').contains(b)
                    }
                })
                && bytes[14] == b'4'
                && matches!(bytes[19], b'8' | b'9' | b'a' | b'b')
        } else {
            // Original random() maps 24 random bytes to one base36 digit each.
            // Its user-facing title says 32; the original output length is 24.
            bytes.len() == 24
                && bytes
                    .iter()
                    .all(|b| b.is_ascii_digit() || b.is_ascii_lowercase())
        };
        let passed = shape_valid
            && actual
                .get(index)
                .is_some_and(|row| row["tool"] == tool && row["command"] == false);
        checks.push(serde_json::json!({"section":section,"index":index,"tool":tool,"actualValue":value,"length":bytes.len(),"passed":passed,"rule":if tool=="uuid" {"36 ASCII characters, lowercase hex with standard hyphens, version 4 and RFC 4122 variant 8/9/a/b"}else{"24 ASCII characters in [0-9a-z], matching original 24-byte generator despite its 32-character title"}}));
        if passed {
            expected[index]["value"] = "<random validated>".into();
            actual[index]["value"] = "<random validated>".into();
        }
    }
    checks
}

fn configure(app: &mut QuickTool, spec: &Value, settings: &Value, fixed_now: u64) {
    app.candidate_testing = true;
    app.panel = Panel::None;
    app.attachment = None;
    app.input = spec["input"].as_str().unwrap_or("").into();
    app.mode = spec["mode"].as_str().map(str::to_string);
    app.submode = spec["submode"].as_str().map(str::to_string);
    app.tab = match spec["tab"].as_str() {
        Some("run") => 1,
        Some("tools") => 2,
        _ => 0,
    };
    app.store.settings = settings.clone();
    app.store.settings["dateOrder"] = "YMD".into();
    app.store.settings["timeCustomFormats"] = "".into();
    app.store.settings["fileSearchGlobal"] = true.into();
    if let Some(overrides) = spec["settings"].as_object() {
        for (key, value) in overrides {
            app.store.settings[key] = value.clone();
        }
    }
    app.store.usage.clear();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    if let Some(usage) = spec["usage"].as_object() {
        for (id, value) in usage {
            let original_last = value["last"].as_u64().unwrap_or(0);
            app.store.usage.insert(
                id.clone(),
                quicktool_rust::storage::Usage {
                    count: value["count"].as_u64().unwrap_or(0),
                    last: if original_last == 0 {
                        0
                    } else {
                        (i128::from(now) + i128::from(original_last) - i128::from(fixed_now)).max(0)
                            as u64
                    },
                },
            );
        }
    }
    app.apps = spec["apps"]
        .as_array()
        .map(|apps| {
            apps.iter()
                .map(|value| system::AppEntry {
                    name: value["name"].as_str().unwrap_or("").into(),
                    kind: "exe".into(),
                    target: value["target"].as_str().unwrap_or("").into(),
                    args: String::new(),
                    source: value["source"].as_str().unwrap_or("").into(),
                    icon_path: String::new(),
                    icon_resource: String::new(),
                })
                .collect()
        })
        .unwrap_or_default();
    app.apps_requested = true;
    app.clipboard_image = None;
    app.clipboard_entries = spec["clipboard"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .map(|value| {
                    let bytes = value["bytes"]
                        .as_array()
                        .map(|bytes| {
                            bytes
                                .iter()
                                .filter_map(Value::as_u64)
                                .map(|b| b as u8)
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();
                    platform::ClipboardEntry {
                        format: value["format"].as_str().unwrap_or("").into(),
                        kind: value["type"].as_str().unwrap_or("").into(),
                        value: value["value"].as_str().unwrap_or("").into(),
                        preview: value["preview"].as_str().unwrap_or("").into(),
                        byte_length: value["byteLength"]
                            .as_u64()
                            .map(|n| n as usize)
                            .unwrap_or(bytes.len()),
                        bytes,
                        width: value["width"].as_u64().unwrap_or(0) as u32,
                        height: value["height"].as_u64().unwrap_or(0) as u32,
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    app.file_results = spec["files"]["items"].as_array().map(|items| {
        (
            spec["files"]["query"]
                .as_str()
                .unwrap_or("")
                .trim()
                .to_string(),
            items
                .iter()
                .map(|value| index::FileEntry {
                    path: value["path"].as_str().unwrap_or("").into(),
                    name: value["name"].as_str().unwrap_or("").into(),
                    is_dir: value["is_dir"].as_bool().unwrap_or(false),
                    size: value["size"].as_u64().unwrap_or(0),
                    modified: value["modified"].as_u64().unwrap_or(0),
                })
                .collect(),
        )
    });
    app.file_query = spec["files"]["query"].as_str().map(str::to_string);
    app.files_pending = spec["files"]["pending"].as_bool().unwrap_or(false);
    let registry_query = spec["registry"]["query"].as_str().unwrap_or("");
    app.registry_query = match spec["registry"]["root"].as_str() {
        Some(root) if root != "all" => format!("{root} {registry_query}"),
        _ => registry_query.into(),
    };
    app.registry_items = spec["registry"]["items"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .map(|value| system::RegistryEntry {
                    kind: value["kind"].as_str().unwrap_or("").into(),
                    path: value["path"].as_str().unwrap_or("").into(),
                    name: value["name"].as_str().unwrap_or("").into(),
                    data: value["data"].as_str().unwrap_or("").into(),
                    value_type: value["valueType"].as_str().unwrap_or("").into(),
                    provider: value["provider"].as_str().unwrap_or("").into(),
                })
                .collect()
        })
        .unwrap_or_default();
    app.registry_due = None;
    app.registry_pending = false;
    app.country_cache.clear();
    app.result_scroll = 0.;
    app.rebuild();
    // Production pending state may be constructed, but must not dispatch host work.
    app.file_debounce = None;
    app.registry_due = None;
}

pub(super) fn tick(app: &mut QuickTool, ctx: &Context) {
    let Some(mut probe) = app.candidates_probe.take() else {
        return;
    };
    if probe.settings.is_null() {
        probe.settings = app.store.settings.clone();
    }
    let fixed_now = probe.oracle["provenance"]["fixedNowMs"]
        .as_u64()
        .expect("Original oracle must declare its fixed usage clock");
    let cases = probe.oracle["cases"].as_array().unwrap();
    for _ in 0..4 {
        let Some(spec) = cases.get(probe.next) else {
            break;
        };
        let index = probe.next;
        probe.next += 1;
        configure(app, spec, &probe.settings, fixed_now);
        let expected_raw = spec["raw"]
            .as_array()
            .unwrap()
            .iter()
            .map(expected_row)
            .collect::<Vec<_>>();
        let expected_visible = spec["visible"]
            .as_array()
            .unwrap()
            .iter()
            .map(expected_row)
            .collect::<Vec<_>>();
        let actual_raw = app
            .rows
            .iter()
            .map(|row| actual_row(app, row))
            .collect::<Vec<_>>();
        let actual_visible = app
            .visible_indices()
            .iter()
            .map(|index| actual_row(app, &app.rows[*index]))
            .collect::<Vec<_>>();
        let time_values_unverified =
            probe.unverified_time_values && expected_raw.iter().any(|row| row["tool"] == "time");
        let mut normalized_expected_raw = expected_raw.clone();
        let mut normalized_actual_raw = actual_raw.clone();
        let mut normalized_expected_visible = expected_visible.clone();
        let mut normalized_actual_visible = actual_visible.clone();
        let mut random_validation = normalize_validated_random(
            &mut normalized_expected_raw,
            &mut normalized_actual_raw,
            "raw",
        );
        random_validation.extend(normalize_validated_random(
            &mut normalized_expected_visible,
            &mut normalized_actual_visible,
            "visible",
        ));
        let raw_diff = difference(
            &normalized_expected_raw,
            &normalized_actual_raw,
            time_values_unverified,
        );
        let visible_diff = difference(
            &normalized_expected_visible,
            &normalized_actual_visible,
            time_values_unverified,
        );
        let passed = raw_diff.is_empty()
            && visible_diff.is_empty()
            && random_validation
                .iter()
                .all(|check| check["passed"] == true);
        probe.checks.push(serde_json::json!({"index":index,"input":spec["input"],"mode":spec["mode"],"tab":spec["tab"],"status":if !passed {"failed"}else if time_values_unverified {"unverified"}else{"passed"},"randomSemanticValidation":random_validation,"unverifiedFields":if time_values_unverified {serde_json::json!([{"tool":"time","field":"value","reason":"Time values excluded by explicit --candidates-unverified-time-values; titles, order, scores and non-time values remain compared."}])}else{serde_json::json!([])},"rawDifferences":raw_diff,"visibleDifferences":visible_diff,"expectedRaw":expected_raw,"actualRaw":actual_raw,"expectedVisible":expected_visible,"actualVisible":actual_visible}));
    }
    if probe.next == cases.len() {
        let passed = probe
            .checks
            .iter()
            .filter(|c| c["status"] == "passed")
            .count();
        let failed = probe
            .checks
            .iter()
            .filter(|c| c["status"] == "failed")
            .count();
        let unverified = probe
            .checks
            .iter()
            .filter(|c| c["status"] == "unverified")
            .count();
        let report = serde_json::json!({"passed":failed==0 && unverified==0,"verifiedCasesPass":failed==0,"caseCount":cases.len(),"passedCount":passed,"failedCount":failed,"unverifiedCount":unverified,"method":"Actual QuickTool::rebuild plus visible_indices; original renderer candidate oracle; synthetic host state only; no OS input or index search. Usage timestamps shifted by the oracle clock delta.","provenance":probe.oracle["provenance"],"checks":probe.checks});
        quicktool_rust::storage::atomic_write(
            &probe.path,
            &serde_json::to_vec_pretty(&report).unwrap(),
        )
        .unwrap();
        ctx.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Close);
        return;
    }
    ctx.request_repaint();
    app.candidates_probe = Some(probe);
}

#[cfg(test)]
mod random_validation_tests {
    use super::*;

    fn validate(tool: &str, value: &str) -> (bool, Value) {
        let mut expected =
            [serde_json::json!({"tool":tool,"command":false,"value":"oracle fixture"})];
        let mut actual = [serde_json::json!({"tool":tool,"command":false,"value":value})];
        let checks = normalize_validated_random(&mut expected, &mut actual, "test");
        (checks[0]["passed"] == true, actual[0]["value"].clone())
    }

    #[test]
    fn uuid_requires_version_variant_case_and_shape_before_normalizing() {
        assert_eq!(
            validate("uuid", "00000000-0000-4000-8000-000000000000"),
            (true, "<random validated>".into())
        );
        for invalid in [
            "00000000-0000-3000-8000-000000000000",
            "00000000-0000-4000-c000-000000000000",
            "A0000000-0000-4000-8000-000000000000",
            "00000000000040008000000000000000",
            "",
        ] {
            assert_eq!(validate("uuid", invalid), (false, invalid.into()));
        }
    }

    #[test]
    fn random_matches_original_actual_24_digit_base36_output() {
        assert_eq!(
            validate("random", "0123456789abcdefghijklmn"),
            (true, "<random validated>".into())
        );
        for invalid in [
            "a".repeat(23),
            "a".repeat(32),
            "A".repeat(24),
            "中".repeat(8),
        ] {
            assert_eq!(validate("random", &invalid), (false, invalid.into()));
        }
    }
}
