use axum::{Form, extract::State, response::IntoResponse};
use maud::{Markup, html};
use redis_derive::ToRedisArgs;
use serde::Deserialize;
use strum::IntoEnumIterator;
use uuid::Uuid;

use crate::{
    AppState,
    r#pub::{Pub, PubChange, Queue},
    routes::utils::{base_templates, theme_extractor::ExplicitTheme},
};

pub async fn admin(
    State(state): State<AppState>,
    ExplicitTheme(theme): ExplicitTheme,
) -> impl IntoResponse {
    let cache = state.pubs.read().await;

    base_templates::base(
        theme,
        html! {
            div class="bg-base-100 border-base-300 p-6" {
                p { """
                This is where pubs are created. Only pubs marked as Active are shown on the front-facing main page.
            """ }
                div class="card bg-base-300 w-full" {
                    div class="card-body" {
                        h2 class="card-title" {"Create pub"}
                        form class="flex flex-col gap-1" hx-post="/pub/create" hx-target="#admin-pubs" hx-swap="beforeend" {
                            label class="input w-full validator" {
                                span class="label" {"Name:"}
                                // TODO: Update this with the same regex as on the server
                                input class="grow" type="text" name="name" required;
                            }
                            label class="input w-full validator" {
                                span class="label" {"Capacity:"}
                                input class="grow" type="text" pattern="^[1-9]\\d*$" name="capacity" required;
                            }
                            label class="input w-full validator" {
                                span class="label" {"Color:"}
                                input class="grow" type="text" pattern="^#([a-fA-F0-9]{6}|[a-fA-F0-9]{3})$" placeholder="#F280A1" name="color" required;
                            }
                            label class="input w-full validator" {
                                span class="label" {"Count Key:"}
                                input class="grow" type="text" name="count_key" required;
                            }
                            button class="btn btn-success w-full" type="submit" { "Create" }
                        }
                    }
                }

                div id="admin-pubs" class="mt-4 flex flex-col gap-2" {
                    @for (key, p) in cache.map.iter() {
                        (pub_markup(key, p))
                    }
                }
            }
        },
    )
}

fn pub_markup(key: &Uuid, p: &Pub) -> Markup {
    html! {
        div class="card card-sm bg-base-300 border-l-6" style={"border-color: "(p.color)} id={"pub-admin-"(key)} {
            div class="card-body flex-col md:flex-row md:items-center gap-3 md:gap-2" {
                form class="flex flex-1 flex-col gap-3 md:flex-row md:gap-2" hx-post="/pub/update" {
                    div class="flex md:flex-col w-full gap-3" {
                        label class="floating-label w-full" {
                            span {"Name:"}
                            // TODO: Update this with the same regex as on the server
                            input class="input validator" placeholder="Name" type="text" name="name" required value=(p.name);
                            input type="hidden" name="key" value=(key);
                        }
                        label class="floating-label w-full" {
                            span {"Color:"}
                            input
                                class="input validator"
                                placeholder="#F280A1"
                                pattern="^#([a-fA-F0-9]{6}|[a-fA-F0-9]{3})$"
                                name="color"
                                required
                                value=(p.color);
                        }
                    }

                    div class="flex md:flex-col w-full gap-3" {
                        label class="floating-label w-full" {
                            span {"Count Key:"}
                            input class="input validator" placeholder="Count Key" name="count_key" required value=(p.count_key);
                        }
                        label class="floating-label w-full" {
                            span class="label" { "Queue" }
                            select class="select validator" placeholder="Queue length" name="queue" {
                                @for q in Queue::iter() {
                                    option value=(q) selected[q == p.queue] {(q)}
                                }
                            }
                        }
                    }

                    div class="flex md:flex-col w-full gap-3" {
                        label class="floating-label w-full" {
                            span {"Counted:"}
                            input class="input validator" placeholder="Counted" pattern="^\\d+$" name="count" required value=(p.count);
                        }
                        label class="floating-label w-full" {
                            span {"Capacity:"}
                            input class="input validator" placeholder="Capacity" pattern="^[1-9]\\d*$" name="capacity" required value=(p.capacity);
                        }
                    }

                    div class="flex flex-row md:flex-col gap-3 md:gap-1" {
                        button class="btn btn-secondary flex-1" {"Save"}
                        button class="btn btn-error flex-1" hx-delete="" hx-include="closest form" hx-confirm="test" {"Remove"}
                    }
                }

                div class="divider md:divider-horizontal divider-default my-0" {}

                form class="flex items-center justify-center" {
                    label class="label flex md:flex-col gap-2" {
                        "Active"
                        input type="hidden" name="key" value=(key);
                        input
                            type="checkbox"
                            name="active"
                            value="true"
                            checked[p.active]
                            hx-post="/pub/active"
                            hx-swap="none"
                            class="toggle toggle-lg toggle-info";
                    }
                }
            }
        }
    }
}

