use redis_derive::{FromRedisValue, ToRedisArgs};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, FromRedisValue, ToRedisArgs, Clone)]
pub struct Pub {
    pub color: String,
    pub count: usize,
    pub capacity: usize,
    // TODO: enum value?
    // pub queue:
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PubName(pub String);

#[derive(Serialize, Deserialize, Clone, Debug)]
// #[serde(tag = "type")] // The discriminator field
pub enum PubChange {
    Count(PubName, Pub),
    Capacity(PubName, Pub),
    Color(PubName, Pub),
    Queue(PubName, Pub),
    Reload,
}
