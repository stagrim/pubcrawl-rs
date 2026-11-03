use axum::{
    Form, debug_handler,
    extract::{Query, State},
    response::IntoResponse,
};
use maud::{Markup, html};
use redis::AsyncCommands;
use serde::Deserialize;
use strum::IntoEnumIterator;

use crate::{
    AppState,
    r#pub::{Pub, PubChange, PubName, Queue},
    routes::utils::{base_templates, theme_extractor::ExplicitTheme},
};

pub async fn count(
    State(state): State<AppState>,
    ExplicitTheme(theme): ExplicitTheme,
) -> impl IntoResponse {
    let pubs = state.pubs.read().await;
    // TODO: this crashes when no pubs are created. Make this work like the real program!
    let first_pub = pubs.iter().next().unwrap();

    base_templates::base(
        theme,
        html! {
            select class="select mb-5" name="name" value=(first_pub.0) hx-get="/count/card" hx-target="#count-card" hx-trigger="change" {
                @for (name, _) in pubs.iter() {
                    option value=(name) { (name) }
                }
            }

            div id="count-card" {
                (count_card_markup(first_pub.0, first_pub.1))
            }
        },
    )
}

#[derive(Deserialize)]
pub struct GetCard {
    name: String,
}

#[debug_handler]
pub async fn count_card(
    Query(GetCard { name }): Query<GetCard>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let pubs = state.pubs.read().await;
    let p = pubs.get(&name).unwrap();

    count_card_markup(&name, p)
}

fn count_card_markup(name: &String, p: &Pub) -> Markup {
    html! {
        div class="card bg-base-300 border-t-6 sm:h-128" style={"border-color:"(p.color)";"} hx-sse:connect={"/count/updates/"(name)} {
            div class="card-body" {
                div class="flex flex-row justify-between" {
                    h1 class="card-title" {
                        span class="text-xl font-bold" {(name)}
                    }
                }

                div class="flex flex-col items-center sm:gap-2" {
                    form class="flex min-h-80 flex-col gap-4 sm:h-full xl:flex-row" hx-swap:inherited="none" {
                        input type="hidden" name="name" value=(name);
                        div class="join join-vertical sm:h-full" {
                            button
                                class="join-item btn btn-xl btn-success h-40 w-60 text-5xl sm:h-1/2 sm:w-128 sm:text-6xl"
                                hx-post="/pub/increase"
                                hx-trigger="click, keydown[!repeat && (key=='ArrowUp' || key=='PageUp' || key==' ')] from:body prevent"
                            {"+"}
                            button
                                class="join-item btn btn-xl btn-error h-40 w-60 text-5xl sm:h-1/2 sm:w-128 sm:text-6xl"
                                hx-post="/pub/increase"
                                hx-trigger="click, keydown[!repeat && (key=='ArrowDown' || key=='PageDown')] from:body prevent"
                                name="delta"
                                value="-1"
                            {"-"}
                        }
                        div class="stats h-full w-full bg-white shadow dark:bg-black" {
                            div class="stat text-center" {
                                span class="stat-title sm:text-xl" {"Occupancy"}
                                span class="stat-value text-5xl font-bold sm:text-8xl " {
                                    span id="count-value" { (p.count) }
                                    " / "
                                    span id="count-capacity" { (p.capacity) }
                                }
                            }
                        }
                    }

                    div class="join join-vertical" {
                        span class="text-md join-item m-2 font-bold" {"Queue Status"}
                        form hx-trigger="change" hx-post="/pub/set_queue" hx-swap="none" {
                            div class="tabs tabs-box join-item" id="count-queue" {
                                (queue(name, &p))
                            }
                        }
                    }
                }
            }
        }
    }
}

pub fn queue(name: &str, p: &Pub) -> Markup {
    html! {
        input type="hidden" value=(name) name="name";
        @for q in Queue::iter() {
            @let bg_color = match q {
                Queue::Short => "--color-success",
                Queue::Medium => "--color-warning",
                Queue::Long => "--color-error",
            };
            input class="tab transition-colors" style={"--tab-bg:var("(bg_color)");"} type="radio" name="queue" value=(q) aria-label=(q) checked[q == p.queue];
        }
    }
}

#[derive(Deserialize)]
pub struct IncreaseRequest {
    name: String,
    delta: Option<isize>,
}

const UPDATE_PUB: &str = r#"
local delta = tonumber(ARGV[1])
if not delta or delta ~= math.floor(delta) then
  return redis.error_reply("delta must be an integer")
end

local current = tonumber(redis.call("HGET", KEYS[1], "count") or "0")

-- A negative delta would take the count below zero: leave the hash unchanged.
if delta < 0 and current + delta < 0 then
  return {current, redis.call("HGETALL", KEYS[1])}
end

local new_count = redis.call("HINCRBY", KEYS[1], "count", ARGV[1])
return {new_count, redis.call("HGETALL", KEYS[1])}
"#;

pub async fn increase(
    State(AppState { mut con, pubs, .. }): State<AppState>,
    Form(IncreaseRequest { name, delta }): Form<IncreaseRequest>,
) -> impl IntoResponse {
    let mut pubs = pubs.write().await;
    let delta = delta.unwrap_or(1);
    //TODO: Error handling on not found
    let key = format!("pub:{name}");
    let (new_count, updated_pub): (usize, Pub) = redis::Script::new(UPDATE_PUB)
        .key(&key)
        .arg(delta)
        .invoke_async(&mut con)
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
pub struct QueueRequest {
    name: String,
    queue: Queue,
}

pub async fn set_queue(
    State(AppState { mut con, pubs, .. }): State<AppState>,
    Form(QueueRequest { name, queue }): Form<QueueRequest>,
) -> impl IntoResponse {
    let mut pubs = pubs.write().await;
    let key = format!("pub:{name}");
    let (_, updated_pub): (usize, Pub) = redis::pipe()
        .atomic()
        .hset(&key, "queue", queue)
        .hgetall(&key)
        .query_async(&mut con)
        .await
        .unwrap();
    pubs.entry(name.clone())
        .and_modify(|p| p.queue = updated_pub.queue.clone());
    let _: () = con
        .publish(
            "pub_updates",
            serde_json::to_string(&PubChange::Queue(PubName(name), updated_pub)).unwrap(),
        )
        .await
        .unwrap();
}
