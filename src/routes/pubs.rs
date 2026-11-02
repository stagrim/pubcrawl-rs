use std::{collections::hash_map::Iter, ops::Div};

use axum::{extract::State, response::IntoResponse};
use maud::{Markup, html};

use crate::{
    AppState,
    r#pub::{Pub, Queue},
    routes::utils::{base_templates, theme_extractor::ExplicitTheme},
};

pub async fn pubs(
    State(state): State<AppState>,
    ExplicitTheme(theme): ExplicitTheme,
) -> impl IntoResponse {
    let pubs = state.pubs.read().await;

    base_templates::base(
        theme,
        html! {
            div id="pubs" hx-sse:connect="/pub-updates" {
                (pubs_markup(pubs.iter()))
            }
        },
    )
}

pub fn pubs_markup(pubs: Iter<String, Pub>) -> Markup {
    let mut pubs = pubs
        .map(|(name, p)| (name, p, pub_order_weight(p)))
        .collect::<Vec<_>>();
    pubs.sort_unstable_by_key(|(_, _, weight)| -weight);

    html! {
        @for (name, p, weight) in pubs {
            (pub_markup(name, p, weight))
        }
    }
}

pub fn pub_order_weight(p: &Pub) -> isize {
    (p.count as f32 / p.capacity as f32 * 1000_000.0) as isize
}

pub fn pub_markup(name: &str, p: &Pub, weight: isize) -> Markup {
    html! {
        div
            class={
                "card dark:bg-opacity-100 bg-opacity-80 m-1 border-l-6 shadow backdrop-blur-2xl border-[color-mix(in_oklab,currentColor_15%,transparent)] "
                (if is_black_text(&p.color) {"text-black"} else {"text-white"})
            }
            style={"background-color:"(p.color)}
            data-id=(name)
        {
            input type="hidden" name="item" value=(weight) id={"pub-value-"(name)};
            div class="card-body" {
                div class="card-header flex flex-row justify-between" {
                    h2 class="card-title text-xl" {
                        (name)
                    }
                    div class="flex flex-col" {
                        p id={"pub-"(name)} class="text-2xl font-bold" {
                            span id=(format!("{name}-count")) {(p.count)}"/"(p.capacity)
                        }
                    }
                }
                div class="flex flex-row items-center gap-2" {
                    (pub_queue(name, &p))
                    div class="flex-grow-1" {
                        (pub_progress_bar(name, &p))
                    }
                }
            }
        }
    }
}

pub fn pub_queue(name: &str, p: &Pub) -> Markup {
    html! {
        div id={ "pub-"(name)"-queue" } class="flex flex-row items-center" {
            span class="text-nowrap whitespace-pre" { "Queue: "(p.queue) }
            @let bg_color = match p.queue {
                Queue::Short => "bg-success",
                Queue::Medium => "bg-warning",
                Queue::Long => "bg-error",
            };
            span class={"status-shadow m-1 rounded-4xl p-1 shadow-lg " (bg_color)} {}
        }
    }
}

pub fn pub_progress_bar(name: &str, p: &Pub) -> Markup {
    html! {
        div class="rs-progress h-3" id={ "pub-"(name)"-pb" } role="progressbar" aria-valuemin=(p.count) aria-valuemax=(p.capacity) aria-valuenow="0" aria-labelledby="pblabel" {
            div id={"pub-"(name)"-pb-inner"} class="rs-progress-bar" style={"width:"((p.count as f32).div(p.capacity as f32) * 100f32)"%"} {}
        }
    }
}

// TODO: Better error handling with options instead
fn is_black_text(color: &str) -> bool {
    let hex_part = &color[1..];

    let full_hex = if hex_part.len() == 3 {
        let mut expanded = String::with_capacity(6);
        for c in hex_part.chars() {
            expanded.push(c);
            expanded.push(c);
        }
        expanded
    } else if hex_part.len() == 6 {
        hex_part.to_string()
    } else {
        panic!("Invalid length");
    };

    let red_str = &full_hex[0..2];
    let green_str = &full_hex[2..4];
    let blue_str = &full_hex[4..6];

    let red = usize::from_str_radix(red_str, 16).unwrap();
    let green = usize::from_str_radix(green_str, 16).unwrap();
    let blue = usize::from_str_radix(blue_str, 16).unwrap();

    ((red * 299 + green * 587 + blue * 114) / 1000) > 125
}
