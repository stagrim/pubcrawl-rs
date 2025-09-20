use std::{collections::HashMap, env, net::SocketAddr, path::Path, sync::Arc};

use axum::{
    Form, Router,
    extract::{
        ConnectInfo, Request, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    response::IntoResponse,
    routing::{any, get, get_service, post},
};
use dotenvy::dotenv;
use listenfd::ListenFd;
use maud::html;
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
use tower_http::services::ServeDir;
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
    let new_count: usize = con.hincr(format!("pub:{name}"), "count", 1).await.unwrap();
    pubs.entry(name.clone()).and_modify(|p| p.count = new_count);
    let _: () = con
        .publish(
            "pub_updates",
            serde_json::to_string(&PubChange::Count(PubName(name), new_count)).unwrap(),
        )
        .await
        .unwrap();

    html! {
        (new_count)
    }
}

// Axum handler to serve the Maud markup
async fn handler(State(state): State<AppState>) -> impl IntoResponse {
    let num = 0;
    let pubs = state.pubs.read().await;

    html! {
        (maud::DOCTYPE)
        head {
            meta charset="utf-8";
            title { "Rust + HTMX + Tailwind + Maud" }
            link rel="stylesheet" href="/static/css/style.min.css";
            script src="/static/js/htmx.min.js" {}
            script src="https://cdn.jsdelivr.net/npm/htmx-ext-ws@2.0.2" crossorigin="anonymous" {}
        }
        body  {
            h1 { "Hello from Maud! hello ✨" }
            div class="p-10 bg-blue-800" {
                p { "This div is styled with Tailwind CSS." }
            }

            div id="pubs" hx-ext="ws" ws-connect="/pub-updates" {

                @for (name, Pub { color, count, capacity }) in pubs.iter() {
                    p id="pub-{name}" { (name)": " span id=(format!("{name}-count")) {(count)} " / "(capacity) }
                }
            }

            form {
                select name="name" {
                    @for (name, _) in pubs.iter() {
                        option value=(name) { (name) }
                    }
                }
                button.btn hx-post="/increase" {"+"}
            }
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
        .route("/", get(handler))
        .route("/increase", post(increase))
        .route("/pub-updates", any(pub_updates_ws))
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

async fn pub_updates_ws(
    ws: WebSocketUpgrade,
    // ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(AppState { tx, .. }): State<AppState>,
) -> impl IntoResponse {
    // println!("{addr} connected.");
    // finalize the upgrade process by returning upgrade callback.
    // we can customize the callback by sending additional info such as address.
    ws.on_upgrade(move |socket| pub_updates_handle(socket, tx.subscribe()))
}

async fn pub_updates_handle(mut socket: WebSocket, mut rx: Receiver<PubChange>) {
    // Send blob first? Or at least update all counts and capacities

    loop {
        let change = rx.recv().await.unwrap();
        println!("Received pubchange: {change:?}");
        socket
            .send(Message::Text(
                html! {
                    @match change {
                        PubChange::Count(PubName(name), c) => span id=(format!("{name}-count")) { (c) },
                        // PubChange::Capacity(pub_name, _) => (todo!()),
                        // PubChange::Color(pub_name, _) => (todo!()),
                        // PubChange::Reload => (todo!()),
                        _ => span {"test"},
                    }
                }
                .into_string()
                .into(),
            ))
            .await
            .unwrap();
    }
}
