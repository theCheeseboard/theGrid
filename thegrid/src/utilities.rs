use contemporary::application::Details;
use gpui::App;

pub fn default_device_name(cx: &mut App) -> String {
    let details = cx.global::<Details>();

    let os = if cfg!(target_os = "macos") {
        "macOS"
    } else if cfg!(target_os = "windows") {
        "Windows"
    } else if cfg!(target_os = "linux") {
        "Linux"
    } else {
        "Unknown OS"
    };

    format!(
        "{} {}",
        details.generatable.application_name.default_value(),
        os
    )
}
