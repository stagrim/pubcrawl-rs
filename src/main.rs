use std::{
    collections::{HashMap, hash_map::Iter},
    convert::Infallible,
    env,
    ops::Div,
    path::Path,
    sync::Arc,
};

use axum::{
    Form, Router,
    extract::{Request, State},
    response::{
        IntoResponse, Sse,
        sse::{Event, KeepAlive},
    },
    routing::{get, get_service, post},
};
use futures::{Stream, StreamExt};
use listenfd::ListenFd;
use maud::{Markup, html};
use notify::Watcher;
use redis::{
    AsyncCommands, Client,
    aio::{ConnectionManager, ConnectionManagerConfig},
};
use serde::Deserialize;
use tokio::{
    net::TcpListener,
    sync::{
        RwLock,
        broadcast::{self, Receiver, Sender},
    },
};
use tokio_stream::wrappers::BroadcastStream;
use tower_http::services::ServeDir;
#[cfg(debug_assertions)]
use tower_livereload::LiveReloadLayer;

use crate::r#pub::{Pub, PubChange, PubName};

mod db;
mod r#pub;

#[derive(Deserialize)]
struct IncreaseRequest {
    name: String,
}

async fn increase(
    State(AppState { mut con, pubs, .. }): State<AppState>,
    Form(IncreaseRequest { name }): Form<IncreaseRequest>,
) -> impl IntoResponse {
    let mut pubs = pubs.write().await;
    //TODO: Error handling on not found
    // let new_count: usize = con.hincr(format!("pub:{name}"), "count", 1).await.unwrap();
    let key = format!("pub:{name}");
    // let updated_pub: Pub = redis::transaction(&mut con, &[key], |con, pipe| async {
    //     let new_count: usize = con.hincr(format!("pub:{name}"), "count", 1).await.unwrap();
    // })
    let (new_count, updated_pub): (usize, Pub) = redis::pipe()
        .atomic()
        .hincr(format!("pub:{name}"), "count", 1)
        // .ignore()
        .hgetall(format!("pub:{name}"))
        .query_async(&mut con)
        .await
        .unwrap();
    pubs.entry(name.clone()).and_modify(|p| p.count = new_count);
    let _: () = con
        .publish(
            "pub_updates",
            serde_json::to_string(&PubChange::Count(PubName(name), updated_pub)).unwrap(),
        )
        .await
        .unwrap();
}

async fn reset(
    State(AppState { mut con, pubs, .. }): State<AppState>,
    Form(IncreaseRequest { name }): Form<IncreaseRequest>,
) -> impl IntoResponse {
    let mut pubs = pubs.write().await;
    //TODO: Error handling on not found
    let (new_count, updated_pub): (usize, Pub) = redis::pipe()
        .atomic()
        .hset(format!("pub:{name}"), "count", 0)
        // .ignore()
        .hgetall(format!("pub:{name}"))
        .query_async(&mut con)
        .await
        .unwrap();
    pubs.entry(name.clone()).and_modify(|p| p.count = new_count);
    let _: () = con
        .publish(
            "pub_updates",
            serde_json::to_string(&PubChange::Count(PubName(name), updated_pub)).unwrap(),
        )
        .await
        .unwrap();
}

#[derive(Deserialize)]
struct CreatePub {
    name: String,
    capacity: usize,
    color: String,
}

async fn create_pub(
    State(AppState { mut con, pubs, .. }): State<AppState>,
    Form(CreatePub {
        name,
        capacity,
        color,
    }): Form<CreatePub>,
) -> impl IntoResponse {
    // TODO: Add some regex check here for invalid characters, like ' :"' '
    let mut pubs = pubs.write().await;
    let new_pub = Pub {
        color,
        count: 0,
        capacity,
    };
    let _ = redis::cmd("HSET")
        .arg(format!("pub:{name}"))
        .arg(&new_pub)
        .exec_async(&mut con)
        .await
        .unwrap();
    pubs.insert(name, new_pub);

    let _: () = con
        .publish(
            "pub_updates",
            serde_json::to_string(&PubChange::Reload).unwrap(),
        )
        .await
        .unwrap();
}

async fn root(State(state): State<AppState>) -> impl IntoResponse {
    let pubs = state.pubs.read().await;

    html! {
        (maud::DOCTYPE)
        head {
            meta charset="utf-8";
            title { "Rust + HTMX + Tailwind + Maud" }
            link rel="stylesheet" href="/static/css/style.min.css";
            script src="/static/js/htmx.min.js" {}
            script src="https://cdn.jsdelivr.net/npm/htmx-ext-sse@2.2.2" crossorigin="anonymous" {}
            script src="https://cdn.jsdelivr.net/npm/sortablejs@latest/Sortable.min.js" crossorigin="anonymous" {}
            script src="/static/js/script.js" {}
        }
        body hx-ext="sse" {
            div id="pubs" sse-connect="/pub-updates" sse-swap="message" {
                (pubs_markup(pubs.iter()))
            }

            form class="mt-5" {
                select name="name" {
                    @for (name, _) in pubs.iter() {
                        option value=(name) { (name) }
                    }
                }
                button.btn hx-post="/increase" hx-swap="none" {"+"}
                button.btn hx-post="/reset" hx-swap="none" {"0"}
            }

            form class="mt-5" {
                label { "Create pub " }
                input class="input" placeholder="name" name="name";
                input class="input" type="number" placeholder="capacity" name="capacity";
                input class="input" placeholder="color" name="color";
                button.btn hx-swap="none" hx-post="/create" { "Create" }
            }
        }
    }
}

