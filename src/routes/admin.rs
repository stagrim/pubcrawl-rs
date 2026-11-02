use axum::{Form, extract::State, response::IntoResponse};
use maud::{Markup, html};
use redis::AsyncCommands;
use serde::Deserialize;
use strum::IntoEnumIterator;

use crate::{
    AppState,
    r#pub::{Pub, PubChange, PubName, Queue},
    routes::utils::{base_templates, theme_extractor::ExplicitTheme},
};

pub async fn admin(
    State(state): State<AppState>,
    ExplicitTheme(theme): ExplicitTheme,
) -> impl IntoResponse {
    let pubs = state.pubs.read().await;

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
                            button class="btn btn-success w-full" type="submit" { "Create" }
                        }
                    }
                }

                div id="admin-pubs" class="mt-4 flex flex-col gap-2" {
                    @for (name, p) in pubs.iter() {
                        (pub_markup(name, p))
                    }
                }
            }
        },
    )
}

fn pub_markup(name: &String, p: &Pub) -> Markup {
    html! {
        div class="card card-sm bg-base-300 border-l-6" style={"border-color: "(p.color)} {
            div class="card-body items-center md:flex-row" {
                form class="flex w-full flex-col gap-3 md:flex-row md:gap-2" hx-post="" {

                    div class="flex md:flex-col w-full gap-3" {
                        label class="floating-label w-full" {
                            span {"Name:"}
                            // TODO: Update this with the same regex as on the server
                            input class="input validator" placeholder="Name" type="text" name="new-name" required value=(name);
                            input type="hidden" name="name" value=(name);
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
                            input class="input validator" placeholder="Count Key" name="key" required;
                        }
                        label class="floating-label w-full" {
                            span class="label" { "Queue" }
                            select class="select validator" placeholder="Queue length" name="queue" {
                                @for q in Queue::iter() {
                                    option value=(q) {(q)}
                                }
                            }
                        }
                    }

                    div class="flex md:flex-col w-full gap-3" {
                        label class="floating-label w-full" {
                            span {"Counted:"}
                            input class="input validator" placeholder="Counted" pattern="^[1-9]\\d*$" name="name" required value=(p.count);
                        }
                        label class="floating-label w-full" {
                            span {"Capacity:"}
                            input class="input validator" placeholder="Capacity" pattern="^[1-9]\\d*$" name="name" required value=(p.capacity);
                        }
                    }

                    div class="flex w-full flex-col md:flex-row gap-1" {
                        div class="flex flex-row md:flex-col gap-3 md:gap-1" {
                            button class="btn btn-secondary flex-1" {"Save"}
                            button class="btn btn-error flex-1" hx-delete="" hx-confirm="test" {"Remove"}
                        }

                        div class="w-full divider md:divider-horizontal divider-default" {}
                        label class="flex md:flex-col label mb-2 md:my-1.5 w-full justify-center" {
                            "Active"
                            input type="checkbox" class="toggle toggle-lg toggle-info";
                        }
                    }
                }
            }
        }
    }
}

#[derive(Deserialize)]
pub struct CreatePub {
    name: String,
    capacity: usize,
    color: String,
}

pub async fn create_pub(
    State(AppState { mut con, pubs, .. }): State<AppState>,
    Form(CreatePub {
        name,
        capacity,
        color,
    }): Form<CreatePub>,
) -> impl IntoResponse {
    // TODO: Add some regex check here for invalid characters, like ' :"' '
    let mut pubs = pubs.write().await;
    let new_pub = Pub {
        color,
        count: 0,
        capacity,
        queue: Queue::Short,
        active: false,
    };
    let _ = redis::cmd("HSET")
        .arg(format!("pub:{name}"))
        .arg(&new_pub)
        .exec_async(&mut con)
        .await
        .unwrap();
    pubs.insert(name.clone(), new_pub.clone());

    let _: () = con
        .publish(
            "pub_updates",
            serde_json::to_string(&PubChange::NewPub(PubName(name.clone()), new_pub.clone()))
                .unwrap(),
        )
        .await
        .unwrap();

    pub_markup(&name, &new_pub)
}
