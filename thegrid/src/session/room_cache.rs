use crate::tokio_helper::TokioHelper;
use gpui::{App, AppContext, AsyncApp, Entity, WeakEntity};
use imbl::Vector;
use matrix_sdk::room::ParentSpace;
use matrix_sdk::ruma::{OwnedRoomId, RoomId};
use matrix_sdk::{Client, Room, RoomState};
use smol::stream::StreamExt;
use std::collections::HashMap;

pub struct RoomCache {
    pub rooms: Entity<Vector<Room>>,
    cached_rooms: HashMap<OwnedRoomId, Entity<CachedRoom>>,
    joined_rooms: Vec<Room>,
    space_rooms: Vec<Room>,
}

pub enum RoomCategory {
    Root,
    Space(OwnedRoomId),
}

impl RoomCache {
    pub fn new(client: &Client, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let rooms = cx.new(|_| Vector::new());
            let rooms_clone = rooms.clone();

            let client = client.clone();

            cx.spawn(
                async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                    let (rooms_vector, mut room_stream) = client.rooms_stream();
                    rooms_clone.write(cx, rooms_vector).unwrap();

                    loop {
                        let Some(mutations) = room_stream.next().await else {
                            return;
                        };

                        if rooms_clone
                            .update(cx, |rooms, cx| {
                                for mutation in mutations {
                                    mutation.apply(rooms);
                                }
                                cx.notify();
                            })
                            .is_err()
                        {
                            return;
                        };
                    }
                },
            )
            .detach();

            cx.observe(&rooms, |this: &mut Self, rooms, cx| {
                let rooms = rooms.read(cx).iter().cloned().collect::<Vec<_>>();
                for room in rooms.iter() {
                    this.cached_rooms
                        .entry(room.room_id().to_owned())
                        .or_insert(CachedRoom::new(room.clone(), cx));
                }
                // this.joined_rooms = rooms
                //     .iter()
                //     .filter(|room| room.state() == RoomState::Joined)
                //     .cloned()
                //     .collect();
                // this.space_rooms = rooms
                //     .iter()
                //     .filter(|room| room.is_space())
                //     .cloned()
                //     .collect();
                cx.notify();
            })
            .detach();

            Self {
                rooms,
                cached_rooms: HashMap::new(),
                joined_rooms: Vec::new(),
                space_rooms: Vec::new(),
            }
        })
    }

    pub fn room(&self, room_id: &RoomId) -> Option<Entity<CachedRoom>> {
        self.cached_rooms.get(room_id).cloned()
    }

    pub fn joined_rooms(&self) -> &Vec<Room> {
        &self.joined_rooms
    }

    pub fn rooms_in_category(&self, category: RoomCategory, cx: &App) -> Vec<Entity<CachedRoom>> {
        match category {
            RoomCategory::Root => self
                .cached_rooms
                .values()
                .filter(|&room| {
                    let room = room.read(cx);
                    room.parent_spaces.is_empty()
                })
                .cloned()
                .collect(),
            RoomCategory::Space(room_id) => self
                .cached_rooms
                .values()
                .filter(|&room| room.read(cx).parent_spaces.contains(&room_id))
                .cloned()
                .collect(),
        }
    }
}

pub struct CachedRoom {
    pub inner: Room,
    parent_spaces: Vec<OwnedRoomId>,
}

impl CachedRoom {
    pub fn new(inner: Room, cx: &mut App) -> Entity<Self> {
        let inner_clone = inner.clone();
        cx.new(|cx| {
            cx.spawn(
                async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                    let inner = inner_clone.clone();
                    if let Ok(parents) = cx
                        .spawn_tokio(async move {
                            match inner.parent_spaces().await {
                                Ok(parents) => Ok(parents.collect::<Vec<_>>().await),
                                Err(e) => Err(e),
                            }
                        })
                        .await
                    {
                        let parent_spaces = parents
                            .into_iter()
                            .filter_map(|space| space.ok())
                            .filter_map(|space| match space {
                                ParentSpace::Reciprocal(room) => Some(room.room_id().to_owned()),
                                _ => None,
                            })
                            .collect::<Vec<_>>();
                        let _ = weak_this
                            .update(cx, |this, cx| {
                                this.parent_spaces = parent_spaces;
                                cx.notify();
                            });
                    }
                },
            )
            .detach();

            Self {
                inner,
                parent_spaces: Vec::new(),
            }
        })
    }
}
