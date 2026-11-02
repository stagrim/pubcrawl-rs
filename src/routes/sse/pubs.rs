use std::convert::Infallible;

use axum::{
    extract::{Query, State},
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

use crate::{
    AppState,
    r#pub::{PubChange, PubName},
    routes::pubs::{pub_markup, pub_order_weight, pub_progress_bar, pub_queue, pubs_markup},
};

#[derive(Deserialize)]
pub struct PubSSE {
    name: String,
}

pub async fn pub_updates(
    State(AppState { pubs, tx, .. }): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    // TODO: Include more info in the change events? Include the entire pub??
    let rx: Receiver<PubChange> = tx.subscribe();

    let pubs_state = pubs.clone();
    let current_state = futures::stream::once(async move {
        // TODO: use custom event here also, and avoid using random string, and switch to constants
        Ok(Event::default().data(
            html! {
                (pubs_markup(pubs_state.read().await.iter()))
            }
            .into_string(),
        ))
    });

    let update_stream = BroadcastStream::new(rx)
        // TODO: Error handling
        .then(move |change| {
            let pubs = pubs.clone();
            async move {
                let change = change.unwrap();
                println!("Received pubchange: {change:?}");
                let snippet = match change {
                    PubChange::Count(PubName(name), p) => html! {
                        hx-partial id={"pub-value-"(name)} hx-swap="outerHTML" {
                            input type="hidden" name="item" value=(pub_order_weight(&p)) id={"pub-value-"(name)};
                        }
                        hx-partial id={(name)"-count"} { (p.count) }
                        hx-partial id={"pub-"(name)"-pb"} hx-swap="outerHTML" {
                            (pub_progress_bar(&name, &p))
                        }
                    },
                    PubChange::NewPub(PubName(name), p) => html! {
                        hx-partial id="pubs" hx-swap="beforeend" {
                            (pub_markup(&name, &p, 0))
                        }
                    },
                    PubChange::Capacity(pub_name, _) => todo!(),
                    PubChange::Color(pub_name, _) => todo!(),
                    PubChange::Reload => html! {
                        hx-partial id="pubs" {
                            (pubs_markup(pubs.read().await.iter()))
                        }
                    },
                    PubChange::Queue(PubName(name), p) => html! {
                        hx-partial id={"pub-"(name)"-queue"} hx-swap="outerHTML" {
                            (pub_queue(&name, &p))
                        }
                        hx-partial id={"pub-"(name)"-pb"} hx-swap="outerHTML" {
                            (pub_progress_bar(&name, &p))
                        }
                    },
                };
                Ok(Event::default().data(snippet.into_string()))
            }
        });

    Sse::new(current_state.chain(update_stream)).keep_alive(KeepAlive::default())
}
