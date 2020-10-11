use dotenv::dotenv;
use serde_json::{from_reader, json, Value};
use serenity::{
    model::{channel::Message, gateway::Ready, id::GuildId, prelude::CurrentUser},
    prelude::*,
};
use std::{
    collections::HashMap,
    env,
    fs::{create_dir_all, File},
    sync::Mutex,
    time::{Duration, Instant},
};

#[derive(Debug)]
enum DumbError {
    SerenityError(SerenityError),
    IOError(std::io::Error),
    JsonError(serde_json::error::Error),
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
        DumbError::JsonError(error)
    }
}

type MaybeError = Result<(), DumbError>;

struct GuildData {
    id: GuildId,
    count: u64,
    last_saved: Instant,
}

const AUTO_SAVE_SPEED: Duration = Duration::from_secs(5);

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
                last_saved: Instant::now(),
            }
        } else {
            Self {
                id,
                count: 0,
                last_saved: Instant::now(),
            }
        }
    }

    fn auto_save(&mut self) -> MaybeError {
        let now = Instant::now();
        if now - self.last_saved > AUTO_SAVE_SPEED {
            serde_json::to_writer(
                &File::create(format!("data/guilds/{}.json", self.id))?,
                &json!({
                    "count": self.count
                }),
            )?;
        }
        Ok(())
    }
}

struct GuildDataKey;

impl TypeMapKey for GuildDataKey {
    type Value = HashMap<GuildId, GuildData>;
}

enum Command {
    Ping,
    GlobalCount,
    GuildCount,
    Ponder,
    Whois(String),
    Ignore,
}

impl Command {
    fn from_msg(current_user: &CurrentUser, msg: &Message) -> Self {
        match msg.content.as_str() {
            "ok moofy" => Self::GlobalCount,
            "kk moofy" => Self::GuildCount,
            "moofy ponder" => Self::Ponder,
            _ => {
                if msg.content.starts_with(":whois") {
                    let (_, user) = msg.content.split_at(":whois".len() + 1);
                    Self::Whois(String::from(user))
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
        match Command::from_msg(&current_user, &msg) {
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
            Command::GuildCount => {
                // https://docs.rs/serenity/0.8.7/serenity/client/struct.Client.html#structfield.data
                match msg.guild_id {
                    Some(guild_id) => {
                        let count = {
                            let mut data = ctx.data.write();
                            let map = data.get_mut::<GuildDataKey>().unwrap();
                            let guild_data = map
                                .entry(guild_id)
                                .or_insert_with(|| GuildData::from_file(guild_id));
                            guild_data.count += 1;
                            guild_data.auto_save()?;
                            guild_data.count
                        };
                        msg.channel_id.say(&ctx.http, format!("{}ish", count))?;
                    }
                    None => {
                        msg.channel_id.say(&ctx.http, "server only hehehe")?;
                    }
                }
            }
            Command::Ponder => {
                msg.channel_id.say(&ctx.http, "let me think")?;
                std::thread::sleep(std::time::Duration::from_millis(5000));
                msg.channel_id.say(&ctx.http, "done")?;
            }
            Command::Whois(user) => {
                msg.channel_id
                    .say(&ctx.http, format!("idk who {} is lmao", user))?;
            }
            Command::Ignore => (),
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
