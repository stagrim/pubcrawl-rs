use axum::{Form, extract::State, response::IntoResponse};
use maud::html;
use redis::AsyncCommands;
use serde::Deserialize;

use crate::{
    AppState,
    r#pub::{Pub, PubChange, PubName},
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
                        form class="flex flex-col gap-1" hx-post="" {
                            label class="input w-full validator" {
                                "Name:"
                                // TODO: Update this with the same regex as on the server
                                input class="grow" type="text" name="name" required;
                            }
                            label class="input w-full validator" {
                                "Capacity:"
                                input class="grow" type="text" pattern="^[1-9]\\d*$" name="capacity" required;
                            }
                            label class="input w-full validator" {
                                "Color:"
                                input class="grow" type="text" pattern="^#([a-fA-F0-9]{6}|[a-fA-F0-9]{3})$" placeholder="#F280A1" name="color" required;
                            }
                            button class="btn btn-success w-full" type="submit" { "Create" }
                        }
                    }
                }

                div class="mt-4 flex flex-col gap-2" {
                    @for (name, p) in pubs.iter() {
                        div class="card card-sm bg-base-300 border-l-6" style={"border-color: "(p.color)} {
                            div class="card-body items-center md:flex-row" {
                                form class="flex w-full flex-col justify-between gap-0 md:flex-row md:gap-1" hx-post="" {
                                    div class="flex w-full flex-col gap-1" {
                                        label class="input validator" {
                                            span class="label" {"Name:"}
                                            // TODO: Update this with the same regex as on the server
                                            input class="" type="text" name="name" required value=(name);
                                        }
                                        label class="input validator" {
                                            span class="label" {"Color:"}
                                            input class="" type="text" pattern="^#([a-fA-F0-9]{6}|[a-fA-F0-9]{3})$" name="color" required value=(p.color);
                                        }
                                    }

                                    div class="flex w-full flex-col gap-1" {
                                        label class="input validator" {
                                            span class="label" {"Count Key:"}
                                            input class="" type="text" name="key" required;
                                        }
                                        label class="select validator" {
                                            span class="label" { "Queue" }
                                            select name="queue" {
                                                option { "Short" }
                                                option { "Medium" }
                                                option { "Long" }
                                            }
                                        }
                                    }

                                    div class="flex w-full flex-col gap-1" {
                                        label class="input validator" {
                                            span class="label" {"Counted:"}
                                            input class="" type="text" name="name" required value=(p.count);
                                        }
                                        label class="input validator" {
                                            span class="label" {"Capacity:"}
                                            input class="" type="text" name="name" required value=(p.capacity);
                                        }
                                    }

                                    div class="flex w-full flex-row gap-1" {
                                        div class="flex flex-col items-center justify-center gap-1 self-center" {
                                            button class="btn btn-secondary self-center w-full" {"Save"}
                                            button class="btn btn-error self-center w-full" hx-delete="" hx-confirm="test" {"Remove"}
                                        }
                                        div class="w-full divider divider-horizontal divider-default" {}
                                        label class="flex flex-col label my-1.5 w-full justify-center" {
                                            "Active"
                                            input type="checkbox" class="toggle toggle-lg toggle-info";
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        },
    )
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
            serde_json::to_string(&PubChange::NewPub(PubName(name), new_pub)).unwrap(),
        )
        .await
        .unwrap();
}
