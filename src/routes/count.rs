use std::net::SocketAddr;

use axum::{
    Form,
    extract::{ConnectInfo, FromRequestParts, State},
    http::{HeaderMap, StatusCode, request::Parts},
    response::IntoResponse,
};
use maud::{Markup, html};
use serde::Deserialize;
use strum::IntoEnumIterator;
use tower_cookies::{Cookie, Cookies, cookie::SameSite};
use uuid::Uuid;

use crate::{
    AppState,
    r#pub::{Pub, PubChange, Queue},
    routes::utils::{base_templates, theme_extractor::ExplicitTheme},
};

pub const COUNT_COOKIE_NAME: &str = "count_auth";

pub struct AuthenticatedPub(pub Uuid);

async fn verify_cookie(cookies: &Cookies, state: &AppState) -> Option<Uuid> {
    let cookie = cookies.get(COUNT_COOKIE_NAME)?;
    let (uuid_str, count_key) = cookie.value().split_once(':')?;
    let key = Uuid::parse_str(uuid_str).ok()?;

    let cache = state.pubs.read().await;
    let p = cache.map.get(&key)?;
    if p.count_key == count_key {
        Some(key)
    } else {
        None
    }
}

impl FromRequestParts<AppState> for AuthenticatedPub {
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let cookies = Cookies::from_request_parts(parts, state)
            .await
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Failed to get cookies"))?;

        verify_cookie(&cookies, state)
            .await
            .map(AuthenticatedPub)
            .ok_or((StatusCode::UNAUTHORIZED, "Unauthorized"))
    }
}

pub async fn count(
    State(state): State<AppState>,
    cookies: Cookies,
    ExplicitTheme(theme): ExplicitTheme,
) -> impl IntoResponse {
    let authed_key = verify_cookie(&cookies, &state).await;
    let cache = state.pubs.read().await;

    let content = if let Some(key) = authed_key
        && let Some(p) = cache.map.get(&key)
    {
        count_card_markup(&key, p, cache.version)
    } else {
        login_markup(None)
    };

    base_templates::base(
        theme,
        html! {
            div id="count-card" {
                (content)
            }
        },
    )
}

fn login_markup(error: Option<&str>) -> Markup {
    html! {
        div class="card bg-base-300 w-full max-w-md mx-auto" {
            div class="card-body" {
                h2 class="card-title" { "Counter Login" }
                @if let Some(err) = error {
                    div class="alert alert-error" { (err) }
                }
                form class="flex flex-col gap-2" hx-post="/count/login" hx-target="#count-card" {
                    label class="input w-full" {
                        span class="label" { "Count Key:" }
                        input class="grow" type="password" name="count_key" required autofocus;
                    }
                    button class="btn btn-primary w-full" type="submit" { "Login" }
                }
            }
        }
    }
}

#[derive(Deserialize)]
pub struct LoginRequest {
    count_key: String,
}

const CHECK_BANNED_LUA: &str = r#"
local attempts = tonumber(redis.call("GET", KEYS[1]) or "0")
return attempts
"#;

const RECORD_FAIL_LUA: &str = r#"
local attempts = redis.call("INCR", KEYS[1])
if attempts == 1 then
    redis.call("EXPIRE", KEYS[1], 300)
end
return attempts
"#;

fn client_ip(headers: &HeaderMap, ConnectInfo(addr): ConnectInfo<SocketAddr>) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_string())
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.trim().to_string())
        })
        .unwrap_or_else(|| addr.ip().to_string())
}

pub async fn login(
    State(mut state): State<AppState>,
    cookies: Cookies,
    headers: HeaderMap,
    addr: ConnectInfo<SocketAddr>,
    Form(LoginRequest { count_key }): Form<LoginRequest>,
) -> impl IntoResponse {
    let ip = client_ip(&headers, addr);
    let ban_key = format!("login_attempts:{ip}");

    let attempts: usize = redis::Script::new(CHECK_BANNED_LUA)
        .key(&ban_key)
        .invoke_async(&mut state.con)
        .await
        .unwrap_or(0);

    if attempts >= 5 {
        return login_markup(Some("Too many failed attempts. Try again in 5 minutes."));
    }

    let found = {
        let cache = state.pubs.read().await;
        cache
            .map
            .iter()
            .find(|(_, p)| p.count_key == count_key)
            .map(|(k, p)| (*k, p.clone(), cache.version))
    };

    if let Some((key, p, version)) = found {
        let mut cookie = Cookie::new(COUNT_COOKIE_NAME, format!("{key}:{count_key}"));
        cookie.set_path("/");
        cookie.set_http_only(true);
        cookie.set_same_site(SameSite::Lax);
        cookies.add(cookie);

        count_card_markup(&key, &p, version)
    } else {
        let _: usize = redis::Script::new(RECORD_FAIL_LUA)
            .key(&ban_key)
            .invoke_async(&mut state.con)
            .await
            .unwrap_or(0);

        login_markup(Some("Invalid count key"))
    }
}

pub async fn logout(cookies: Cookies) -> impl IntoResponse {
    let mut cookie = Cookie::new(COUNT_COOKIE_NAME, "");
    cookie.set_path("/");
    cookies.remove(cookie);

    login_markup(None)
}

fn count_card_markup(key: &Uuid, p: &Pub, version: u64) -> Markup {
    html! {
        div class="card bg-base-300 border-t-6 sm:h-128" style={"border-color:"(p.color)";"} hx-sse:connect={"/count/updates?v="(version)} {
            div class="card-body" {
                div class="flex flex-row justify-between items-center" {
                    h1 class="card-title" {
                        span id="count-title" class="text-xl font-bold" {(p.name)}
                    }
                    button class="btn btn-sm btn-error" hx-post="/count/logout" hx-target="#count-card" {
                        "Logout"
                    }
                }

                div class="flex flex-col items-center sm:gap-2" {
                    form class="flex min-h-80 flex-col gap-4 sm:h-full xl:flex-row" hx-swap:inherited="none" {
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

pub fn queue_markup(_key: &Uuid, queue: &Queue) -> Markup {
    html! {
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
    AuthenticatedPub(key): AuthenticatedPub,
    State(AppState { mut con, .. }): State<AppState>,
    Form(IncreaseRequest { delta }): Form<IncreaseRequest>,
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
    queue: Queue,
}

pub async fn set_queue(
    AuthenticatedPub(key): AuthenticatedPub,
    State(AppState { mut con, .. }): State<AppState>,
    Form(QueueRequest { queue }): Form<QueueRequest>,
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
