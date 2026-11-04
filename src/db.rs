use std::{collections::HashMap, str::FromStr, sync::Arc, time::Duration};

use maud::html;
use redis::{PushInfo, Value, aio::ConnectionManager};
use tokio::sync::{RwLock, broadcast::Sender, mpsc::UnboundedReceiver};
use uuid::Uuid;

use crate::{
    r#pub::{Pub, PubBroadcast, PubChange, PubEvent, PubsCache},
    routes::pubs::{pub_markup, pub_order_weight, pub_progress_bar, pub_queue, pubs_markup},
};

pub const PUBLISH_EVENT_LUA: &str = r#"
local v = redis.call("INCR", "pub_version")
local change = cjson.decode(ARGV[1])
redis.call("PUBLISH", "pub_updates", cjson.encode({version = v, change = change}))
return v
"#;

pub async fn start_background_thread(
    mut con: ConnectionManager,
    mut rx: UnboundedReceiver<PushInfo>,
    pubs: Arc<RwLock<PubsCache>>,
    pub_change_sender: Sender<PubBroadcast>,
) {
    con.subscribe(&["pub_updates"]).await.unwrap();
    if let Some((version, loaded)) = fetch_all_pubs(&mut con).await {
        let mut cache = pubs.write().await;
        cache.map = loaded;
        cache.version = version;
    }

    tokio::spawn(async move {
        loop {
            let Some(push_info) = rx.recv().await else {
                break;
            };

            if push_info.kind == redis::PushKind::Disconnection {
                println!("Disconnected, trying to reconnect");
                pubs.write().await.map.clear();

                loop {
                    if let Some((version, loaded)) = fetch_all_pubs(&mut con).await {
                        let pubs_html: Arc<str> = {
                            let mut cache = pubs.write().await;
                            cache.map = loaded;
                            cache.version = version;
                            html! {
                                hx-partial id="pubs" {
                                    (pubs_markup(cache.map.iter()))
                                }
                            }
                            .into_string()
                            .into()
                        };
                        let _ = pub_change_sender.send(PubBroadcast {
                            version,
                            change: PubChange::Reload,
                            pubs_html,
                        });
                        break;
                    } else {
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                }
                continue;
            }

            if let [_, Value::BulkString(payload_bytes)] = &push_info.data[..] {
                let payload_str = String::from_utf8_lossy(payload_bytes);
                match serde_json::from_str::<PubEvent>(&payload_str) {
                    Ok(PubEvent {
                        version,
                        change: update,
                    }) => {
                        // Check if this replica missed any intermediate messages (e.g. version gap)
                        let current_version = pubs.read().await.version;
                        let missed_messages = current_version != 0 && version > current_version + 1;

                        let (final_version, final_change, pubs_html): (u64, PubChange, Arc<str>) =
                            if matches!(update, PubChange::Reload) || missed_messages {
                                if let Some((loaded_version, loaded)) =
                                    fetch_all_pubs(&mut con).await
                                {
                                    let mut cache = pubs.write().await;
                                    cache.map = loaded;
                                    cache.version = loaded_version;
                                    let html_str = html! {
                                        hx-partial id="pubs" {
                                            (pubs_markup(cache.map.iter()))
                                        }
                                    }
                                    .into_string()
                                    .into();
                                    (loaded_version, PubChange::Reload, html_str)
                                } else {
                                    continue;
                                }
                            } else {
                                let mut cache = pubs.write().await;
                                let snippet: Arc<str> = match &update {
                                    PubChange::Count(key, count) => {
                                        if let Some(p) = cache.map.get_mut(key) {
                                            p.count = *count;
                                            let weight = pub_order_weight(p);
                                            let capacity = p.capacity;
                                            html! {
                                                hx-partial id={"pub-value-"(key)} hx-swap="outerHTML" {
                                                    input type="hidden" name="item" value=(weight) id={"pub-value-"(key)};
                                                }
                                                hx-partial id={(key)"-count"} { (count) }
                                                hx-partial id={"pub-"(key)"-pb"} hx-swap="outerHTML" {
                                                    (pub_progress_bar(key, *count, capacity))
                                                }
                                            }
                                            .into_string()
                                            .into()
                                        } else {
                                            continue;
                                        }
                                    }
                                    PubChange::NewPub(key, p) => {
                                        let snippet = html! {
                                            hx-partial id="pubs" hx-swap="beforeend" {
                                                (pub_markup(key, p, pub_order_weight(p)))
                                            }
                                        }
                                        .into_string()
                                        .into();
                                        cache.map.insert(*key, p.clone());
                                        snippet
                                    }
                                    PubChange::Queue(key, queue) => {
                                        if let Some(p) = cache.map.get_mut(key) {
                                            p.queue = queue.clone();
                                        }
                                        html! {
                                            hx-partial id={"pub-"(key)"-queue"} hx-swap="outerHTML" {
                                                (pub_queue(key, queue))
                                            }
                                        }
                                        .into_string()
                                        .into()
                                    }
                                    PubChange::UpdateEntirePub(key, updated_pub) => {
                                        let snippet = html! {
                                            hx-partial id={"pub-card-"(key)} hx-swap="outerHTML" {
                                                (pub_markup(key, updated_pub, pub_order_weight(updated_pub)))
                                            }
                                        }
                                        .into_string()
                                        .into();
                                        cache.map.insert(*key, updated_pub.clone());
                                        snippet
                                    }
                                    PubChange::Active(key, active) => {
                                        if let Some(p) = cache.map.get_mut(key) {
                                            p.active = *active;
                                            if *active {
                                                html! {
                                                    hx-partial id="pubs" hx-swap="beforeend" {
                                                        (pub_markup(key, p, pub_order_weight(p)))
                                                    }
                                                }
                                                .into_string()
                                                .into()
                                            } else {
                                                html! {
                                                    hx-partial id={"pub-card-"(key)} hx-swap="delete" {}
                                                }
                                                .into_string()
                                                .into()
                                            }
                                        } else {
                                            continue;
                                        }
                                    }
                                    PubChange::Reload => unreachable!(),
                                };
                                cache.version = version;
                                (version, update, snippet)
                            };

                        // Notify SSE threads of update
                        let _ = pub_change_sender.send(PubBroadcast {
                            version: final_version,
                            change: final_change,
                            pubs_html,
                        });
                    }
                    Err(e) => {
                        eprintln!("Failed to deserialize payload: {}", e);
                    }
                }
            }
        }
    });
}

const FETCH_ALL_PUBS_LUA: &str = r#"
local version = tonumber(redis.call("GET", "pub_version") or "0")

local keys = {}
local cursor = "0"
repeat
    local res = redis.call("SCAN", cursor, "MATCH", "pub:*", "COUNT", 100)
    cursor = res[1]
    for _, key in ipairs(res[2]) do
        table.insert(keys, key)
    end
until cursor == "0"

local pubs = {}
for i, key in ipairs(keys) do
    pubs[i] = redis.call("HGETALL", key)
end

return { version, keys, pubs }
"#;

async fn fetch_all_pubs(con: &mut ConnectionManager) -> Option<(u64, HashMap<Uuid, Pub>)> {
    let (version, keys, pubs_vec): (u64, Vec<String>, Vec<Pub>) =
        redis::Script::new(FETCH_ALL_PUBS_LUA)
            .invoke_async(con)
            .await
            .map_err(|e| eprintln!("Failed to fetch pubs snapshot: {e}"))
            .ok()?;

    let map = keys
        .into_iter()
        .zip(pubs_vec)
        .filter_map(|(key, p)| {
            let uuid = Uuid::from_str(key.trim_start_matches("pub:")).ok()?;
            Some((uuid, p))
        })
        .collect();

    Some((version, map))
}
