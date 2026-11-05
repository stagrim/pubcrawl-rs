use std::{env, sync::Arc};

use axum::{
    Router,
    http::{HeaderValue, header},
    routing::{get, post},
};
use dotenvy::dotenv;
use listenfd::ListenFd;
#[cfg(debug_assertions)]
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
use tower_http::{
    compression::CompressionLayer, services::ServeDir, set_header::SetResponseHeaderLayer,
};
#[cfg(debug_assertions)]
use tower_livereload::LiveReloadLayer;

use crate::r#pub::{PubBroadcast, PubsCache};

mod db;
mod r#pub;
mod routes;

#[cfg(debug_assertions)]
fn not_htmx_predicate<T>(req: &axum::extract::Request<T>) -> bool {
    !req.headers().contains_key("hx-request")
}

#[derive(Clone)]
struct AppState {
    con: ConnectionManager,
    pubs: Arc<RwLock<PubsCache>>,
    tx: Sender<PubBroadcast>,
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
    let pubs = Arc::new(RwLock::new(PubsCache::default()));
    let (tx, _) = broadcast::channel(64);

    db::start_background_thread(con.clone(), rx, pubs.clone(), tx.clone()).await;

    let state = AppState { con, pubs, tx };

    let static_service = tower::ServiceBuilder::new()
        .layer(SetResponseHeaderLayer::if_not_present(
            header::CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=31536000, immutable"),
        ))
        .service(
            ServeDir::new("./static")
                .precompressed_br()
                .precompressed_zstd()
                .precompressed_gzip(),
        );

    let html_routes = Router::new()
        .route("/", get(routes::pubs::pubs))
        .route("/admin", get(routes::admin::admin))
        .route("/count", get(routes::count::count))
        .route("/count/card", get(routes::count::count_card))
        .route_layer(CompressionLayer::new());

    // TODO: Create a etag system for html routes?
    let app = Router::new()
        .merge(html_routes)
        .route("/count/updates/{name}", get(routes::sse::count::count))
        .nest(
            "/pub",
            Router::new()
                .route("/create", post(routes::admin::create_pub))
                .route("/update", post(routes::admin::update_pub))
                .route("/active", post(routes::admin::toggle_active))
                .route("/increase", post(routes::count::increase))
                .route("/set_queue", post(routes::count::set_queue)),
        )
        .route("/pub-updates", get(routes::sse::pubs::pub_updates))
        .nest_service("/static", static_service)
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
            .watch(
                std::path::Path::new("static/css"),
                notify::RecursiveMode::NonRecursive,
            )
            .unwrap();
    }

    println!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}
