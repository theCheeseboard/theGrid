use gpui::http_client::anyhow;
use gpui::private::anyhow;
use gpui::private::anyhow::Error;
use gpui::{AppContext, AsyncApp};
use gpui_tokio::Tokio;

pub trait TokioHelper {
    #[allow(async_fn_in_trait)]
    async fn spawn_tokio<Fut, T, E>(&self, f: Fut) -> anyhow::Result<T>
    where
        Fut: Future<Output = Result<T, E>> + Send + 'static,
        T: Send + 'static,
        E: Send + Into<Error> + 'static,
        Self: AppContext + Sized;
}

impl TokioHelper for AsyncApp {
    async fn spawn_tokio<Fut, T, E>(&self, f: Fut) -> gpui::Result<T>
    where
        Fut: Future<Output = Result<T, E>> + Send + 'static,
        T: Send + 'static,
        E: Send + Into<Error> + 'static,
        Self: AppContext + Sized,
    {
        Tokio::spawn_result(self, async move { f.await.map_err(|e| anyhow!(e)) })?.await
    }
}
