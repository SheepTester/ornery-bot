use super::ClientData;
use crate::{
    serde_conversions::{hashmap_to_serde_object, serde_map_to_hashmap},
    MaybeError,
};
use serde_json::{from_reader, Number, Value};
use serenity::{model::id::GuildId, prelude::*};
use std::{collections::HashMap, fs::File};

pub struct EmojiData {
    pub id: GuildId,
    pub emoji: HashMap<String, u64>,
}

impl ClientData for EmojiData {
    type Id = GuildId;

    fn from_file(id: GuildId) -> Self {
        if let Some(json) = File::open(format!("data/emoji/{}.json", id))
            .ok()
            .and_then(|file| from_reader::<File, Value>(file).ok())
        {
            Self {
                id,
                emoji: json
                    .as_object()
                    .map(|map| serde_map_to_hashmap(map, |value| value.as_u64()))
                    .unwrap_or_else(|| HashMap::new()),
            }
        } else {
            Self {
                id,
                emoji: HashMap::new(),
            }
        }
    }

    fn save(&mut self) -> MaybeError {
        let json = hashmap_to_serde_object(&self.emoji, |num| {
            Number::from_f64(*num as f64).map(|float| Value::Number(float))
        });
        serde_json::to_writer(
            &File::create(format!("data/emoji/{}.json", self.id))?,
            &json,
        )?;
        Ok(())
    }
}

pub struct EmojiDataKey;

impl TypeMapKey for EmojiDataKey {
    type Value = HashMap<GuildId, EmojiData>;
}
