use gpui::{App, AsyncApp, Context, WeakEntity};
use imbl::Vector;
use matrix_sdk_ui::Timeline as MatrixUiTimeline;
use matrix_sdk_ui::timeline::TimelineItem;
use smol::stream::StreamExt;
use std::sync::Arc;

pub struct Timeline {
    pub inner: Arc<MatrixUiTimeline>,
    timeline_items: Vector<Arc<TimelineItem>>,
}

impl Timeline {
    pub fn new(timeline: MatrixUiTimeline, cx: &mut Context<Self>) -> Self {
        let timeline_arc = Arc::new(timeline);
        let timeline_arc_2 = timeline_arc.clone();
        cx.spawn(
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let subscription = timeline_arc.subscribe();
                let (vec, mut updates) = subscription.await;

                if weak_this
                    .update(cx, |this, cx| {
                        this.timeline_items = vec;
                        cx.notify()
                    })
                    .is_err()
                {
                    return;
                };

                while let Some(diffs) = updates.next().await {
                    if weak_this
                        .update(cx, |this, cx| {
                            for diff in diffs {
                                diff.apply(&mut this.timeline_items);
                            }
                            cx.notify()
                        })
                        .is_err()
                    {
                        return;
                    };
                }
            },
        )
        .detach();

        Self {
            inner: timeline_arc_2,
            timeline_items: Default::default(),
        }
    }

    pub fn timeline_items(&self) -> &Vector<Arc<TimelineItem>> {
        &self.timeline_items
    }
}
