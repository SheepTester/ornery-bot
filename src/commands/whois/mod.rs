use crate::{db, error_with_reason::ErrorWithReason};
use lazy_static::lazy_static;
use mongodb::{
    bson::{doc, Bson, Document},
    options::UpdateOptions,
    Collection,
};
use regex::{Captures, Regex};
use reqwest::{get, Url};
use serenity::{
    client::{bridge::gateway::ChunkGuildFilter, Context},
    framework::standard::{
        macros::{command, group},
        ArgError, Args, CommandResult,
    },
    model::channel::Message,
    utils::Colour,
};

#[group]
#[prefixes("whois", "who")]
#[only_in(guilds)]
#[default_command(identify)]
#[commands(fetch, identify, here, config)]
#[description = "Give information about a user from a CSV file."]
struct Whois;

async fn display_whois_entry(
    ctx: &Context,
    msg: &Message,
    whois_data: &Collection,
    guild_id: &u64,
    id: &String,
    other_users: Option<&String>,
) -> CommandResult<bool> {
    match whois_data
        .find_one(
            doc! {
                "_guild": guild_id,
                "_user": id,
            },
            None,
        )
        .await?
    {
        Some(doc) => {
            msg.channel_id
                .send_message(&ctx.http, |message| {
                    message.embed(|embed| {
                        embed.colour(Colour::MAGENTA);
                        embed.description(match other_users {
                            Some(others) => if others.is_empty() {
                                format!("What I know about <@{}> (whom I'm guessing you're referring to)", id)
                            } else {
                                format!("Other users you may have meant:\n{}\nBut here's what we know about <@{}>", others, id)
                            },
                            None => format!("What I know about <@{}>", id)
                        });
                        for (key, value) in doc.iter() {
                            if !key.starts_with("_") {
                                if let Bson::String(str) = value {
                                    if !str.is_empty() {
                                        embed.field(key, str, true);
                                    }
                                }
                            }
                        }
                        embed
                    });
                    message.content("Fresh from the FBI's kitchen!");
                    message
                })
                .await?;
            Ok(true)
        }
        None => Ok(false),
    }
}

#[command]
#[aliases("is")]
#[usage = "<user id or name>"]
#[example = "393248490739859458"]
#[example = "moofy-bot"]
/// List information about the given user from a CSV file.
async fn identify(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let guild = match msg.guild(&ctx.cache).await {
        Some(guild) => guild,
        None => {
            msg.channel_id
                .say(&ctx.http, "You aren't in a server.")
                .await?;
            return Ok(());
        }
    };
    let guild_id = guild.id.as_u64();

    let data = ctx.data.read().await;
    let db = data.get::<db::Db>().expect("Expected Db in TypeMap.");

    let whois_data = db.collection("whois-data");

    let username_search = args.rest();

    let mut tried_id = false;
    let mut tried_member_match = false;
    let mut tried_first_search = false;

    lazy_static! {
        static ref USER_ID: Regex = Regex::new(r"\d+").unwrap();
    }

    if let Some(matched_id) = USER_ID.find(username_search) {
        let id = matched_id.as_str();
        if display_whois_entry(ctx, msg, &whois_data, guild_id, &id.to_string(), None).await? {
            return Ok(());
        }
        tried_id = true;
    }

    ctx.shard
        .chunk_guild(guild.id, None, ChunkGuildFilter::None, None);
    if let Some(member) = guild.member_named(username_search) {
        if display_whois_entry(
            ctx,
            msg,
            &whois_data,
            guild_id,
            &member.user.id.to_string(),
            None,
        )
        .await?
        {
            return Ok(());
        }
        tried_member_match = true;
    }

    let mut possibilities = guild.members_containing(username_search, false, true).await;
    let first_guess = possibilities.pop();
    let mut display_matches = String::new();
    for (member, _) in &possibilities {
        let line = format!("<@{}> ({})\n", member.user.id, member.user.id);
        // 1900 to allow for other text
        if display_matches.len() + line.len() > 1900 {
            break;
        }
        display_matches.push_str(line.as_str());
    }
    if let Some((search_result, _)) = first_guess {
        if display_whois_entry(
            ctx,
            msg,
            &whois_data,
            guild_id,
            &search_result.user.id.to_string(),
            Some(&display_matches),
        )
        .await?
        {
            return Ok(());
        }
        tried_first_search = true;
    }
    msg.channel_id
        .send_message(&ctx.http, move |message| {
            let tried = if tried_id || tried_member_match || tried_first_search {
                format!(
                    "\nI tried:{}{}{}\nbut I couldn't find anyone on this server who has a whois entry.\n\n",
                    if tried_id {
                        "\n- looking up the user ID/mention."
                    } else {
                        ""
                    },
                    if tried_member_match {
                        "\n- matching the member by their exact name."
                    } else {
                        ""
                    },
                    if tried_first_search {
                        "\n- trying the first result that somewhat resembled what you wrote."
                    } else {
                        ""
                    },
                )
            } else {
                String::from(" ")
            };
            if let Some((search_result, _)) = first_guess {
                message.content(format!(
                    "I don't know the person you're referring to.{}(Hint: Have the mods done `:whois fetch`?) Perhaps I may know these other users? If so, do `:whois <user id>`.",
                    tried
                ));
                message.embed(|embed| {
                    embed.colour(Colour::MAGENTA);
                    embed.description(format!(
                        "**<@{}> ({})\n**\n{}",
                        search_result.user.id, search_result.user.id, display_matches
                    ));
                    embed
                });
            } else {
                message.content(format!(
                    "I don't know the person you're referring to.{}(Hint: It might be possible the mods have not done `:whois fetch`?)",
                    tried
                ));
            }
            message
        })
        .await?;

    Ok(())
}

