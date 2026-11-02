use redis_derive::{FromRedisValue, ToRedisArgs};
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter};

#[derive(Debug, Serialize, Deserialize, FromRedisValue, ToRedisArgs, Clone)]
pub struct Pub {
    pub color: String,
    pub count: usize,
    pub capacity: usize,
    pub queue: Queue,
    pub active: bool,
    // pub count_key: String,
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
pub struct PubName(pub String);

// TODO: flip the logic with a enum of SpecificChange(PubName, Pub, ChangeType), and JustFuckingReloadEverything for easier logic?
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum PubChange {
    Count(PubName, Pub),
    Capacity(PubName, Pub),
    Color(PubName, Pub),
    NewPub(PubName, Pub),
    Queue(PubName, Pub),
    Reload,
}
