use dotenv::dotenv;
use reqwest::{blocking::get, Url};
use serde_json::{from_reader, map::Map, Number, Value};
use serenity::{
    model::{channel::Message, gateway::Ready, id::GuildId, prelude::CurrentUser},
    prelude::*,
};
use std::{
    collections::HashMap,
    env,
    fs::{create_dir_all, File},
    sync::Mutex,
};

#[derive(Debug)]
enum DumbError {
    SerenityError(SerenityError),
    IOError(std::io::Error),
    SerdeError(serde_json::error::Error),
    ReqwestError(reqwest::Error),
    CsvError(csv::Error),
}

impl From<SerenityError> for DumbError {
    fn from(error: SerenityError) -> Self {
        DumbError::SerenityError(error)
    }
}

impl From<std::io::Error> for DumbError {
    fn from(error: std::io::Error) -> Self {
        DumbError::IOError(error)
    }
}

impl From<serde_json::error::Error> for DumbError {
    fn from(error: serde_json::error::Error) -> Self {
        DumbError::SerdeError(error)
    }
}

impl From<reqwest::Error> for DumbError {
    fn from(error: reqwest::Error) -> Self {
        DumbError::ReqwestError(error)
    }
}

impl From<csv::Error> for DumbError {
    fn from(error: csv::Error) -> Self {
        DumbError::CsvError(error)
    }
}

type MaybeError = Result<(), DumbError>;

fn serde_value_to_vec<T, F>(value: &Value, transform: F) -> Option<Vec<T>>
where
    F: FnMut(&Value) -> Option<T>,
{
    value
        .as_array()
        .and_then(|vec| Some(vec.iter().filter_map(transform).collect::<Vec<T>>()))
}

struct GuildData {
    id: GuildId,
    count: u64,
    whois_url: Option<String>,
    whois_headers: Vec<String>,
    whois_data: Vec<Vec<String>>,
}

impl GuildData {
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
                    .and_then(|value| serde_value_to_vec(value, |val| val.as_str().map(|str| String::from(str))))
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

enum GuildCommand {
    GuildCount,
    Whois(String),
    WhoisFetch(String),
}

enum Command {
    Ping,
    GlobalCount,
    Ponder,
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
                    let (_, user) = msg.content.split_at(":whois".len());
                    Self::GuildCommand(GuildCommand::Whois(String::from(user)))
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

struct Handler {
    count: Mutex<u32>,
}

impl Handler {
    fn message(&self, ctx: Context, msg: Message) -> MaybeError {
        let current_user = ctx.http.get_current_user()?;

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

            Command::Ignore => (),

            // Requires guild
            // https://docs.rs/serenity/0.8.7/serenity/client/struct.Client.html#structfield.data
            Command::GuildCommand(command) => match msg.guild_id {
                Some(guild_id) => {
                    let mut data = ctx.data.write();
                    let map = data.get_mut::<GuildDataKey>().unwrap();
                    let guild_data = map
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
                            msg.channel_id
                                .say(&ctx.http, format!("idk who {} is lmao", user))?;
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
    }

    // Finally, start a single shard, and start listening to events.
    //
    // Shards will automatically attempt to reconnect, and will perform
    // exponential backoff until it reconnects.
    if let Err(why) = client.start() {
        println!("Client error: {:?}", why);
    }
}
