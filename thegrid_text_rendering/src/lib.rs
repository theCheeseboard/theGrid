mod cursor;
mod format;
mod global_state;
mod highlighter;
mod inline;
mod node;
mod style;
mod text_view;
mod utils;

use gpui::App;
pub use style::*;
pub use text_view::*;

pub fn init(cx: &mut App) {
    text_view::init(cx);
}
