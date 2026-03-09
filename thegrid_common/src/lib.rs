use cntp_i18n::{I18N_MANAGER, tr_load};

pub mod mxc_image;
pub mod outbound_track;
pub mod room;
pub mod session;
pub mod surfaces;
pub mod thegrid_error;
pub mod tokio_helper;

pub fn setup_thegrid_common() {
    I18N_MANAGER.write().unwrap().load_source(tr_load!());
}
