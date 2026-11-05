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
use serde::Deserialize;
use tokio::sync::broadcast::Receiver;
use tokio_stream::{StreamExt, wrappers::BroadcastStream};

use crate::{AppState, r#pub::PubBroadcast};

#[derive(Deserialize, Default)]
pub struct VersionQuery {
    pub v: Option<u64>,
}

pub async fn pub_updates(
    State(AppState { pubs, tx, .. }): State<AppState>,
    Query(VersionQuery { v }): Query<VersionQuery>,
    headers: HeaderMap,
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
            let evt = Event::default().id(cache.version.to_string()).data(
                html! {
                    hx-partial id="pubs" {
                        (maud::PreEscaped(cache.rendered_pubs.as_ref()))
                    }
                }
                .into_string(),
            );
            (Some(Ok(evt)), cache.version)
        } else {
            (None, cache.version)
        }
    };

    let catchup_stream = futures::stream::iter(initial_event);

    let update_stream = BroadcastStream::new(rx).filter_map(move |msg| match msg {
        Ok(broadcast) if broadcast.version > start_version => Some(Ok(Event::default()
            .id(broadcast.version.to_string())
            .data(broadcast.pubs_html.as_ref()))),
        _ => None,
    });

    Sse::new(catchup_stream.chain(update_stream)).keep_alive(KeepAlive::default())
}