fn pubs_markup(pubs: Iter<String, Pub>) -> Markup {
    html! {
        @for (name, Pub { color, count, capacity }) in pubs {
            div
                class="card dark:bg-opacity-100 bg-opacity-80 m-1 border-l-6 shadow backdrop-blur-2xl text-black"
                style={"background-color:"(color)}
                data-id=(name)
            {
                // TODO: Update the value to the same as pubcrawl
                input type="hidden" name="item" value=(count) id={"pub-value-"(name)};
                div class="card-body" {
                    div class="card-header flex flex-row justify-between" {
                        h2 class="card-title text-xl" {
                            (name)
                        }
                        div class="flex flex-col" {
                            p id={"pub-"(name)} class="text-2xl font-bold" {
                                span id=(format!("{name}-count")) {(count)}"/"(capacity)
                            }
                        }
                    }
                    div class="flex flex-row items-center gap-2" {
                        div class="flex flex-row items-center" {
                            span class="text-nowrap whitespace-pre" { "Queue: Short" }
                            span class="status-shadow m-1 rounded-4xl p-1 shadow-lg bg-success" {}
                        }
                        div class="flex-grow-1" sse-swap=(name) {
                            (pub_progress_bar(name, color, count, capacity))
                        }
                    }
                }
            }
        }
    }
}

fn pub_progress_bar(name: &str, color: &str, count: &usize, capacity: &usize) -> Markup {
    html! {
        div class="rs-progress h-3" id={ "pub-"(name)"-pb" } role="progressbar" aria-valuemin=(count) aria-valuemax=(capacity) aria-valuenow="0" aria-labelledby="pblabel" {
            div id={"pub-"(name)"-pb-inner"} class="rs-progress-bar" style={"width:"((*count as f32).div(*capacity as f32) * 100f32)"%"} {}
        }
    }
}

fn not_htmx_predicate<T>(req: &Request<T>) -> bool {
    !req.headers().contains_key("hx-request")
}

#[derive(Clone)]
struct AppState {
    con: ConnectionManager,
    pubs: Arc<RwLock<HashMap<String, Pub>>>,
    tx: Sender<PubChange>,
}

#[tokio::main]
async fn main() {
    // let _ = dotenv();

    let port = env::var("PORT").unwrap_or("3000".into());
    let host = env::var("HOST").unwrap_or("127.0.0.1".into());

    let livereload = LiveReloadLayer::new();
    let reloader = livereload.reloader();
    println!("Connecting to db on redis://127.0.0.1:6388/?protocol=resp3");
    let redis_client = Client::open("redis://127.0.0.1:6388/?protocol=resp3").unwrap();

    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let config = ConnectionManagerConfig::new()
        .set_push_sender(tx)
        .set_max_delay(2_000)
        // .set_response_timeout(Duration::from_secs(5))
        .set_automatic_resubscription();
    let con = ConnectionManager::new_with_config(redis_client, config)
        .await
        .unwrap();
    let pubs = Arc::new(RwLock::new(HashMap::new()));
    let (tx, _) = broadcast::channel(16);

    db::start_background_thread(con.clone(), rx, pubs.clone(), tx.clone()).await;

    let state = AppState { con, pubs, tx };

    let app = Router::new()
        .route("/", get(root))
        .route("/increase", post(increase))
        .route("/reset", post(reset))
        .route("/create", post(create_pub))
        .route("/pub-updates", get(pub_updates_sse))
        .nest_service("/static", get_service(ServeDir::new("static")))
        .with_state(state);

    #[cfg(debug_assertions)]
    let app = app.layer(livereload.request_predicate(not_htmx_predicate));

    let mut listenfd = ListenFd::from_env();
    let listener = match listenfd.take_tcp_listener(0).unwrap() {
        // if we are given a tcp listener on listen fd 0, we use that one
        Some(listener) => {
            listener.set_nonblocking(true).unwrap();
            TcpListener::from_std(listener).unwrap()
        }
        // otherwise fall back to local listening
        None => TcpListener::bind(format!("{host}:{port}")).await.unwrap(),
    };

    #[cfg(debug_assertions)]
    {
        let mut watcher = notify::recommended_watcher(move |_| reloader.reload()).unwrap();
        watcher
            .watch(Path::new("static/css"), notify::RecursiveMode::NonRecursive)
            .unwrap();
    }

    println!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

async fn pub_updates_sse(
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
        .then(move |change|  {
        let pubs = pubs.clone();
        async move {
            let change = change.unwrap();
            println!("Received pubchange: {change:?}");
            let (name, snippet) =
                match change {
                    PubChange::Count(PubName(name), Pub { color, count, capacity }) =>
                        (name.clone(),
                        html! {
                            input type="hidden" name="item" value=(count) hx-swap-oob={"outerHTML:#pub-value-"(name)} id={"pub-value-"(name)};
                            span hx-swap-oob=(format!("innerHTML:#{name}-count")) { (count) }
                            (pub_progress_bar(&name, &color, &count, &capacity))
                        }),
                    // PubChange::Capacity(pub_name, _) => (todo!()),
                    // PubChange::Color(pub_name, _) => (todo!()),
                    PubChange::Reload =>  ("reload".to_string(), html! {div id="pubs" hx-swap-oob="InnerHTML:#pubs" { (pubs_markup(pubs.read().await.iter())) }}),
                    _ => ("reload".to_string(), html! {span {"test"}}),

            };
            Ok(Event::default()
            //TODO: Event must have prefix to not mess with my commands
                .event(name)
                .data(snippet.into_string()))
        }
        });

    Sse::new(current_state.chain(update_stream)).keep_alive(KeepAlive::default())
}
