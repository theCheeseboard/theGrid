use crate::tokio_helper::TokioHelper;
use gpui::http_client::anyhow;
use gpui::{AsyncApp, Context, WeakEntity};
use matrix_sdk::ruma::events::ignored_user_list::IgnoredUserListEventContent;
use matrix_sdk::Client;

pub struct IgnoredUsersCache {
    ignore_user_list: Vec<String>,
}

impl IgnoredUsersCache {
    pub fn new(client: &Client, cx: &mut Context<Self>) -> Self {
        let client_clone = client.clone();
        cx.spawn(
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let client = client_clone.clone();
                if let Ok(Some(ignored_user_list)) = cx
                    .spawn_tokio(async move {
                        client
                            .account()
                            .account_data::<IgnoredUserListEventContent>()
                            .await
                    })
                    .await
                    && let Ok(ignored_user_list) = ignored_user_list.deserialize()
                {
                    if weak_this
                        .update(cx, |ignored_users, cx| {
                            ignored_users.ignore_user_list = ignored_user_list
                                .ignored_users
                                .iter()
                                .map(|(user, _)| user.to_string())
                                .collect();
                            cx.notify()
                        })
                        .is_err()
                    {
                        return;
                    }
                }

                let subscriber = client_clone.subscribe_to_ignore_user_list_changes();

                while let Ok(ignore_user_list) = cx
                    .spawn_tokio({
                        let mut subscriber = subscriber.clone();
                        async move { subscriber.next().await.ok_or(anyhow!("Error")) }
                    })
                    .await
                {
                    if weak_this
                        .update(cx, |ignored_users, cx| {
                            ignored_users.ignore_user_list = ignore_user_list.clone();
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

        Self {
            ignore_user_list: Vec::new(),
        }
    }

    pub fn ignore_user_list(&self) -> &Vec<String> {
        &self.ignore_user_list
    }
}
