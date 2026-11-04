use axum::{
    Form, debug_handler,
    extract::{Query, State},
    response::IntoResponse,
};
use maud::{Markup, html};
use serde::Deserialize;
use strum::IntoEnumIterator;
use uuid::Uuid;

use crate::{
    AppState,
    r#pub::{Pub, PubChange, Queue},
    routes::utils::{base_templates, theme_extractor::ExplicitTheme},
};

pub async fn count(
    State(state): State<AppState>,
    ExplicitTheme(theme): ExplicitTheme,
) -> impl IntoResponse {
    let cache = state.pubs.read().await;
    // TODO: this crashes when no pubs are created. Make this work like the real program!
    let first_pub = cache.map.iter().next().unwrap();

    base_templates::base(
        theme,
        html! {
            select class="select mb-5" name="key" value=(first_pub.0) hx-get="/count/card" hx-target="#count-card" hx-trigger="change" {
                @for (key, p) in cache.map.iter() {
                    option id={"count-option-"(key)} value=(key) { (p.name) }
                }
            }

            div id="count-card" {
                (count_card_markup(first_pub.0, first_pub.1, cache.version))
            }
        },
    )
}

#[derive(Deserialize)]
pub struct GetCard {
    key: Uuid,
}

#[debug_handler]
pub async fn count_card(
    Query(GetCard { key }): Query<GetCard>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let cache = state.pubs.read().await;
    let p = cache.map.get(&key).unwrap();

    count_card_markup(&key, p, cache.version)
}

fn count_card_markup(key: &Uuid, p: &Pub, version: u64) -> Markup {
    html! {
        div class="card bg-base-300 border-t-6 sm:h-128" style={"border-color:"(p.color)";"} hx-sse:connect={"/count/updates/"(key)"?v="(version)} {
            div class="card-body" {
                div class="flex flex-row justify-between" {
                    h1 class="card-title" {
                        span id="count-title" class="text-xl font-bold" {(p.name)}
                    }
                }

                div class="flex flex-col items-center sm:gap-2" {
                    form class="flex min-h-80 flex-col gap-4 sm:h-full xl:flex-row" hx-swap:inherited="none" {
                        input type="hidden" name="key" value=(key);
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
                        span class="text-md m-2 font-bold" {"Queue Status"}
                        form hx-trigger="change" hx-post="/pub/set_queue" hx-swap="none" {
                            div class="tabs tabs-box" id="count-queue" {
                                (queue_markup(key, &p.queue))
                            }
                        }
                    }
                }
            }
        }
    }
}

pub fn queue_markup(key: &Uuid, queue: &Queue) -> Markup {
    html! {
        input type="hidden" value=(key) name="key";
        @for q in Queue::iter() {
            @let bg_color = match q {
                Queue::Short => "--color-success",
                Queue::Medium => "--color-warning",
                Queue::Long => "--color-error",
            };
            input class="tab transition-colors" style={"--tab-bg:var("(bg_color)");"} type="radio" name="queue" value=(q) aria-label=(q) checked[&q == queue];
        }
    }
}

#[derive(Deserialize)]
pub struct IncreaseRequest {
    key: Uuid,
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
  return current
end

local new_count = redis.call("HINCRBY", KEYS[1], "count", ARGV[1])
local v = redis.call("INCR", "pub_version")
redis.call("PUBLISH", "pub_updates", cjson.encode({version = v, change = {Count = {ARGV[2], new_count}}}))
return new_count
"#;

pub async fn increase(
    State(AppState { mut con, .. }): State<AppState>,
    Form(IncreaseRequest { key, delta }): Form<IncreaseRequest>,
) -> impl IntoResponse {
    let delta = delta.unwrap_or(1);
    let _: usize = redis::Script::new(UPDATE_PUB)
        .key(&format!("pub:{key}"))
        .arg(delta)
        .arg(key.to_string())
        .invoke_async(&mut con)
        .await
        .unwrap();
}

#[derive(Deserialize)]
pub struct QueueRequest {
    key: Uuid,
    queue: Queue,
}

pub async fn set_queue(
    State(AppState { mut con, .. }): State<AppState>,
    Form(QueueRequest { key, queue }): Form<QueueRequest>,
) -> impl IntoResponse {
    let payload = serde_json::to_string(&PubChange::Queue(key, queue.clone())).unwrap();
    let _: () = redis::pipe()
        .atomic()
        .hset(&format!("pub:{key}"), "queue", &queue)
        .ignore()
        .cmd("EVAL")
        .arg(crate::db::PUBLISH_EVENT_LUA)
        .arg(0)
        .arg(payload)
        .ignore()
        .exec_async(&mut con)
        .await
        .unwrap();
}
