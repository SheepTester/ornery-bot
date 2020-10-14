use super::ClientData;
use crate::{
    serde_conversions::{hashmap_to_serde_object, serde_map_to_hashmap, serde_value_to_vec},
    MaybeError,
};
use serde_json::{from_reader, map::Map, Value};
use serenity::{model::id::GuildId, prelude::*};
use std::{collections::HashMap, fs::File};

pub struct GuildData {
    pub id: GuildId,
    pub count: u64,
    pub whois_url: Option<String>,
    pub whois_headers: Vec<String>,
    pub whois_data: Vec<Vec<String>>,
    pub webtoons: HashMap<String, String>,
    json: Value,
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

    pub fn update_count(&mut self) {
        if let Value::Object(map) = &mut self.json {
            map.insert(String::from("count"), Value::from(self.count));
        } else {
            self.ensure_init_json();
        }
    }

    pub fn update_whois(&mut self) {
        if let Value::Object(map) = &mut self.json {
            map.insert(
                String::from("whois_url"),
                self.whois_url
                    .as_ref()
                    .map_or_else(|| Value::Null, |str| Value::from(str.clone())),
            );
            map.insert(
                String::from("whois_headers"),
                Value::from(self.whois_headers.clone()),
            );
            map.insert(
                String::from("whois_data"),
                Value::from(self.whois_data.clone()),
            );
        } else {
            self.ensure_init_json();
        }
    }

    pub fn update_webtoons(&mut self) {
        if let Value::Object(map) = &mut self.json {
            map.insert(
                String::from("webtoons"),
                hashmap_to_serde_object(&self.webtoons, |str| Value::from(str.clone())),
            );
        } else {
            self.ensure_init_json();
        }
    }

    /// Returns whether the JSON has just been initialized
    fn ensure_init_json(&mut self) -> bool {
        match self.json {
            Value::Object(_) => false,
            _ => {
                self.json = Value::Object(Map::new());
                self.update_count();
                self.update_whois();
                self.update_webtoons();
                true
            }
        }
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
                webtoons: json
                    .get("webtoons")
                    .and_then(|value| value.as_object())
                    .map(|map| {
                        serde_map_to_hashmap(map, |value| {
                            value.as_str().map(|str| String::from(str))
                        })
                    })
                    .unwrap_or_else(|| HashMap::new()),
                json: Value::Null,
            }
        } else {
            Self {
                id,
                count: 0,
                whois_url: None,
                whois_headers: Vec::new(),
                whois_data: Vec::new(),
                webtoons: HashMap::new(),
                json: Value::Null,
            }
        }
    }

    fn save(&mut self) -> MaybeError {
        self.ensure_init_json();
        serde_json::to_writer(
            &File::create(format!("data/guilds/{}.json", self.id))?,
            &self.json,
        )?;
        Ok(())
    }
}

pub struct GuildDataKey;

impl TypeMapKey for GuildDataKey {
    type Value = HashMap<GuildId, GuildData>;
}
