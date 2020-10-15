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
    model::{
        channel::{Message, Reaction, ReactionType},
        gateway::Ready,
        id::GuildId,
        prelude::{Activity, CurrentUser},
    },
    prelude::*,
};
use std::{
    collections::{hash_map::Entry, HashMap},
    env,
    fs::create_dir_all,
    sync::Mutex,
};

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

/// Maximum number of emoji that can be sent in the most/least used emoji commands
const MAX_EMOJI: usize = 25;

enum WhoisResolution<'a> {
    EntryFound(String, &'a Vec<String>),
    UserFound(String),
    NoUser,
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
    Help,
    MostUsedEmoji(usize),
    LeastUsedEmoji(usize),
    GuildCommand(GuildCommand),
    Ignore,
}

impl Command {
    fn parse(msg: &Message, current_user: &CurrentUser) -> Self {
        match msg.content.as_str() {
            "ok moofy" => Self::GlobalCount,
            "kk moofy" => Self::GuildCommand(GuildCommand::GuildCount),
            "moofy ponder" => Self::Ponder,
            "moofy what pisses you off" => Self::Help,
            _ => {
                lazy_static! {
                    static ref ADD_WEBTOON: Regex =
                        Regex::new(r"moofy check out (\w+) at (.+)").unwrap();
                    static ref REMOVE_WEBTOON: Regex =
                        Regex::new(r"moofy do not read (\w+)").unwrap();
                    static ref MOST_USED_EMOJI: Regex =
                        Regex::new(r"top (\d+) most used emoji").unwrap();
                    static ref LEAST_USED_EMOJI: Regex =
                        Regex::new(r"top (\d+) least used emoji").unwrap();
                }
                if msg.content.starts_with(":whois") {
                    Self::WhoisOld
                } else if msg.content.starts_with("bruh who is") {
                    let (_, user) = msg.content.split_at("bruh who is".len());
                    Self::GuildCommand(GuildCommand::Whois(String::from(user.trim())))
                } else if msg.content.starts_with("moofy go fetch") {
                    let (_, url) = msg.content.split_at("moofy go fetch".len());
                    Self::GuildCommand(GuildCommand::WhoisFetch(String::from(url)))
                } else if msg.content.chars().nth(0) == Some(':') {
                    Self::GuildCommand(GuildCommand::GetWebtoon(String::from(
                        msg.content.get(1..).unwrap_or(""),
                    )))
                } else if msg.mentions_user_id(current_user) {
                    Self::Ping
                } else if let Some(captures) = ADD_WEBTOON.captures(msg.content.as_str()) {
                    Self::GuildCommand(GuildCommand::AddWebtoon(
                        String::from(captures.get(1).map(|rmatch| rmatch.as_str()).unwrap_or("")),
                        String::from(captures.get(2).map(|rmatch| rmatch.as_str()).unwrap_or("")),
                    ))
                } else if let Some(captures) = REMOVE_WEBTOON.captures(msg.content.as_str()) {
                    Self::GuildCommand(GuildCommand::RemoveWebtoon(String::from(
                        captures.get(1).map(|rmatch| rmatch.as_str()).unwrap_or(""),
                    )))
                } else if let Some(captures) = MOST_USED_EMOJI.captures(msg.content.as_str()) {
                    Self::MostUsedEmoji(
                        captures
                            .get(1)
                            .and_then(|rmatch| rmatch.as_str().parse::<usize>().ok())
                            .unwrap_or(MAX_EMOJI + 1),
                    )
                } else if let Some(captures) = LEAST_USED_EMOJI.captures(msg.content.as_str()) {
                    Self::LeastUsedEmoji(
                        captures
                            .get(1)
                            .and_then(|rmatch| rmatch.as_str().parse::<usize>().ok())
                            .unwrap_or(MAX_EMOJI + 1),
                    )
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
    fn reaction_add(&self, ctx: Context, reaction: Reaction) -> MaybeError {
        if let Some(guild_id) = reaction.guild_id {
            if let ReactionType::Custom { id, .. } = reaction.emoji {
                let mut data = ctx.data.write();
                let emoji_data = data
                    .get_mut::<EmojiDataKey>()
                    .unwrap()
                    .entry(guild_id)
                    .or_insert_with(|| EmojiData::from_file(guild_id));
                emoji_data.track_emoji(id.to_string());
                emoji_data.save()?;
            }
        }
        Ok(())
    }

    fn guild_command(
        &self,
        ctx: &Context,
        msg: &Message,
        command: GuildCommand,
        guild_id: GuildId,
        guild_data: &mut GuildData,
    ) -> MaybeError {
        match command {
            GuildCommand::GuildCount => {
                guild_data.count += 1;
                msg.channel_id
                    .say(&ctx.http, format!("{}ish", guild_data.count))?;
                guild_data.update_count();
                guild_data.save()?;
            }

            GuildCommand::Whois(user) => {
                lazy_static! {
                    static ref DIGITS: Regex = Regex::new(r"\d+").unwrap();
                }
                let whois_resolution = match DIGITS.find(user.as_str()) {
                    Some(rmatch) => {
                        let id = String::from(rmatch.as_str());
                        match guild_data.get_whois_entry_by_id(&id) {
                            Some(entry) => WhoisResolution::EntryFound(id, entry),
                            None => WhoisResolution::UserFound(id),
                        }
                    }
                    None => WhoisResolution::NoUser,
                };
                match whois_resolution {
                    WhoisResolution::EntryFound(id, entry) => {
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
                    WhoisResolution::UserFound(id) => {
                        send_with_embed(
                            &ctx,
                            &msg,
                            "idk who they are lmao",
                            &format!("<@{}>", id),
                        )?;
                    }
                    WhoisResolution::NoUser => {
                        msg.channel_id
                            .say(&ctx.http, "sry i need the id im working on it tho")?;
                        // msg.channel_id
                        //     .say(&ctx.http, "idk who ur talkin bout lmao")?;
                    }
                }
            }

            GuildCommand::WhoisFetch(url) => {
                if !guild_id
                    .member(&ctx.http, msg.author.id)?
                    .permissions(&ctx.cache)?
                    .manage_guild()
                {
                    msg.channel_id.say(
                        &ctx.http,
                        "u cant even manage the server and you want ME to fetch it for u? lmao",
                    )?;
                    return Ok(());
                }
                let url = match Url::parse(&url).ok().or(guild_data
                    .whois_url
                    .as_ref()
                    .and_then(|url| Url::parse(url).ok()))
                {
                    Some(url) => url,
                    None => {
                        msg.channel_id
                            .say(&ctx.http, "uhhh where lol can u give me a url thx")?;
                        return Ok(());
                    }
                };
                let response = match get(url.as_str()) {
                    Ok(response) => response,
                    Err(error) => {
                        msg.channel_id.say(&ctx.http, "i tripped and fel")?;
                        return Err(error.into());
                    }
                };
                let mut rdr = csv::Reader::from_reader(response);
                guild_data.whois_url = Some(String::from(url.as_str()));
                guild_data.whois_headers =
                    rdr.headers()?.iter().map(|str| String::from(str)).collect();
                guild_data.whois_data = rdr
                    .records()
                    .filter_map(|result| result.ok())
                    .map(|record| record.iter().map(|str| String::from(str)).collect())
                    .collect();
                guild_data.update_whois();
                guild_data.save()?;
                msg.channel_id.say(&ctx.http, "k")?;
            }

            GuildCommand::GetWebtoon(webtoon_id) => {
                let url_str = match guild_data.webtoons.get(&webtoon_id) {
                    Some(url_str) => url_str,
                    None => return Ok(()),
                };
                let url = match Url::parse(&url_str) {
                    Ok(url) => url,
                    Err(_) => {
                        send_with_embed(
                            &ctx,
                            &msg,
                            "uhhh i dont think u gave me a url lol",
                            url_str,
                        )?;
                        return Ok(());
                    }
                };
                let response = match get(url.as_str()) {
                    Ok(response) => response,
                    Err(error) => {
                        msg.channel_id
                            .say(&ctx.http, "hmm the url doesnt seem to work")?;
                        return Err(error.into());
                    }
                };
                let html = response.text()?;
                lazy_static! {
                    static ref WEBTOON_NAME: Regex =
                        Regex::new(r#"property="og:title" content="([^"]+)""#).unwrap();
                    static ref EP_NAME: Regex =
                        Regex::new(r#"<span class="subj"><span>([^<]+)</span>"#).unwrap();
                    static ref IS_UP: Regex = Regex::new(r#"<em class="tx_up">UP</em>"#).unwrap();
                }
                let webtoon_name = WEBTOON_NAME
                    .captures(html.as_ref())
                    .and_then(|captures| captures.get(1))
                    .map(|rmatch| rmatch.as_str())
                    .unwrap_or("[couldn't get Webtoon name]");
                let ep_name = EP_NAME
                    .captures(html.as_ref())
                    .and_then(|captures| captures.get(1))
                    .map(|rmatch| rmatch.as_str())
                    .unwrap_or("[couldn't get episode name]");
                let is_up = IS_UP.is_match(html.as_ref());
                msg.channel_id.send_message(&ctx.http, |message| {
                    message.embed(|embed| {
                        embed.title(webtoon_name);
                        embed.description(format!(
                            "[{}]({}){}",
                            ep_name,
                            "url here",
                            if is_up { " **UP**" } else { "" },
                        ));
                        embed
                    });
                    message.content("i already read this lmao ur slow");
                    message
                })?;
            }

            GuildCommand::AddWebtoon(webtoon_id, url_str) => {
                let success_msg = format!("now you can do `:{}`", webtoon_id);
                match guild_data.webtoons.entry(webtoon_id) {
                    Entry::Occupied(occupied_entry) => {
                        send_with_embed(
                            &ctx,
                            &msg,
                            "oh someone already gave a url with that",
                            occupied_entry.get(),
                        )?;
                    }
                    Entry::Vacant(vacant_entry) => {
                        vacant_entry.insert(url_str);
                        guild_data.update_webtoons();
                        guild_data.save()?;
                        send_with_embed(&ctx, &msg, "yes PLEASE", &success_msg)?;
                    }
                }
            }

            GuildCommand::RemoveWebtoon(webtoon_id) => {
                if let Some(url) = guild_data.webtoons.remove(&webtoon_id) {
                    send_with_embed(&ctx, &msg, "ok will not read", &url)?;
                } else {
                    msg.channel_id
                        .say(&ctx.http, "wasnt reading it anyways lmao")?;
                }
                guild_data.update_webtoons();
                guild_data.save()?;
            }
        }
        Ok(())
    }

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
                    emoji_data.track_emoji(String::from(rmatch.as_str()));
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
                    "-5 karma. im emo now so pls use `bruh who is <user id or mention>`",
                )?;
            }

            Command::Help => {
                msg.channel_id.say(&ctx.http, include_str!("./help.md"))?;
            }

            Command::MostUsedEmoji(limit) => {
                if limit > MAX_EMOJI {
                    msg.channel_id.say(&ctx.http, "thats too many emoji")?;
                } else if let Some(guild_id) = msg.guild_id {
                    let mut data = ctx.data.write();
                    let emoji_data = data
                        .get_mut::<EmojiDataKey>()
                        .unwrap()
                        .entry(guild_id)
                        .or_insert_with(|| EmojiData::from_file(guild_id));
                    let mut emoji_list = emoji_data.emoji.iter().collect::<Vec<_>>();
                    if emoji_list.is_empty() {
                        msg.channel_id.say(&ctx.http, "no emoji oop")?;
                    } else {
                        emoji_list.sort_unstable_by(|(_, a), (_, b)| b.cmp(a));
                        msg.channel_id.say(
                            &ctx.http,
                            emoji_list
                                .iter()
                                .take(limit)
                                .map(|(emoji_id, count)| format!("<:z:{}> {}", emoji_id, count))
                                .collect::<Vec<_>>()
                                .join("\n"),
                        )?;
                    }
                } else {
                    msg.channel_id.say(
                        &ctx.http,
                        "dont care about the custom emoji u send me fuck off",
                    )?;
                }
            }

            Command::LeastUsedEmoji(limit) => {
                if limit > MAX_EMOJI {
                    msg.channel_id.say(&ctx.http, "thats too many emoji")?;
                } else if let Some(guild_id) = msg.guild_id {
                    let mut data = ctx.data.write();
                    let emoji_data = data
                        .get_mut::<EmojiDataKey>()
                        .unwrap()
                        .entry(guild_id)
                        .or_insert_with(|| EmojiData::from_file(guild_id));
                    let mut emoji_list = emoji_data.emoji.iter().collect::<Vec<_>>();
                    if emoji_list.is_empty() {
                        msg.channel_id.say(&ctx.http, "no emoji oop")?;
                    } else {
                        emoji_list.sort_unstable_by(|(_, a), (_, b)| a.cmp(b));
                        msg.channel_id.say(
                            &ctx.http,
                            emoji_list
                                .iter()
                                .take(limit)
                                .map(|(emoji_id, count)| format!("<:z:{}> {}", emoji_id, count))
                                .collect::<Vec<_>>()
                                .join("\n"),
                        )?;
                    }
                } else {
                    msg.channel_id
                        .say(&ctx.http, "dont care about the custom emoji u send me")?;
                }
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
                    self.guild_command(&ctx, &msg, command, guild_id, guild_data)?;
                }
                None => {
                    msg.channel_id.say(&ctx.http, "server only hehehe")?;
                }
            },
        }
        return Ok(());
    }

    fn ready(&self, ctx: Context, ready: Ready) -> MaybeError {
        println!("{} is connected!", ready.user.name);
        ctx.set_activity(Activity::listening("ask me \"moofy what pisses you off\""));
        return Ok(());
    }
}

impl EventHandler for Handler {
    fn reaction_add(&self, ctx: Context, add_reaction: Reaction) {
        if let Err(why) = self.reaction_add(ctx, add_reaction) {
            println!("Error from reaction_add handler: {:?}", why);
        }
    }

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