async fn get_display_form(
    whois_data: &Collection,
    display_field: &str,
    guild_id: &u64,
    id: &String,
) -> CommandResult<String> {
    match whois_data
        .find_one(
            doc! {
                "_guild": guild_id,
                "_user": id,
            },
            None,
        )
        .await?
    {
        Some(doc) => {
            lazy_static! {
                static ref DISPLAY_FIELD: Regex = Regex::new(r"\{\{(.+?)\}\}").unwrap();
            }
            let display = DISPLAY_FIELD.replace_all(display_field, |captures: &Captures| {
                let field_name = captures.get(1).map_or("", |m| m.as_str());
                doc.get_str(field_name).unwrap_or("")
            });
            Ok(String::from(display))
        }
        None => Ok(format!("[<@{}> not known]", id)),
    }
}

#[command]
#[usage = "[number of messages]"]
#[example = ""]
#[example = "10"]
/// Get information about the last few people who sent messages in chat. By default, the authors of
/// the last 5 messages are checked. The number of messages is limited by the maximum message
/// length and the maximum number of messages I can fetch.
async fn here(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let guild_id = match msg.guild_id {
        Some(id) => id.as_u64().to_owned(),
        None => {
            msg.channel_id
                .say(&ctx.http, "You aren't in a server.")
                .await?;
            return Ok(());
        }
    };

    let limit = args.single::<u64>().unwrap_or(5);
    let messages = msg
        .channel_id
        .messages(&ctx.http, |retriever| retriever.before(msg.id).limit(limit))
        .await?;

    let data = ctx.data.read().await;
    let db = data.get::<db::Db>().expect("Expected Db in TypeMap.");
    let whois_data = db.collection("whois-data");
    let whois_settings = db.collection("whois-settings");

    let settings = whois_settings
        .find_one(doc! { "_guild": guild_id }, None)
        .await?
        .unwrap_or_else(|| Document::new());
    let display_field = settings.get_str("display").unwrap_or("<@{{_user}}>");

    let mut names = Vec::new();
    let mut total_length: usize = 0;
    for msg in &messages {
        let display = get_display_form(
            &whois_data,
            display_field,
            &guild_id,
            &msg.author.id.to_string(),
        )
        .await?;
        // Include an extra character for the newline
        if total_length + display.len() + 1 < 2000 - 5 {
            total_length += display.len() + 1;
            names.push(display);
        } else {
            names.push(String::from("[...]"));
            break;
        }
    }
    names.reverse();

    msg.channel_id
        .send_message(&ctx.http, |message| {
            message.embed(|embed| {
                embed.colour(Colour::MAGENTA);
                embed.description(names.join("\n"));
                embed
            });
            message.content("Here, allow me to introduce you:");
            message
        })
        .await?;

    Ok(())
}

