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

// TODO: flip the logic with a enum of SpecificChange(Uuid, Pub, ChangeType), and JustFuckingReloadEverything for easier logic?
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum PubChange {
    Count(Uuid, usize),
    NewPub(Uuid, Pub),
    Queue(Uuid, Queue),
    UpdateEntirePub(Uuid, Pub),
    Active(Uuid, bool),
    Reload,
}
