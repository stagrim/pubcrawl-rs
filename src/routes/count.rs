use axum::{Form, extract::State, response::IntoResponse};
use maud::html;
use redis::AsyncCommands;
use serde::Deserialize;

use crate::{
    AppState,
    r#pub::{Pub, PubChange, PubName},
    routes::utils::{base_templates, theme_extractor::ExplicitTheme},
};

pub async fn count(
    State(state): State<AppState>,
    ExplicitTheme(theme): ExplicitTheme,
) -> impl IntoResponse {
    let pubs = state.pubs.read().await;

    base_templates::base(
        theme,
        html! {
            p { "Count" }
        },
    )
}

#[derive(Deserialize)]
pub struct IncreaseRequest {
    name: String,
}

pub async fn increase(
    State(AppState { mut con, pubs, .. }): State<AppState>,
    Form(IncreaseRequest { name }): Form<IncreaseRequest>,
) -> impl IntoResponse {
    let mut pubs = pubs.write().await;
    //TODO: Error handling on not found
    let key = format!("pub:{name}");
    let (new_count, updated_pub): (usize, Pub) = redis::pipe()
        .atomic()
        .hincr(&key, "count", 1)
        // .ignore()
        .hgetall(&key)
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
