pub mod client_data;
pub mod dumb_error;
pub mod serde_conversions;

#[macro_use]
extern crate lazy_static;

use client_data::{
    emoji_data::{EmojiData, EmojiDataKey},
    guild_data::{GuildData, GuildDataKey},
    ClientData,
};
use dotenv::dotenv;
pub use dumb_error::{DumbError, MaybeError};
use regex::Regex;
use reqwest::{blocking::get, Url};
use serenity::{
    model::{channel::Message, gateway::Ready, prelude::CurrentUser},
    prelude::*,
};
use std::{collections::HashMap, env, fs::create_dir_all, sync::Mutex};

/// Sends a message with a very minimal embed, mostly to display untrusted user input (*cough*
/// chop0).
fn send_with_embed(
    ctx: &Context,
    msg: &Message,
    content: &str,
    embed_content: &String,
) -> MaybeError {
    msg.channel_id.send_message(&ctx.http, |message| {
        message.embed(|embed| {
            embed.description(embed_content);
            embed
        });
        message.content(content);
        message
    })?;
    Ok(())
}

enum GuildCommand {
    GuildCount,
    Whois(String),
    WhoisFetch(String),
    GetWebtoon(String),
    AddWebtoon(String, String),
    RemoveWebtoon(String),
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

                        GuildCommand::GetWebtoon(webtoon_id) => {
                            // problem: Result<(), DumbError>
                            // There's no actual problem if there's a ().
                            if let Err(problem) = guild_data.webtoons.get(&webtoon_id).ok_or_else(|| {
                                msg.channel_id
                                    .say(&ctx.http, "omg is that another webtoon can i have the url plssss (`moofy check this out <url>`)")
                                    .map(|_| ())
                                    .map_err(|err| err.into())
                            }).and_then(|url_str| {
                                Url::parse(&url_str).map_err(|_| {
                                    send_with_embed(
                                        &ctx,
                                        &msg,
                                        "uhhh i dont think u gave me a url lol",
                                        url_str,
                                    )
                                })
                            }).and_then(|url| get(url.as_str()).map_err(|error| {
                                msg.channel_id
                                    .say(&ctx.http, "hmm the url doesnt seem to work")
                                    .or(Err(error))
                                    .map(|_| ())
                                    .map_err(|err| err.into())
                            })).and_then::<(), _>(|response| {
                                //
                                Err(Ok(()))
                            }) {
                                problem?;
                            }
                        }

                        GuildCommand::AddWebtoon(_, _) => {
                            unimplemented!();
                        }

                        GuildCommand::RemoveWebtoon(_) => {
                            unimplemented!();
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