//TODO: Make sure that count_key is unique

#[derive(Deserialize)]
pub struct CreatePub {
    name: String,
    capacity: usize,
    color: String,
    count_key: String,
}

pub async fn create_pub(
    State(AppState { mut con, .. }): State<AppState>,
    Form(CreatePub {
        name,
        capacity,
        color,
        count_key,
    }): Form<CreatePub>,
) -> impl IntoResponse {
    // TODO: Add some regex check here for invalid characters, like ' :"' '
    let key = Uuid::new_v4();
    let new_pub = Pub {
        name,
        color,
        count: 0,
        capacity,
        queue: Queue::Short,
        active: true,
        count_key,
    };
    let payload = serde_json::to_string(&PubChange::NewPub(key, new_pub.clone())).unwrap();
    let _: () = redis::pipe()
        .atomic()
        .cmd("HSET")
        .arg(format!("pub:{key}"))
        .arg(&new_pub)
        .ignore()
        .cmd("EVAL")
        .arg(crate::db::PUBLISH_EVENT_LUA)
        .arg(0)
        .arg(payload)
        .ignore()
        .exec_async(&mut con)
        .await
        .unwrap();

    pub_markup(&key, &new_pub)
}

#[derive(Deserialize)]
pub struct UpdatePubForm {
    key: Uuid,
    name: String,
    capacity: usize,
    color: String,
    queue: Queue,
    count: usize,
    count_key: String,
}

#[derive(ToRedisArgs)]
struct UpdatePub {
    name: String,
    capacity: usize,
    color: String,
    queue: Queue,
    count: usize,
    count_key: String,
}

//TODO: make the form not update volatile fields like count and queue, and allow partial updates
pub async fn update_pub(
    State(AppState { mut con, pubs, .. }): State<AppState>,
    Form(UpdatePubForm {
        key,
        name,
        capacity,
        color,
        queue,
        count,
        count_key,
    }): Form<UpdatePubForm>,
) -> impl IntoResponse {
    let active = pubs
        .read()
        .await
        .map
        .get(&key)
        .map(|p| p.active)
        .unwrap_or(true);
    let new_pub = UpdatePub {
        name,
        color,
        count,
        capacity,
        queue,
        count_key,
    };
    let updated_pub = Pub {
        name: new_pub.name.clone(),
        color: new_pub.color.clone(),
        count,
        capacity,
        queue: new_pub.queue.clone(),
        active,
        count_key: new_pub.count_key.clone(),
    };
    let payload =
        serde_json::to_string(&PubChange::UpdateEntirePub(key, updated_pub.clone())).unwrap();
    let _: () = redis::pipe()
        .atomic()
        .cmd("HSET")
        .arg(format!("pub:{key}"))
        .arg(&new_pub)
        .ignore()
        .cmd("EVAL")
        .arg(crate::db::PUBLISH_EVENT_LUA)
        .arg(0)
        .arg(payload)
        .ignore()
        .exec_async(&mut con)
        .await
        .unwrap();

    html! {
        hx-partial id={"pub-admin-"(key)} hx-swap="outerHTML" {
            (pub_markup(&key, &updated_pub))
        }
    }
}

#[derive(Deserialize)]
pub struct ToggleActive {
    key: Uuid,
    #[serde(default)]
    active: bool,
}

pub async fn toggle_active(
    State(AppState { mut con, .. }): State<AppState>,
    Form(ToggleActive { key, active }): Form<ToggleActive>,
) -> impl IntoResponse {
    let payload = serde_json::to_string(&PubChange::Active(key, active)).unwrap();
    let _: () = redis::pipe()
        .atomic()
        .hset(format!("pub:{key}"), "active", active)
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
