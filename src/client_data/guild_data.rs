use super::ClientData;
use crate::{serde_conversions::serde_value_to_vec, MaybeError};
use serde_json::{from_reader, map::Map, Number, Value};
use serenity::{model::id::GuildId, prelude::*};
use std::{collections::HashMap, fs::File};

pub struct GuildData {
    pub id: GuildId,
    pub count: u64,
    pub whois_url: Option<String>,
    pub whois_headers: Vec<String>,
    pub whois_data: Vec<Vec<String>>,
}

impl GuildData {
    pub fn get_whois_entry_by_id(&self, id: &String) -> Option<&Vec<String>> {
        self.whois_headers
            .iter()
            .position(|header| header.to_ascii_lowercase().contains("id"))
            .and_then(|id_index| {
                self.whois_data
                    .iter()
                    .find(|entry| match entry.get(id_index) {
                        Some(entry_id) => entry_id == id,
                        None => false,
                    })
            })
    }
}

impl ClientData for GuildData {
    type Id = GuildId;

    fn from_file(id: GuildId) -> Self {
        if let Some(json) = File::open(format!("data/guilds/{}.json", id))
            .ok()
            .and_then(|file| from_reader::<File, Value>(file).ok())
        {
            Self {
                id,
                count: json
                    .get("count")
                    .and_then(|value| value.as_u64())
                    .unwrap_or(0),
                whois_url: json
                    .get("whois_url")
                    .and_then(|value| value.as_str())
                    .map(|str| String::from(str)),
                whois_headers: json
                    .get("whois_headers")
                    .and_then(|value| {
                        serde_value_to_vec(value, |val| val.as_str().map(|str| String::from(str)))
                    })
                    .unwrap_or_else(|| Vec::new()),
                whois_data: json
                    .get("whois_data")
                    .and_then(|value| {
                        serde_value_to_vec(value, |val| {
                            serde_value_to_vec(val, |v| v.as_str().map(|str| String::from(str)))
                        })
                    })
                    .unwrap_or_else(|| Vec::new()),
            }
        } else {
            Self {
                id,
                count: 0,
                whois_url: None,
                whois_headers: Vec::new(),
                whois_data: Vec::new(),
            }
        }
    }

    fn save(&mut self) -> MaybeError {
        let mut object = Map::new();
        if let Some(number) = Number::from_f64(self.count as f64) {
            object.insert(String::from("count"), Value::Number(number));
        }
        object.insert(
            String::from("whois_url"),
            self.whois_url
                .as_ref()
                .map_or_else(|| Value::Null, |str| Value::String(str.clone())),
        );
        object.insert(
            String::from("whois_headers"),
            Value::Array(
                self.whois_headers
                    .iter()
                    .map(|str| Value::String(str.clone()))
                    .collect(),
            ),
        );
        object.insert(
            String::from("whois_data"),
            Value::Array(
                self.whois_data
                    .iter()
                    .map(|vec| {
                        Value::Array(vec.iter().map(|str| Value::String(str.clone())).collect())
                    })
                    .collect(),
            ),
        );
        let json = Value::Object(object);
        serde_json::to_writer(
            &File::create(format!("data/guilds/{}.json", self.id))?,
            &json,
        )?;
        Ok(())
    }
}

pub struct GuildDataKey;

impl TypeMapKey for GuildDataKey {
    type Value = HashMap<GuildId, GuildData>;
}
