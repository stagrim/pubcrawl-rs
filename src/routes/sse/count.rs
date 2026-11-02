use std::convert::Infallible;

use axum::{
    extract::{Path, State},
    response::{
        Sse,
        sse::{Event, KeepAlive},
    },
};
use futures::Stream;
use maud::html;
use tokio::sync::broadcast::Receiver;
use tokio_stream::{StreamExt, wrappers::BroadcastStream};

use crate::{
    AppState,
    r#pub::{PubChange, PubName},
    routes::count::queue,
};

pub const EVENT_COUNT: &str = "count";
pub const EVENT_CAPACITY: &str = "capacity";
pub const EVENT_QUEUE: &str = "queue";

pub async fn count(
    Path(name): Path<String>,
    State(AppState { pubs, tx, .. }): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx: Receiver<PubChange> = tx.subscribe();

    let pubs_state = pubs.clone();

    let name_clone = name.clone();
    let current_state = futures::stream::once(async move {
        Ok(Event::default().data(
            // TODO: Replace with entire state
            html! {
                span id="current-count" { (pubs_state.read().await.get(&name_clone).and_then(|p| Some(p.count)).unwrap_or(0)) }
            }
            .into_string(),
        ))
    });

    let update_stream = BroadcastStream::new(rx)
        // TODO: Error handling
        .filter(move |change| match change.as_ref().unwrap() {
            PubChange::Count(PubName(n), _) => n == &name,
            PubChange::Capacity(PubName(n), _) => n == &name,
            PubChange::Color(PubName(n), _) => n == &name,
            PubChange::NewPub(PubName(n), _) => n == &name,
            PubChange::Queue(PubName(n), _) => n == &name,
            PubChange::Reload => todo!(),
        })
        .then(async move |change| {
            let change = change.unwrap();
            println!("Received pubchange: {change:?}");
            let (name, snippet) = match change {
                PubChange::Count(_, p) => (
                    EVENT_COUNT,
                    html! {
                        span {(p.count)}
                    },
                ),
                PubChange::Queue(PubName(name), p) => (
                    EVENT_QUEUE,
                    html! {
                        div hx-swap-oob=("innerHTML:#count-queue") {
                            (queue(&name, &p))
                        }
                    },
                ),
                _ => todo!(),
            };
            Ok(Event::default()
                //TODO: Event must have prefix to not mess with my commands
                .event(name)
                .data(snippet.into_string()))
        });

    Sse::new(current_state.chain(update_stream)).keep_alive(KeepAlive::default())
}
