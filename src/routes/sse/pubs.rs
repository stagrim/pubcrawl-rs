use std::convert::Infallible;

use axum::{
    extract::State,
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
    r#pub::PubChange,
    routes::pubs::{pub_markup, pub_order_weight, pub_progress_bar, pub_queue, pubs_markup},
};

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
                    PubChange::Count(key, p) => html! {
                        hx-partial id={"pub-value-"(key)} hx-swap="outerHTML" {
                            input type="hidden" name="item" value=(pub_order_weight(&p)) id={"pub-value-"(key)};
                        }
                        hx-partial id={(key)"-count"} { (p.count) }
                        hx-partial id={"pub-"(key)"-pb"} hx-swap="outerHTML" {
                            (pub_progress_bar(&key, &p))
                        }
                    },
                    PubChange::NewPub(key, p) => html! {
                        hx-partial id="pubs" hx-swap="beforeend" {
                            (pub_markup(&key, &p, pub_order_weight(&p)))
                        }
                    },
                    PubChange::Reload => html! {
                        hx-partial id="pubs" {
                            (pubs_markup(pubs.read().await.iter()))
                        }
                    },
                    PubChange::Queue(key, p) => html! {
                        hx-partial id={"pub-"(key)"-queue"} hx-swap="outerHTML" {
                            (pub_queue(&key, &p))
                        }
                        hx-partial id={"pub-"(key)"-pb"} hx-swap="outerHTML" {
                            (pub_progress_bar(&key, &p))
                        }
                    },
                    PubChange::UpdateEntirePub(key, p) => {
                        html! {
                            hx-partial id={"pub-card-"(key)} hx-swap="outerHTML" {
                                (pub_markup(&key, &p, pub_order_weight(&p)))
                            }
                        }
                    },
                    PubChange::Active(key, p) => {
                        if p.active {
                            html! {
                                hx-partial id="pubs" hx-swap="beforeend" {
                                    (pub_markup(&key, &p, pub_order_weight(&p)))
                                }
                            }
                        } else {
                            html! {
                                hx-partial id={"pub-card-"(key)} hx-swap="delete" {}
                            }
                        }
                    },
                };
                Ok(Event::default().data(snippet.into_string()))
            }
        });

    Sse::new(current_state.chain(update_stream)).keep_alive(KeepAlive::default())
}
