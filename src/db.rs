use std::{collections::HashMap, sync::Arc, time::Duration};

use redis::{AsyncCommands, PushInfo, Value, aio::ConnectionManager};
use tokio::sync::{RwLock, RwLockWriteGuard, broadcast::Sender, mpsc::UnboundedReceiver};

use crate::r#pub::{Pub, PubChange, PubName};

pub async fn start_background_thread(
    mut con: ConnectionManager,
    mut rx: UnboundedReceiver<PushInfo>,
    pubs: Arc<RwLock<HashMap<String, Pub>>>,
    pub_change_sender: Sender<PubChange>,
) {
    tokio::spawn(async move {
        con.subscribe(&["pub_updates"]).await.unwrap();
        get_pubs(&mut con, &mut pubs.write().await).await;

        //TODO: barrier here to signal data finished loading?

        loop {
            let push_info = rx.recv().await.unwrap();

            // Disconnected logic
            if push_info.kind == redis::PushKind::Disconnection {
                let mut pubs = pubs.write().await;
                println!("Disconnected, trying to reconnect");
                pubs.clear();

                loop {
                    if let Some(_) = get_pubs(&mut con, &mut pubs).await {
                        if let Err(_) = pub_change_sender.send(PubChange::Reload) {
                            println!("No connection listening");
                        }
                        break;
                    } else {
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                }
            }

            if let [_, Value::BulkString(payload_bytes)] = &push_info.data[..] {
                let payload_str = String::from_utf8_lossy(payload_bytes);
                match serde_json::from_str::<PubChange>(&payload_str) {
                    Ok(update) => {
                        let mut pubs = pubs.write().await;

                        // Update cache
                        match update.clone() {
                            PubChange::Count(PubName(name), updated_pub) => {
                                pubs.entry(name).and_modify(|p| p.count = updated_pub.count);
                            }
                            PubChange::Capacity(PubName(name), updated_pub) => {
                                pubs.entry(name)
                                    .and_modify(|p| p.capacity = updated_pub.capacity);
                            }
                            PubChange::Color(PubName(name), updated_pub) => {
                                pubs.entry(name).and_modify(|p| p.color = updated_pub.color);
                            }
                            PubChange::NewPub(PubName(name), p) => {
                                pubs.insert(name, p);
                            }
                            PubChange::Reload => todo!(),
                        }

                        // Notify SSE threads of update
                        if let Err(_) = pub_change_sender.send(update) {
                            println!("No connection listening");
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to deserialize payload: {}", e);
                    }
                }
            }
        }
    });
}

async fn get_pubs<'a>(
    con: &mut ConnectionManager,
    pubs: &mut RwLockWriteGuard<'a, HashMap<String, Pub>>,
) -> Option<()> {
    let keys: Vec<String> = con.keys("pub:*").await.ok()?;
    for key in keys {
        pubs.insert(
            key.trim_start_matches("pub:").into(),
            con.hgetall(&key).await.unwrap(),
        );
    }
    Some(())
}
