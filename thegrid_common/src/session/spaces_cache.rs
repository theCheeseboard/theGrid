use crate::tokio_helper::TokioHelper;
use async_channel::Sender;
use gpui::private::anyhow;
use gpui::{AppContext, AsyncApp, Context, Entity, WeakEntity};
use imbl::Vector;
use matrix_sdk::ruma::OwnedRoomId;
use matrix_sdk::stream::StreamExt;
use matrix_sdk::Client;
use matrix_sdk_ui::spaces::room_list::SpaceRoomListPaginationState;
use matrix_sdk_ui::spaces::{SpaceRoom, SpaceRoomList, SpaceService};
use std::sync::Arc;

pub struct SpacesCache {
    space_service: Arc<SpaceService>,
    joined_spaces: Vector<SpaceRoom>,
}

impl SpacesCache {
    pub fn new(client: &Client, cx: &mut Context<Self>) -> Self {
        let space_service = Arc::new(SpaceService::new(client.clone()));

        cx.spawn({
            let space_service = space_service.clone();
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let (joined_spaces, mut stream) = space_service.subscribe_to_joined_spaces().await;
                if weak_this
                    .update(cx, |this, cx| {
                        this.joined_spaces = joined_spaces;
                        cx.notify();
                    })
                    .is_err()
                {
                    return;
                }

                while let Some(diff) = stream.next().await {
                    if weak_this
                        .update(cx, |this, cx| {
                            for diff in diff {
                                diff.apply(&mut this.joined_spaces);
                            }
                            cx.notify();
                        })
                        .is_err()
                    {
                        return;
                    }
                }
            }
        })
        .detach();

        Self {
            space_service,
            joined_spaces: Vector::new(),
        }
    }

    pub fn space_room_list(
        &mut self,
        room_id: OwnedRoomId,
        cx: &mut Context<Self>,
    ) -> Entity<SpaceRoomListEntity> {
        let space_service = self.space_service.clone();
        let space_room_list_entity = cx.new(|cx| SpaceRoomListEntity::new());
        let space_room_list_entity_clone = space_room_list_entity.clone();
        cx.spawn(
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let room_list = cx
                    .spawn_tokio(async move {
                        Ok::<_, anyhow::Error>(space_service.space_room_list(room_id).await)
                    })
                    .await
                    .unwrap();

                space_room_list_entity_clone.update(cx, |space_room_list, cx| {
                    space_room_list.setup(room_list, cx);
                })
            },
        )
        .detach();

        space_room_list_entity
    }
}

pub struct SpaceRoomListEntity {
    rooms: Vector<SpaceRoom>,
    ready: bool,

    paginate_tx: Option<Sender<()>>,
    pagination_state: SpaceRoomListPaginationState,
}

impl SpaceRoomListEntity {
    fn new() -> Self {
        Self {
            rooms: Vector::new(),
            ready: false,
            paginate_tx: None,
            pagination_state: SpaceRoomListPaginationState::Idle { end_reached: false },
        }
    }

    fn setup(&mut self, space_room_list: SpaceRoomList, cx: &mut Context<Self>) {
        let (rooms, mut stream) = space_room_list.subscribe_to_room_updates();
        self.rooms = rooms;

        let (tx, rx) = async_channel::bounded(1);
        self.paginate_tx = Some(tx);

        cx.spawn(
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                loop {
                    let Some(diffs) = stream.next().await else {
                        return;
                    };

                    if weak_this
                        .update(cx, |this, cx| {
                            for diff in diffs {
                                diff.apply(&mut this.rooms);
                            }
                            cx.notify()
                        })
                        .is_err()
                    {
                        return;
                    }
                }
            },
        )
        .detach();

        self.pagination_state = space_room_list.pagination_state();
        let mut stream = space_room_list.subscribe_to_pagination_state_updates();
        cx.spawn(async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
            while let Some(pagination_state) = stream.next().await {
                if weak_this
                    .update(cx, |this, cx| {
                        this.pagination_state = pagination_state;
                        cx.notify();
                    })
                    .is_err()
                {
                    return;
                }
            }
        }).detach();

        cx.spawn(
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let _ = cx
                    .spawn_tokio(async move {
                        let _ = space_room_list.paginate().await;
                        while rx.recv().await.is_ok() {
                            let _ = space_room_list.paginate().await;
                        }

                        Ok::<_, anyhow::Error>(())
                    })
                    .await;
            },
        )
        .detach();

        self.ready = true;
        cx.notify();
    }

    pub fn rooms(&self) -> &Vector<SpaceRoom> {
        &self.rooms
    }

    pub fn ready(&self) -> bool {
        self.ready
    }
    
    pub fn pagination_state(&self) -> &SpaceRoomListPaginationState {
        &self.pagination_state
    }

    pub fn paginate(&self) {
        if let Some(tx) = self.paginate_tx.as_ref() {
            let _ = smol::block_on(tx.send(()));
        }
    }
}
