use gpui::{App, actions};

actions!(
    thegrid,
    [AccountSettings, AccountSwitcher, LogOut, CreateRoom]
);

pub fn register_actions(cx: &mut App) {}
