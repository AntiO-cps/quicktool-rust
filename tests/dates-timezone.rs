use quicktool_rust::dates;
use serde_json::{Value, json};
#[test]
fn original_javascript_timezone_differential() {
    let cases: Vec<Value> =
        serde_json::from_str(include_str!("dates-timezone-oracle.json")).unwrap();
    let mut failures = Vec::new();
    for case in cases {
        let zone = case["zone"].as_str().unwrap();
        let input = case["input"].as_str().unwrap();
        let actual = dates::results_in_zone(
            input,
            case["order"].as_str().unwrap(),
            case["custom"].as_str().unwrap(),
            zone,
        )
        .map(|rows| serde_json::to_value(rows).unwrap())
        .unwrap_or(Value::Null);
        if actual != case["result"] {
            failures
                .push(json!({"zone":zone,"input":input,"actual":actual,"expected":case["result"]}));
        }
    }
    assert!(
        failures.is_empty(),
        "{} timezone differences:\n{}",
        failures.len(),
        serde_json::to_string_pretty(&failures).unwrap()
    );
}
#[test]
fn historical_dst_uses_date_specific_offset() {
    let summer = dates::parse_in_zone("1991-06-01 12:34:56", "YMD", "Asia/Shanghai").unwrap();
    assert_eq!(summer.date.unix_timestamp(), 675747296);
    assert_eq!(summer.date.offset().whole_hours(), 9);
    let winter = dates::parse_in_zone("1991-01-01 12:34:56", "YMD", "Asia/Shanghai").unwrap();
    assert_eq!(winter.date.offset().whole_hours(), 8);
}
#[test]
fn javascript_compatible_gap_overlap_and_half_hour_transitions() {
    let gap = dates::parse_in_zone("2024-03-10 02:30:00", "YMD", "America/New_York").unwrap();
    assert_eq!(
        dates::recognized_in_zone(gap.date, "America/New_York").unwrap(),
        "2024-03-10 03:30:00"
    );
    let overlap = dates::parse_in_zone("2024-11-03 01:30:00", "YMD", "America/New_York").unwrap();
    assert_eq!(overlap.date.offset().whole_hours(), -4);
    let half = dates::parse_in_zone("2024-10-06 02:15:00", "YMD", "Australia/Lord_Howe").unwrap();
    assert_eq!(
        dates::recognized_in_zone(half.date, "Australia/Lord_Howe").unwrap(),
        "2024-10-06 02:45:00"
    );
    assert!(dates::parse_in_zone("2011-12-30 12:00:00", "YMD", "Pacific/Apia").is_none());
}
#[test]
fn explicit_offset_instants_are_zone_independent_and_formatting_is_local() {
    let a = dates::parse_in_zone("1991-06-01T12:34+0800", "YMD", "America/New_York").unwrap();
    let b = dates::parse_in_zone("1991-06-01T12:34+0800", "YMD", "Asia/Shanghai").unwrap();
    assert_eq!(a.date.unix_timestamp(), b.date.unix_timestamp());
    assert_eq!(
        dates::recognized_in_zone(a.date, "Asia/Shanghai").unwrap(),
        "1991-06-01 13:34:00"
    );
    assert!(dates::parse_in_zone("2024-01-01", "YMD", "Invalid/Zone").is_none());
    assert!(dates::results_in_zone("2024-01-01", "YMD", "", "Invalid/Zone").is_err());
}

#[test]
fn out_of_range_specials_fail_without_panicking() {
    for input in [
        "ft: 170141183460469231731687303715884105727",
        "oa: 9999999999999999999999999999999999999999999",
    ] {
        assert!(
            dates::results_in_zone(input, "YMD", "", "Asia/Shanghai").is_err(),
            "{input}"
        );
    }
}

#[test]
fn full_javascript_timeclip_and_expanded_years_are_supported() {
    for (input, prefix) in [
        ("ft: 86516444736000000000", "+275760-09-13T00:00:00.000Z"),
        ("ft: -86283555264000000000", "-271821-04-20T00:00:00.000Z"),
    ] {
        let rows = dates::results_in_zone(input, "YMD", "", "America/New_York").unwrap();
        assert_eq!(
            rows.iter().find(|r| r.title == "ISO 8601").unwrap().value,
            prefix
        );
    }
    assert!(dates::results_in_zone("ft: 86516444736000010000", "YMD", "", "UTC").is_err());
    let expanded = dates::results_in_zone("9999-12-31T23:59:59-23:59", "YMD", "", "UTC").unwrap();
    assert!(
        expanded
            .iter()
            .find(|r| r.title == "ISO 8601")
            .unwrap()
            .value
            .starts_with("+010000-")
    );
    let summer = dates::parse_in_zone("2400-07-01 12:00:00", "YMD", "America/New_York").unwrap();
    assert_eq!(
        summer.date.offset().whole_hours(),
        -4,
        "future POSIX DST must extend past the last explicit TZif transition"
    );
}

#[test]
#[ignore = "Manual date query microbenchmark; no system timezone changes"]
fn timezone_query_microbenchmark() {
    use std::time::Instant;
    let cases: Vec<Value> =
        serde_json::from_str(include_str!("dates-timezone-oracle.json")).unwrap();
    let start = Instant::now();
    for c in &cases {
        std::hint::black_box(dates::results_in_zone(
            c["input"].as_str().unwrap(),
            "YMD",
            c["custom"].as_str().unwrap(),
            c["zone"].as_str().unwrap(),
        ))
        .ok();
    }
    let corpus_ms = start.elapsed().as_secs_f64() * 1000.;
    let start = Instant::now();
    for _ in 0..1000 {
        std::hint::black_box(iana_time_zone::get_timezone()).unwrap();
    }
    let detection_ms = start.elapsed().as_secs_f64() * 1000.;
    let start = Instant::now();
    for _ in 0..1000 {
        std::hint::black_box(dates::results("2026-01-01 12:00:00", "YMD", "")).unwrap();
    }
    let system_ms = start.elapsed().as_secs_f64() * 1000.;
    let start = Instant::now();
    for _ in 0..1000 {
        std::hint::black_box(dates::results_in_zone(
            "2026-01-01 12:00:00",
            "YMD",
            "",
            "Asia/Shanghai",
        ))
        .unwrap();
    }
    let injected_ms = start.elapsed().as_secs_f64() * 1000.;
    eprintln!(
        "DATE_BENCH debug corpus_{}_ms={corpus_ms:.3} raw_OS_detection_1000_ms={detection_ms:.3} system_queries_1000_ms={system_ms:.3} injected_queries_1000_ms={injected_ms:.3}",
        cases.len()
    );
}

#[test]
fn bundled_timezone_database_matches_original_electron() {
    assert_eq!(tzdb::VERSION, "2025b");
}
