use std::{collections::HashMap, sync::Arc};

use redis_derive::{FromRedisValue, ToRedisArgs};
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, FromRedisValue, ToRedisArgs, Clone)]
pub struct Pub {
    pub color: String,
    pub count: usize,
    pub capacity: usize,
    pub queue: Queue,
    pub active: bool,
    pub name: String,
    pub count_key: String,
}

#[derive(
    Debug, Serialize, Deserialize, FromRedisValue, ToRedisArgs, Clone, EnumIter, Display, PartialEq,
)]
pub enum Queue {
    Short,
    Medium,
    Long,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum PubChange {
    Count(Uuid, usize),
    NewPub(Uuid, Pub),
    Queue(Uuid, Queue),
    UpdateEntirePub(Uuid, Pub),
    Active(Uuid, bool),
    Reload,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PubEvent {
    pub version: u64,
    pub change: PubChange,
}

#[derive(Clone, Debug)]
pub struct PubBroadcast {
    pub version: u64,
    pub change: PubChange,
    pub pubs_html: Arc<str>,
}

#[derive(Default)]
pub struct PubsCache {
    pub version: u64,
    pub map: HashMap<Uuid, Pub>,
}
