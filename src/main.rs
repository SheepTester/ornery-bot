pub mod dumb_error;

use dotenv::dotenv;
use dumb_error::MaybeError;
use regex::Regex;
use reqwest::{blocking::get, Url};
use serde_json::{from_reader, map::Map, Number, Value};
use serenity::{
    model::{channel::Message, gateway::Ready, id::GuildId, prelude::CurrentUser},
    prelude::*,
};
use std::{
    cmp::Eq,
    collections::HashMap,
    env,
    fs::{create_dir_all, File},
    hash::Hash,
    marker::{Send, Sync},
    sync::Mutex,
};
#[macro_use]
extern crate lazy_static;

fn serde_value_to_vec<T, F>(value: &Value, transform: F) -> Option<Vec<T>>
where
    F: FnMut(&Value) -> Option<T>,
{
    value
        .as_array()
        .and_then(|vec| Some(vec.iter().filter_map(transform).collect::<Vec<T>>()))
}

fn serde_map_to_hashmap<V, F>(map: &Map<String, Value>, transform: F) -> HashMap<String, V>
where
    F: Fn(&Value) -> Option<V>,
{
    let map_iter = map.iter();
    let (min_size, _) = map_iter.size_hint();
    let mut hash_map = HashMap::with_capacity(min_size);
    for (key, value) in map_iter {
        if let Some(val) = transform(value) {
            hash_map.insert(key.to_owned(), val);
        }
    }
    hash_map
}

fn hashmap_to_serde_object<V, F>(hash_map: &HashMap<String, V>, transform: F) -> Value
where
    F: Fn(&V) -> Option<Value>,
{
    let hash_map_iter = hash_map.iter();
    let (min_size, _) = hash_map_iter.size_hint();
    let mut map = Map::with_capacity(min_size);
    for (key, value) in hash_map_iter {
        if let Some(val) = transform(value) {
            map.insert(key.to_owned(), val);
        }
    }
    Value::Object(map)
}

trait ClientData {
    // As needed by serenity::prelude::ShareMap and HashMap's K
    type Id: 'static + Sync + Send + Eq + Hash + Copy;
    fn from_file(id: Self::Id) -> Self;
    fn save(&mut self) -> MaybeError;
}

struct GuildData {
    id: GuildId,
    count: u64,
    whois_url: Option<String>,
    whois_headers: Vec<String>,
    whois_data: Vec<Vec<String>>,
}

