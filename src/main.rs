use std::{collections::HashMap, env, path::Path, sync::Arc};

use axum::{
    Router,
    extract::Request,
    middleware,
    routing::{get, post},
};
use dotenvy::dotenv;
use listenfd::ListenFd;
use notify::Watcher;
use redis::{
    Client,
    aio::{ConnectionManager, ConnectionManagerConfig},
};
use tokio::{
    net::TcpListener,
    sync::{
        RwLock,
        broadcast::{self, Sender},
    },
};
use tower_cookies::CookieManagerLayer;
use tower_http::services::ServeDir;
#[cfg(debug_assertions)]
use tower_livereload::LiveReloadLayer;
use uuid::Uuid;

use crate::{
    r#pub::{Pub, PubChange},
    routes::utils::compress_sse::compress_sse,
};

mod db;
mod r#pub;
mod routes;

#[cfg(debug_assertions)]
fn not_htmx_predicate<T>(req: &Request<T>) -> bool {
    !req.headers().contains_key("hx-request")
}

#[derive(Clone)]
struct AppState {
    con: ConnectionManager,
    pubs: Arc<RwLock<HashMap<Uuid, Pub>>>,
    tx: Sender<PubChange>,
}

#[tokio::main]
async fn main() {
    let _ = dotenv();

    let port = env::var("PORT").unwrap_or("3000".into());
    let host = env::var("HOST").unwrap_or("127.0.0.1".into());

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

    // TODO: Create a etag system for html routes?
    let app = Router::new()
        .route("/", get(routes::pubs::pubs))
        .route("/admin", get(routes::admin::admin))
        .nest(
            "/count",
            Router::new()
                .route("/", get(routes::count::count))
                .route("/card", get(routes::count::count_card))
                .route("/updates/{name}", get(routes::sse::count::count))
                .route_layer(middleware::from_fn(compress_sse)),
        )
        .nest(
            "/pub",
            Router::new()
                .route("/create", post(routes::admin::create_pub))
                .route("/update", post(routes::admin::update_pub))
                .route("/increase", post(routes::count::increase))
                .route("/set_queue", post(routes::count::set_queue)),
        )
        .route("/pub-updates", get(routes::sse::pubs::pub_updates))
        .route_layer(middleware::from_fn(compress_sse))
        .nest_service(
            "/static",
            ServeDir::new("./static")
                .precompressed_zstd()
                .precompressed_gzip(),
        )
        .with_state(state)
        .layer(CookieManagerLayer::new());

    #[cfg(debug_assertions)]
    let livereload = LiveReloadLayer::new();
    #[cfg(debug_assertions)]
    let reloader = livereload.reloader();
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
