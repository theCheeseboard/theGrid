use gpui::{App, actions};

actions!(
    thegrid,
    [AccountSettings, AccountSwitcher, LogOut, CreateRoom, DirectJoinRoom]
);

pub fn register_actions(cx: &mut App) {}
