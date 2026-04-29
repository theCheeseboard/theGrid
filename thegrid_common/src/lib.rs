use cntp_i18n::{I18N_MANAGER, tr_load};
use contemporary::icon_tool::Url;
use contemporary::setup_parlance::setup_parlance_i18n_if_enabled;
use gpui::App;

pub mod mxc_image;
pub mod outbound_track;
pub mod room;
pub mod sas_emoji;
pub mod session;
pub mod sfx;
pub mod surfaces;
pub mod thegrid_error;
pub mod tokio_helper;

pub fn setup_thegrid_common(cx: &mut App) {
    I18N_MANAGER.write().unwrap().load_source(tr_load!());

    setup_parlance_i18n_if_enabled(
        Url::parse("https://parlance.vicr123.com/").unwrap(),
        "thegrid",
        "thegrid-core",
        "thegrid_common",
        cx,
    );
}
