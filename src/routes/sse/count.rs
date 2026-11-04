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
use uuid::Uuid;

use crate::{AppState, r#pub::PubChange, routes::count::queue};

pub async fn count(
    Path(key): Path<Uuid>,
    State(AppState { pubs, tx, .. }): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx: Receiver<PubChange> = tx.subscribe();

    let pubs_state = pubs.clone();

    let key_clone = key.clone();
    let current_state = futures::stream::once(async move {
        Ok(Event::default().data(
            // TODO: Replace with entire state
            html! {
                hx-partial id="count-value" {
                    (pubs_state.read().await.get(&key_clone).and_then(|p| Some(p.count)).unwrap_or(0))
                }
            }
            .into_string(),
        ))
    });

    let update_stream = BroadcastStream::new(rx)
        // TODO: Error handling
        .filter(move |change| match change.as_ref().unwrap() {
            PubChange::Count(k, _) => k == &key,
            PubChange::NewPub(k, _) => k == &key,
            PubChange::Queue(k, _) => k == &key,
            PubChange::Reload => true,
            PubChange::UpdateEntirePub(k, _) => k == &key,
            PubChange::Active(k, _) => false,
        })
        .then(async move |change| {
            let change = change.unwrap();
            println!("Received pubchange: {change:?}");
            let snippet = match change {
                PubChange::Count(_, p) => html! {
                    hx-partial id="count-value" { (p.count) }
                },
                PubChange::Queue(key, p) => html! {
                    hx-partial id="count-queue" { (queue(&key, &p)) }
                },
                PubChange::UpdateEntirePub(key, p) => html! {
                    hx-partial id={"count-option-"(key)} { (&p.name) }
                    hx-partial id="count-title" { (&p.name) }
                    hx-partial id="count-value" { (p.count) }
                    hx-partial id="count-capacity" { (p.capacity) }
                    hx-partial id="count-queue" { (queue(&key, &p)) }
                },
                PubChange::NewPub(key, _) => todo!(),
                PubChange::Reload => todo!(),
                PubChange::Active(_, _) => unreachable!(),
            };
            Ok(Event::default().data(snippet.into_string()))
        });

    Sse::new(current_state.chain(update_stream)).keep_alive(KeepAlive::default())
}
