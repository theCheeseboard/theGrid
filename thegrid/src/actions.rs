use gpui::{App, actions, KeyBinding};

actions!(
    thegrid,
    [
        AccountSettings,
        AccountSwitcher,
        LogOut,
        CreateRoom,
        CreateSpace,
        DirectJoinRoom
    ]
);

pub fn register_actions(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("secondary-n", CreateRoom, None),
        KeyBinding::new("secondary-s", CreateSpace, None),
        KeyBinding::new("secondary-j", DirectJoinRoom, None),
    ])
}