#[command]
#[usage = "\"[url]\" [id field]"]
#[example = "\"https://example.com/users.csv\" \"User ID\""]
#[example = ""]
#[required_permissions("MANAGE_GUILD")]
/// Fetch whois informaton from the given URL to a CSV file. The ID field will be used to identify
/// which Discord user corresponds to which entry. Both arguments are optional and will use the
/// last given URL/ID field. Requires that you can manage the guild (the MANAGE_GUILD permission).
async fn fetch(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let guild_id = match msg.guild_id {
        Some(id) => id.as_u64().to_owned(),
        None => {
            msg.channel_id
                .say(&ctx.http, "You aren't in a server.")
                .await?;
            return Ok(());
        }
    };

    let data = ctx.data.read().await;
    let db = data.get::<db::Db>().expect("Expected Db in TypeMap.");

    let whois_data = db.collection("whois-data");
    let whois_settings = db.collection("whois-settings");

    let settings = whois_settings
        .find_one(doc! { "_guild": guild_id }, None)
        .await?
        .unwrap_or_else(|| Document::new());

    let url_str = match args.single_quoted::<String>() {
        Ok(url) => url,
        Err(ArgError::Eos) => {
            if let Ok(url) = settings.get_str("url") {
                String::from(url)
            } else {
                msg.channel_id
                    .say(
                        &ctx.http,
                        "I don't remember the last URL you used, so you'll have to specify it.",
                    )
                    .await?;
                return Ok(());
            }
        }
        Err(err) => Err(err)?,
    };

    let id_field = match args.single_quoted::<String>() {
        Ok(id_field) => id_field,
        Err(ArgError::Eos) => {
            if let Ok(id_field) = settings.get_str("id") {
                String::from(id_field)
            } else {
                String::from("ID")
            }
        }
        Err(err) => Err(err)?,
    };

    let csv = get(Url::parse(&url_str)?.as_str()).await?.text().await?;
    let mut reader = csv::Reader::from_reader(csv.as_bytes());
    let headers = reader.headers()?.clone();
    let mut data = Vec::new();

    for (i, record) in reader.records().enumerate() {
        let mut doc = doc! { "_guild": guild_id };
        for (key, value) in headers.iter().zip(record?.iter()) {
            doc.insert(key, Bson::String(String::from(value)));
        }
        let id_value = doc
            .get(id_field.as_str())
            .ok_or_else(|| {
                ErrorWithReason(format!(
                    "Row {} does not have a value for {}",
                    i + 1,
                    id_field
                ))
            })?
            .clone();
        doc.insert("_user", id_value);
        data.push(doc);
    }

    whois_data
        .delete_many(
            doc! {
                "_guild": guild_id,
            },
            None,
        )
        .await?;
    whois_data.insert_many(data, None).await?;

    whois_settings
        .update_one(
            doc! {
                "_guild": guild_id,
            },
            doc! {
                "$set": {
                    "url": url_str,
                    "id": id_field,
                },
                "$setOnInsert": {
                    "_guild": &guild_id,
                },
            },
            UpdateOptions::builder().upsert(true).build(),
        )
        .await?;

    msg.react(&ctx.http, '👌').await?;

    Ok(())
}

const VALID_OPTION_NAMES: [&str; 3] = ["id", "url", "display"];

#[command]
#[usage = r#"<option name> "[option value]""#]
#[example = r#"display "{{First Name}} {{Last Name}}""#]
#[example = "display"]
#[required_permissions("MANAGE_GUILD")]
/// Set server-wide configuration options for whois output. If the option value isn't given, then
/// the option will be returned instead of set. Here's a list of option names:
///
/// - `id` The field name that contains the Discord ID.
/// - `url` The last used fetch URL for `:whois fetch`.
/// - `display` Define the format for a summary of the whois data for a person. Use `{{field
/// name}}` to denote field names.
async fn config(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let guild_id = match msg.guild_id {
        Some(id) => id.as_u64().to_owned(),
        None => {
            msg.channel_id
                .say(&ctx.http, "You aren't in a server.")
                .await?;
            return Ok(());
        }
    };

    let option_name = args.single::<String>()?;
    let option_value = args.single_quoted::<String>();

    if !VALID_OPTION_NAMES.contains(&option_name.as_str()) {
        msg.channel_id
            .say(&ctx.http, "That's not a valid option name. Do `:help whois config` for a list of valid option names.")
            .await?;
        return Ok(());
    }

    let data = ctx.data.read().await;
    let db = data.get::<db::Db>().expect("Expected Db in TypeMap.");
    let whois_settings = db.collection("whois-settings");

    if let Ok(value) = option_value {
        whois_settings
            .update_one(
                doc! {
                    "_guild": guild_id,
                },
                doc! {
                    "$set": {
                        option_name: value,
                    },
                    "$setOnInsert": {
                        "_guild": &guild_id,
                    },
                },
                UpdateOptions::builder().upsert(true).build(),
            )
            .await?;
        msg.react(&ctx.http, '👌').await?;
    } else {
        let settings = whois_settings
            .find_one(doc! { "_guild": guild_id }, None)
            .await?
            .unwrap_or_else(|| Document::new());
        if let Ok(value) = settings.get_str(option_name.as_str()) {
            msg.channel_id
                .send_message(&ctx.http, |message| {
                    message.embed(|embed| {
                        embed.colour(Colour::MAGENTA);
                        embed.description(value);
                        embed
                    });
                    message
                })
                .await?;
        } else {
            msg.channel_id
                .say(&ctx.http, "This option name has not been set before.")
                .await?;
        }
    }

    Ok(())
}
