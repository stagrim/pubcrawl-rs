use std::{collections::HashMap, sync::Arc, thread, time::Duration};

use redis::{AsyncCommands, PushInfo, Value, aio::ConnectionManager};
use tokio::sync::{RwLock, broadcast::Sender, mpsc::UnboundedReceiver};

use crate::r#pub::{Pub, PubChange};

pub async fn start_background_thread(
    mut con: ConnectionManager,
    mut rx: UnboundedReceiver<PushInfo>,
    pubs: Arc<RwLock<HashMap<String, Pub>>>,
    mut pub_change_sender: Sender<PubChange>,
) {
    tokio::spawn(async move {
        con.subscribe(&["pub_updates"]).await.unwrap();
        // Use mutex, and lock while empty

        // con.subs
        // let entry = Pub {
        //     color: "#FFFF00".into(),
        //     count: 110,
        //     capacity: 200,
        // };

        // let _ = redis::cmd("HSET")
        //     .arg("pub:edekvata")
        //     .arg(&entry)
        //     .exec_async(&mut con)
        //     .await
        //     .unwrap();

        {
            let mut pubs = pubs.write().await;
            let keys: Vec<String> = con.keys("pub:*").await.unwrap();
            for key in keys {
                pubs.insert(
                    key.trim_start_matches("pub:").into(),
                    con.hgetall(&key).await.unwrap(),
                );
            }
        };

        // barrier here to signal data finished loading?

        loop {
            let push_info = rx.recv().await.unwrap();
            println!("Received {push_info:?}");

            if push_info.kind == redis::PushKind::Disconnection {
                let mut pubs = pubs.write().await;
                pubs.clear();

                loop {
                    println!("Here we go again");
                    //:
                    if let Ok(keys) = con.keys::<_, Vec<String>>("pub:*").await {
                        for key in keys {
                            pubs.insert(
                                key.trim_start_matches("pub:").into(),
                                con.hgetall(&key).await.unwrap(),
                            );
                        }
                        break;
                    } else {
                        thread::sleep(Duration::from_secs(1));
                    }
                }
            }

            if let [_, Value::BulkString(payload_bytes)] = &push_info.data[..] {
                let payload_str = String::from_utf8_lossy(payload_bytes);
                match serde_json::from_str::<PubChange>(&payload_str) {
                    Ok(update) => {
                        println!("Successfully deserialized update: {:?}", update);
                        pub_change_sender.send(update).unwrap();
                        //TODO: Send it to the listening websocket threads
                    }
                    Err(e) => {
                        eprintln!("Failed to deserialize payload: {}", e);
                    }
                }
            }
        }
    });
}
