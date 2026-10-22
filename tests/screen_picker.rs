use quicktool_rust::platform::{EyeDropper, EyeDropperPreview, configure_screen_picker};
#[test]
fn public_controller_is_send_and_available_without_starting_capture() {
    fn require_send<T: Send>() {}
    require_send::<EyeDropper>();
    let _constructor: fn(fn()) -> Result<EyeDropper, String> = EyeDropper::new;
    let _preview: Option<EyeDropperPreview> = None;
    assert!(configure_screen_picker("", [0, 0, 0, 0], false).is_err());
}
