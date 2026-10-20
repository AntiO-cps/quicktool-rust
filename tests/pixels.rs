use quicktool_rust::{
    pixels::{Filters, process},
    undo::Undo,
};
use serde::Deserialize;
#[derive(Deserialize)]
struct Case {
    filters: Filters,
    input: Vec<u8>,
    output: Vec<u8>,
}
#[test]
fn original_pixel_pipeline_differential() {
    let cases: Vec<Case> = serde_json::from_str(include_str!("pixels-oracle.json")).unwrap();
    for (i, mut case) in cases.into_iter().enumerate() {
        process(&mut case.input, case.filters).unwrap();
        assert_eq!(case.input, case.output, "Filter case {i}");
    }
}
#[test]
fn undo_coalesces_prunes_and_branches() {
    let mut history = Undo::new(0, 4, 8192);
    history.record(1, Some("typing"), 0);
    history.record(2, Some("typing"), 20);
    assert_eq!(history.undo(), Some(0));
    assert_eq!(history.redo(), Some(2));
    history.record(3, None, 1000);
    assert_eq!(history.undo(), Some(2));
    history.record(4, None, 2000);
    assert!(!history.can_redo());
    for n in 5..10 {
        history.record(n, None, n as u64 * 1000);
    }
    assert_eq!(history.undo(), Some(8));
    assert_eq!(history.undo(), Some(7));
    assert_eq!(history.undo(), Some(6));
    assert!(!history.can_undo());
}
