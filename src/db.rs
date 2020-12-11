// /c/Program\ Files/MongoDB/Server/4.4/bin/mongod.exe

use mongodb::{error::Result, Client, Database};
use serenity::prelude::TypeMapKey;

pub struct Db;

impl TypeMapKey for Db {
    type Value = Database;
}

pub async fn init_db() -> Result<Database> {
    let client = Client::with_uri_str("mongodb://localhost:27017/")
        .await?;
    let db = client.database("ornery-bot");

    // Sad, no index-creating support yet
    // https://github.com/mongodb/mongo-rust-driver/pull/188
    // let collection = db.collection("test-count");

    Ok(db)
}