impl GuildData {
    fn get_whois_entry_by_id(&self, id: &String) -> Option<&Vec<String>> {
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

struct GuildDataKey;

impl TypeMapKey for GuildDataKey {
    type Value = HashMap<GuildId, GuildData>;
}

struct EmojiData {
    id: GuildId,
    emoji: HashMap<String, u64>,
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

struct EmojiDataKey;

impl TypeMapKey for EmojiDataKey {
    type Value = HashMap<GuildId, EmojiData>;
}

enum GuildCommand {
    GuildCount,
    Whois(String),
    WhoisFetch(String),
}

enum Command {
    Ping,
    GlobalCount,
    Ponder,
    WhoisOld,
    GuildCommand(GuildCommand),
    Ignore,
}

impl Command {
    fn parse(msg: &Message, current_user: &CurrentUser) -> Self {
        match msg.content.as_str() {
            "ok moofy" => Self::GlobalCount,
            "kk moofy" => Self::GuildCommand(GuildCommand::GuildCount),
            "moofy ponder" => Self::Ponder,
            _ => {
                if msg.content.starts_with(":whois") {
                    Self::WhoisOld
                } else if msg.content.starts_with("bruh who is") {
                    let (_, user) = msg.content.split_at("bruh who is".len());
                    Self::GuildCommand(GuildCommand::Whois(String::from(user.trim())))
                } else if msg.content.starts_with("moofy go fetch") {
                    let (_, url) = msg.content.split_at("moofy go fetch".len());
                    Self::GuildCommand(GuildCommand::WhoisFetch(String::from(url)))
                } else if msg.mentions_user_id(current_user) {
                    Self::Ping
                } else {
                    Self::Ignore
                }
            }
        }
    }
}

// fn get_client_data<'a, K, D>(ctx: &'a Context, id: D::Id) -> &'a mut D
// where
//     K: TypeMapKey<Value = HashMap<D::Id, D>>,
//     // D is 'static because of D::Id
//     D: ClientData + 'static + Sync + Send,
// {
//     ctx.data
//         .write()
//         .get_mut::<K>()
//         .unwrap()
//         .entry(id)
//         .or_insert_with(|| D::from_file(id))
// }

struct Handler {
    count: Mutex<u32>,
}

impl Handler {
    fn message(&self, ctx: Context, msg: Message) -> MaybeError {
        if msg.author.bot {
            return Ok(());
        }

        let current_user = ctx.http.get_current_user()?;

        if let Some(guild_id) = msg.guild_id {
            lazy_static! {
                static ref EMOJI: Regex = Regex::new(r"<a?:\w+:(\d+)>").unwrap();
            }
            let mut data = ctx.data.write();
            let emoji_data = data
                .get_mut::<EmojiDataKey>()
                .unwrap()
                .entry(guild_id)
                .or_insert_with(|| EmojiData::from_file(guild_id));
            for captures in EMOJI.captures_iter(msg.content.as_str()) {
                if let Some(rmatch) = captures.get(1) {
                    let count = emoji_data
                        .emoji
                        .entry(String::from(rmatch.as_str()))
                        .or_insert(0);
                    *count += 1;
                }
            }
            emoji_data.save()?;
        }

        match Command::parse(&msg, &current_user) {
            Command::Ping => {
                msg.channel_id
                    .say(&ctx.http, "<:ping:719277539113041930>")?;
            }

            Command::GlobalCount => {
                // https://doc.rust-lang.org/book/ch16-03-shared-state.html#sharing-a-mutext-between-multiple-threads
                let count = {
                    let mut count = self.count.lock().unwrap();
                    *count += 1;
                    count
                };
                msg.channel_id.say(&ctx.http, count)?;
            }

            Command::Ponder => {
                msg.channel_id.say(&ctx.http, "let me think")?;
                std::thread::sleep(std::time::Duration::from_millis(5000));
                msg.channel_id.say(&ctx.http, "done")?;
            }

            Command::WhoisOld => {
                msg.channel_id.say(
                    &ctx.http,
                    "-5 karma. im emo now so pls use `bruh who is <name>`",
                )?;
            }

            Command::Ignore => (),

            // Requires guild
            // https://docs.rs/serenity/0.8.7/serenity/client/struct.Client.html#structfield.data
            Command::GuildCommand(command) => match msg.guild_id {
                Some(guild_id) => {
                    let mut data = ctx.data.write();
                    let guild_data = data
                        .get_mut::<GuildDataKey>()
                        .unwrap()
                        .entry(guild_id)
                        .or_insert_with(|| GuildData::from_file(guild_id));

                    match command {
                        GuildCommand::GuildCount => {
                            guild_data.count += 1;
                            msg.channel_id
                                .say(&ctx.http, format!("{}ish", guild_data.count))?;
                            guild_data.save()?;
                        }

                        GuildCommand::Whois(user) => {
                            lazy_static! {
                                static ref DIGITS: Regex = Regex::new(r"\d+").unwrap();
                            }
                            match DIGITS.find(user.as_str())
                                .and_then(|m| guild_data.get_whois_entry_by_id(&String::from(m.as_str())).map(|entry| (String::from(m.as_str()), entry)))
                                .or_else(|| {
                                    msg.channel_id.say(&ctx.http, "a literacy test is now required to get a user by their nickname/username. please read https://docs.rs/serenity/0.8.7/serenity/index.html and answer the following question: 1. how does one ensure that Serenity caches the members of a guild in order to prevent needlessly fetching from the API?").ok();
                                    None
                                }) {
                                Some((id, entry)) => {
                                    msg.channel_id.send_message(&ctx.http, |message| {
                                        message.embed(|embed| {
                                            embed.title("Watchlist | fbi.gov");
                                            embed.description(format!(
                                                "Information about Discord user <@{}>",
                                                id
                                            ));
                                            for (header, value) in
                                                guild_data.whois_headers.iter().zip(entry.iter())
                                            {
                                                if !value.is_empty() {
                                                    embed.field(header, value, true);
                                                }
                                            }
                                            embed
                                        });
                                        message.content("found this on google idk hope it helps");
                                        message
                                    })?;
                                }
                                _ => {
                                    msg.channel_id.say(&ctx.http, "idk who that is lmao")?;
                                }
                            }
                            println!("end {}", user);
                        }

                        GuildCommand::WhoisFetch(url) => {
                            if guild_id
                                .member(&ctx.http, msg.author)?
                                .permissions(&ctx.cache)?
                                .manage_guild()
                            {
                                match Url::parse(&url).ok().or(guild_data
                                    .whois_url
                                    .as_ref()
                                    .and_then(|url| Url::parse(url).ok()))
                                {
                                    Some(url) => match get(url.as_str()) {
                                        Ok(response) => {
                                            let mut rdr = csv::Reader::from_reader(response);
                                            guild_data.whois_url = Some(String::from(url.as_str()));
                                            guild_data.whois_headers = rdr
                                                .headers()?
                                                .iter()
                                                .map(|str| String::from(str))
                                                .collect();
                                            guild_data.whois_data = rdr
                                                .records()
                                                .filter_map(|result| result.ok())
                                                .map(|record| {
                                                    record
                                                        .iter()
                                                        .map(|str| String::from(str))
                                                        .collect()
                                                })
                                                .collect();
                                            guild_data.save()?;
                                            msg.channel_id.say(&ctx.http, "k")?;
                                        }
                                        Err(error) => {
                                            msg.channel_id.say(&ctx.http, "i tripped and fel")?;
                                            Err(error)?;
                                        }
                                    },
                                    None => {
                                        msg.channel_id.say(
                                            &ctx.http,
                                            "uhhh where lol can u give me a url thx",
                                        )?;
                                    }
                                }
                            } else {
                                msg.channel_id
                                    .say(&ctx.http, "u cant even manage the server and you want ME to fetch it for u? lmao")?;
                            }
                        }
                    }
                }
                None => {
                    msg.channel_id.say(&ctx.http, "server only hehehe")?;
                }
            },
        }
        return Ok(());
    }

    fn ready(&self, _: Context, ready: Ready) -> MaybeError {
        println!("{} is connected!", ready.user.name);
        return Ok(());
    }
}

impl EventHandler for Handler {
    fn message(&self, ctx: Context, msg: Message) {
        if let Err(why) = self.message(ctx, msg) {
            println!("Error from message handler: {:?}", why);
        }
    }

    fn ready(&self, ctx: Context, ready: Ready) {
        if let Err(why) = self.ready(ctx, ready) {
            println!("Error from ready handler: {:?}", why);
        }
    }
}

fn main() {
    dotenv().ok();

    // Configure the client with your Discord bot token in the environment.
    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");

    create_dir_all("data/guilds").expect("Error creating guild data folder");
    create_dir_all("data/users").expect("Error creating user data folder");
    create_dir_all("data/emoji").expect("Error creating emoji data folder");

    let mut client = Client::new(
        &token,
        Handler {
            count: Mutex::new(0),
        },
    )
    .expect("Error creating client");

    {
        let mut data = client.data.write();
        data.insert::<GuildDataKey>(HashMap::default());
        data.insert::<EmojiDataKey>(HashMap::default());
    }

    // Finally, start a single shard, and start listening to events.
    //
    // Shards will automatically attempt to reconnect, and will perform
    // exponential backoff until it reconnects.
    if let Err(why) = client.start() {
        println!("Client error: {:?}", why);
    }
}
