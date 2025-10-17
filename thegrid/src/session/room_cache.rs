use crate::tokio_helper::TokioHelper;
use gpui::http_client::anyhow;
use gpui::private::anyhow;
use gpui::{App, AppContext, AsyncApp, Context, Entity, WeakEntity};
use imbl::Vector;
use matrix_sdk::room::{Invite, ParentSpace};
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

    pub fn invited_rooms(&self, cx: &App) -> Vec<Entity<CachedRoom>> {
        self.cached_rooms
            .values()
            .filter(|&room| {
                let room = room.read(cx);
                room.inner.state() == RoomState::Invited && room.invite_details.is_some()
            })
            .cloned()
            .collect()
    }

    pub fn rooms_in_category(&self, category: RoomCategory, cx: &App) -> Vec<Entity<CachedRoom>> {
        match category {
            RoomCategory::Root => self
                .cached_rooms
                .values()
                .filter(|&room| {
                    let room = room.read(cx);
                    room.parent_spaces.is_empty() && room.inner.state() == RoomState::Joined
                })
                .cloned()
                .collect(),
            RoomCategory::Space(room_id) => self
                .cached_rooms
                .values()
                .filter(|&room| {
                    let room = room.read(cx);
                    room.parent_spaces.contains(&room_id) && room.inner.state() == RoomState::Joined
                })
                .cloned()
                .collect(),
        }
    }
}

pub struct CachedRoom {
    pub inner: Room,
    parent_spaces: Vec<OwnedRoomId>,
    invite_details: Option<Invite>,
}

impl CachedRoom {
    pub fn new(inner: Room, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let mut room = Self {
                inner,
                parent_spaces: Vec::new(),
                invite_details: None,
            };

            room.sync_changes(cx);

            let (sync_changes_tx, sync_changes_rx) = async_channel::bounded(1);

            cx.spawn(
                async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                    loop {
                        if sync_changes_rx.recv().await.is_err() {
                            return;
                        }

                        if weak_this
                            .update(cx, |this, cx| {
                                this.sync_changes(cx);
                            })
                            .is_err()
                        {
                            return;
                        }
                    }
                },
            )
            .detach();

            let room_inner = room.inner.clone();
            cx.spawn(
                async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                    let _: anyhow::Result<()> = cx
                        .spawn_tokio(async move {
                            let mut updates = room_inner.subscribe_to_updates();
                            while updates.recv().await.is_ok() {
                                if sync_changes_tx.send(()).await.is_err() {
                                    // Sync stream is closed so there's nothing else to do here
                                    return Ok(());
                                }
                            }

                            Ok(())
                        })
                        .await;
                },
            )
            .detach();

            room
        })
    }

    fn sync_changes(&mut self, cx: &mut Context<Self>) {
        let inner = self.inner.clone();
        cx.spawn(
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let inner_clone = inner.clone();
                if let Ok(parents) = cx
                    .spawn_tokio(async move {
                        match inner_clone.parent_spaces().await {
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
                    let _ = weak_this.update(cx, |this, cx| {
                        this.parent_spaces = parent_spaces;
                        cx.notify();
                    });
                }

                let inner_clone = inner.clone();
                let invite = cx
                    .spawn_tokio(async move { inner_clone.invite_details().await })
                    .await
                    .ok();
                let _ = weak_this.update(cx, |this, cx| {
                    this.invite_details = invite;
                    cx.notify();
                });
            },
        )
        .detach();
    }

    pub fn invite_details(&self) -> Option<Invite> {
        self.invite_details.clone()
    }

    pub fn display_name(&self) -> String {
        self.inner
            .cached_display_name()
            .map(|name| name.to_string())
            .or_else(|| self.inner.name())
            .unwrap_or_default()
    }
}
