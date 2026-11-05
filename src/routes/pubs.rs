use std::{collections::hash_map::Iter, ops::Div};

use axum::{extract::State, response::IntoResponse};
use maud::{Markup, html};
use uuid::Uuid;

use crate::{
    AppState,
    r#pub::{Pub, Queue},
    routes::utils::{base_templates, theme_extractor::ExplicitTheme},
};

pub async fn pubs(
    State(state): State<AppState>,
    ExplicitTheme(theme): ExplicitTheme,
) -> impl IntoResponse {
    let (version, rendered_pubs) = {
        let cache = state.pubs.read().await;
        (cache.version, cache.rendered_pubs.clone())
    };

    base_templates::base(
        theme,
        html! {
            div id="pubs" hx-sse:connect={"/pub-updates?v="(version)} {
                (maud::PreEscaped(rendered_pubs.as_ref()))
            }
        },
    )
}

pub fn pubs_markup(pubs: Iter<Uuid, Pub>) -> Markup {
    let mut pubs = pubs
        .filter(|(_, p)| p.active)
        .map(|(key, p)| (key, p, pub_order_weight(p)))
        .collect::<Vec<_>>();
    pubs.sort_unstable_by_key(|(_, _, weight)| -weight);

    html! {
        @for (key, p, weight) in pubs {
            (pub_markup(key, p, weight))
        }
    }
}

pub fn pub_order_weight(p: &Pub) -> isize {
    (p.count as f32 / p.capacity as f32 * 1000_000.0) as isize
}

pub fn pub_markup(key: &Uuid, p: &Pub, weight: isize) -> Markup {
    html! {
        div
            class={
                "card m-1 border-l-6 shadow border-[color-mix(in_oklab,currentColor_15%,transparent)] "
                (if is_black_text(&p.color) {"text-black"} else {"text-white"})
            }
            style={"background-color:"(p.color)}
            data-id=(key)
            id={"pub-card-"(key)}
        {
            input type="hidden" name="item" value=(weight) id={"pub-value-"(key)};
            div class="card-body" {
                div class="card-header flex flex-row justify-between" {
                    h2 class="card-title text-xl" {
                        (&p.name)
                    }
                    div class="flex flex-col" {
                        p id={"pub-"(key)} class="text-2xl font-bold" {
                            span id={(key)"-count"} {(p.count)}"/"(p.capacity)
                        }
                    }
                }
                div class="flex flex-row items-center gap-2" {
                    (pub_queue(key, &p.queue))
                    div class="flex-grow-1" {
                        (pub_progress_bar(key, p.count, p.capacity))
                    }
                }
            }
        }
    }
}

pub fn pub_queue(key: &Uuid, queue: &Queue) -> Markup {
    html! {
        div id={ "pub-"(key)"-queue" } class="flex flex-row items-center" {
            span class="text-nowrap whitespace-pre" { "Queue: "(queue) }
            @let bg_color = match queue {
                Queue::Short => "--color-success",
                Queue::Medium => "--color-warning",
                Queue::Long => "--color-error",
            };
            span class="status-shadow m-1 rounded-4xl p-1 shadow-lg" style={"background-color: var("(bg_color)");"} {}
        }
    }
}

pub fn pub_progress_bar(key: &Uuid, count: usize, capacity: usize) -> Markup {
    html! {
        div class="rs-progress h-3" id={ "pub-"(key)"-pb" } role="progressbar" aria-valuemin=(count) aria-valuemax=(capacity) aria-valuenow="0" aria-labelledby="pblabel" {
            div id={"pub-"(key)"-pb-inner"} class="rs-progress-bar" style={"width:"((count as f32).div(capacity as f32) * 100f32)"%"} {}
        }
    }
}

fn is_black_text(color: &str) -> bool {
    let hex = color.strip_prefix('#').unwrap_or(color);
    let (r, g, b) = match hex.len() {
        3 => (
            u32::from_str_radix(&hex[0..1], 16).unwrap_or(0) * 17,
            u32::from_str_radix(&hex[1..2], 16).unwrap_or(0) * 17,
            u32::from_str_radix(&hex[2..3], 16).unwrap_or(0) * 17,
        ),
        6 => (
            u32::from_str_radix(&hex[0..2], 16).unwrap_or(0),
            u32::from_str_radix(&hex[2..4], 16).unwrap_or(0),
            u32::from_str_radix(&hex[4..6], 16).unwrap_or(0),
        ),
        _ => return false,
    };

    (r * 299 + g * 587 + b * 114) > 125_000
}
