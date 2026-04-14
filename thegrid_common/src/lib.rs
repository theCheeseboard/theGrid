use cntp_i18n::{tr_load, I18N_MANAGER};

pub mod mxc_image;
pub mod outbound_track;
pub mod room;
pub mod sas_emoji;
pub mod session;
pub mod sfx;
pub mod surfaces;
pub mod thegrid_error;
pub mod tokio_helper;

pub fn setup_thegrid_common() {
    I18N_MANAGER.write().unwrap().load_source(tr_load!());
}
