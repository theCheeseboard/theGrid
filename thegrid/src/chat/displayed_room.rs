use matrix_sdk::ruma::{OwnedRoomId, RoomId};

#[derive(Clone)]
pub enum DisplayedRoom {
    None,
    Room(OwnedRoomId),
    CreateRoom,
}
