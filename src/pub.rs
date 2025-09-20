use redis_derive::{FromRedisValue, ToRedisArgs};
use serde::{Deserialize, Serialize};

#[derive(Debug, FromRedisValue, ToRedisArgs)]
pub struct Pub {
    pub color: String,
    pub count: usize,
    pub capacity: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PubName(pub String);

#[derive(Serialize, Deserialize, Clone, Debug)]
// #[serde(tag = "type")] // The discriminator field
pub enum PubChange {
    Count(PubName, usize),
    Capacity(PubName, usize),
    Color(PubName, String),
    Reload,
}
