use std::convert::Infallible;

use axum::{
    extract::{Query, State},
    http::HeaderMap,
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
    r#pub::{PubBroadcast, PubChange},
    routes::{
        count::{AuthenticatedPub, queue_markup},
        sse::pubs::VersionQuery,
    },
};

pub async fn count(
    AuthenticatedPub(key): AuthenticatedPub,
    Query(VersionQuery { v }): Query<VersionQuery>,
    headers: HeaderMap,
    State(AppState { pubs, tx, .. }): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx: Receiver<PubBroadcast> = tx.subscribe();

    let client_version = headers
        .get("last-event-id")
        .and_then(|val| val.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .or(v);

    let (initial_event, start_version) = {
        let cache = pubs.read().await;
        if client_version != Some(cache.version) {
            let evt = cache.map.get(&key).map(|p| {
                Ok(Event::default().id(cache.version.to_string()).data(
                    html! {
                        hx-partial id="count-title" { (&p.name) }
                        hx-partial id="count-value" { (p.count) }
                        hx-partial id="count-capacity" { (p.capacity) }
                        hx-partial id="count-queue" { (queue_markup(&key, &p.queue)) }
                    }
                    .into_string(),
                ))
            });
            (evt, cache.version)
        } else {
            (None, cache.version)
        }
    };

    let catchup_stream = futures::stream::iter(initial_event);

    let update_stream = BroadcastStream::new(rx)
        .filter_map(|msg| msg.ok())
        .filter(move |broadcast| {
            broadcast.version > start_version
                && match &broadcast.change {
                    PubChange::Count(k, _) => k == &key,
                    PubChange::Queue(k, _) => k == &key,
                    PubChange::Reload => true,
                    PubChange::UpdateEntirePub(k, _) => k == &key,
                    PubChange::NewPub(_, _) | PubChange::Active(_, _) => false,
                }
        })
        .then(move |broadcast| {
            let pubs = pubs.clone();
            async move {
                let version_id = broadcast.version.to_string();
                let snippet = match broadcast.change {
                    PubChange::Count(_, count) => html! {
                        hx-partial id="count-value" { (count) }
                    },
                    PubChange::Queue(key, queue) => html! {
                        hx-partial id="count-queue" { (queue_markup(&key, &queue)) }
                    },
                    PubChange::UpdateEntirePub(key, p) => html! {
                        hx-partial id="count-title" { (p.name) }
                        hx-partial id="count-value" { (p.count) }
                        hx-partial id="count-capacity" { (p.capacity) }
                        hx-partial id="count-queue" { (queue_markup(&key, &p.queue)) }
                    },
                    PubChange::Reload => {
                        let cache = pubs.read().await;
                        if let Some(p) = cache.map.get(&key) {
                            html! {
                                hx-partial id="count-title" { (&p.name) }
                                hx-partial id="count-value" { (p.count) }
                                hx-partial id="count-capacity" { (p.capacity) }
                                hx-partial id="count-queue" { (queue_markup(&key, &p.queue)) }
                            }
                        } else {
                            html! {}
                        }
                    }
                    PubChange::NewPub(_, _) | PubChange::Active(_, _) => unreachable!(),
                };
                Ok(Event::default().id(version_id).data(snippet.into_string()))
            }
        });

    Sse::new(catchup_stream.chain(update_stream)).keep_alive(KeepAlive::default())
}
