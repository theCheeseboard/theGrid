use crate::session::session_manager::SessionManager;
use crate::tokio_helper::TokioHelper;
use gpui::private::anyhow;
use gpui::{App, AppContext, AsyncApp, AsyncWindowContext, Context, Entity, WeakEntity, Window};
use imbl::Vector;
use matrix_sdk::room::{Invite, ParentSpace};
use matrix_sdk::ruma::events::space::child::SpaceChildEventContent;
use matrix_sdk::ruma::{OwnedRoomId, OwnedRoomOrAliasId, RoomId};
use matrix_sdk::Error;
use matrix_sdk::{Client, OwnedServerName, Room, RoomState};
use smol::stream::StreamExt;
use std::collections::{HashMap, HashSet};

pub struct RoomCache {
    pub rooms: Entity<Vector<Room>>,
    cached_rooms: HashMap<OwnedRoomId, Entity<CachedRoom>>,
    joined_rooms: Vec<Room>,
    space_rooms: Vec<Room>,
    joining_rooms: HashSet<OwnedRoomOrAliasId>,
}

pub enum RoomCategory {
    Root,
    Space(OwnedRoomId),
}

pub struct RoomJoinEvent {
    pub result: Result<Room, Error>,
}

impl RoomCache {
    pub fn new(client: &Client, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let rooms = cx.new(|_| Vector::new());

            let client = client.clone();

            let weak_rooms = rooms.downgrade();
            cx.spawn(
                async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                    let (rooms_vector, mut room_stream) = client.rooms_stream();
                    weak_rooms
                        .update(cx, |rooms, cx| {
                            *rooms = rooms_vector;
                            cx.notify();
                        })
                        .unwrap();

                    loop {
                        let Some(mutations) = tokio::task::unconstrained(room_stream.next()).await
                        else {
                            return;
                        };

                        if weak_rooms
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
                joining_rooms: HashSet::new(),
            }
        })
    }

    pub fn room(&self, room_id: &RoomId) -> Option<Entity<CachedRoom>> {
        self.cached_rooms.get(room_id).cloned()
    }

    pub fn joined_rooms(&self) -> &Vec<Room> {
        &self.joined_rooms
    }

    pub fn cached_rooms(&self) -> Vec<Entity<CachedRoom>> {
        self.cached_rooms.values().cloned().collect()
    }

    pub fn joining_room(&self, room: impl Into<OwnedRoomOrAliasId>) -> bool {
        self.joining_rooms.contains(&room.into())
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
            RoomCategory::Root => {
                let child_rooms = self
                    .cached_rooms
                    .values()
                    .flat_map(|room| room.read(cx).child_rooms.clone())
                    .collect::<HashSet<_>>();

                // TODO: This does not take into account loops in the room graph.
                self.cached_rooms
                    .values()
                    .filter(|&room| {
                        let room = room.read(cx);
                        !child_rooms.contains(room.inner.room_id())
                    })
                    .cloned()
                    .collect()
            }
            RoomCategory::Space(room_id) => {
                let Some(room) = self.cached_rooms.get(&room_id) else {
                    return Default::default();
                };
                room.read(cx)
                    .child_rooms
                    .iter()
                    .filter_map(|child_room| self.cached_rooms.get(child_room))
                    .cloned()
                    .collect()
            }
        }
    }

    pub fn join_room(
        &mut self,
        room_id: impl Into<OwnedRoomOrAliasId>,
        knock: bool,
        vias: Vec<OwnedServerName>,
        callback: impl Fn(&RoomJoinEvent, &mut Window, &mut App) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let session_manager = cx.global::<SessionManager>();
        let client = session_manager.client().unwrap().read(cx).clone();

        let room_id = room_id.into();

        self.joining_rooms.insert(room_id.clone());
        cx.notify();

        cx.spawn_in(
            window,
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncWindowContext| {
                let room_id_clone = room_id.clone();
                let result = cx
                    .spawn_tokio(async move {
                        if knock {
                            client.knock(room_id_clone, None, vias).await
                        } else {
                            client.join_room_by_id_or_alias(&room_id_clone, &vias).await
                        }
                    })
                    .await;

                let _ = weak_this.update_in(cx, |this, window, cx| {
                    this.joining_rooms.remove(&room_id);

                    callback(&RoomJoinEvent { result }, window, cx);
                    cx.notify();
                });
            },
        )
        .detach();
    }
}

pub struct CachedRoom {
    pub inner: Room,
    parent_spaces: Vec<OwnedRoomId>,
    child_rooms: Vec<OwnedRoomId>,
    invite_details: Option<Invite>,
    is_direct: bool,
}

#[derive(Default)]
pub struct UnreadState {
    pub unread_notifications: u64,
    pub unread_messages: u64,
}

impl CachedRoom {
    pub fn new(inner: Room, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let mut room = Self {
                inner,
                parent_spaces: Vec::new(),
                child_rooms: Vec::new(),
                invite_details: None,
                is_direct: false,
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

        cx.spawn({
            let inner = self.inner.clone();
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let Ok(space_children) = cx
                    .spawn_tokio(async move {
                        inner
                            .get_state_events_static::<SpaceChildEventContent>()
                            .await
                    })
                    .await
                else {
                    return;
                };

                let _ = weak_this.update(cx, |this, cx| {
                    this.child_rooms = space_children
                        .iter()
                        .filter_map(|space_child_event| {
                            Some(space_child_event.deserialize().ok()?.state_key().clone())
                        })
                        .collect();
                    cx.notify();
                });
            }
        })
        .detach();

        let inner = self.inner.clone();
        cx.spawn(
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let is_direct = inner.is_direct().await.is_ok_and(|is_direct| is_direct);

                let _ = weak_this.update(cx, |this, cx| {
                    this.is_direct = is_direct;
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

    pub fn is_direct(&self) -> bool {
        self.is_direct
    }

    pub fn unread_state(&self, cx: &App) -> UnreadState {
        if self.inner.is_space() {
            let session_manager = cx.global::<SessionManager>();
            let room_cache = session_manager.rooms().read(cx);

            let mut rooms_to_visit = vec![room_cache.room(self.inner.room_id()).unwrap().clone()];
            let mut child_rooms = Vec::new();
            while let Some(room_entity) = rooms_to_visit.pop() {
                let room = room_entity.read(cx);
                if child_rooms
                    .iter()
                    .any(|child_room: &Room| child_room.room_id() == room.inner.room_id())
                {
                    continue;
                }

                if room.inner.is_space() {
                    let child_rooms = room_cache.rooms_in_category(
                        RoomCategory::Space(room.inner.room_id().to_owned()),
                        cx,
                    );
                    rooms_to_visit.extend(child_rooms.iter().cloned());
                }
                child_rooms.push(room.inner.clone())
            }

            child_rooms.iter().filter(|room| !room.is_space()).fold(
                UnreadState::default(),
                |unread_state, room| UnreadState {
                    unread_notifications: unread_state.unread_notifications
                        + room.num_unread_notifications(),
                    unread_messages: unread_state.unread_messages + room.num_unread_messages(),
                },
            )
        } else {
            UnreadState {
                unread_messages: self.inner.num_unread_messages(),
                unread_notifications: self.inner.num_unread_notifications(),
            }
        }
    }
}
