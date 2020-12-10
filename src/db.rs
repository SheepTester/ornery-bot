use mongodb::{error::Result, Client, Database};
use serenity::prelude::TypeMapKey;

pub struct Db;

impl TypeMapKey for Db {
    type Value = Database;
}

pub async fn init_db() -> Result<Database> {
    Client::with_uri_str("mongodb://localhost:27017/")
        .await
        .map(|client| client.database("ornery-bot"))
}
