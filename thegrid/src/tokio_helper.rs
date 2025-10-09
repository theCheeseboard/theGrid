use gpui::http_client::anyhow;
use gpui::private::anyhow;
use gpui::private::anyhow::Error;
use gpui::{AppContext, AsyncApp};
use gpui_tokio::Tokio;
use std::fmt::{Debug, Display};

pub trait TokioHelper {
    #[allow(async_fn_in_trait)]
    async fn spawn_tokio<Fut, T, E>(&self, f: Fut) -> Result<T, E>
    where
        Fut: Future<Output = Result<T, E>> + Send + 'static,
        T: Send + 'static,
        E: Display + Debug + Sync + Send + Into<Error> + 'static,
        Self: AppContext + Sized;
}

impl TokioHelper for AsyncApp {
    async fn spawn_tokio<Fut, T, E>(&self, f: Fut) -> Result<T, E>
    where
        Fut: Future<Output = Result<T, E>> + Send + 'static,
        T: Send + 'static,
        E: Display + Debug + Sync + Send + Into<Error> + 'static,
        Self: AppContext + Sized,
    {
        Tokio::spawn_result(self, async move { f.await.map_err(|e| anyhow!(e)) })
            .unwrap()
            .await
            .map_err(|e| e.downcast::<E>().unwrap())
    }
}
