use crate::{db, error_with_reason::ErrorWithReason};
use lazy_static::lazy_static;
use mongodb::{
    bson::{doc, Bson, Document},
    options::UpdateOptions,
    Collection,
};
use regex::Regex;
use reqwest::{get, Url};
use serenity::{
    client::{bridge::gateway::ChunkGuildFilter, Context},
    framework::standard::{
        macros::{command, group},
        ArgError, Args, CommandResult,
    },
    model::channel::Message,
};

#[group]
#[prefixes("whois", "who")]
#[default_command(identify)]
#[commands(fetch, identify)]
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
                "_id": id,
            },
            None,
        )
        .await?
    {
        Some(doc) => {
            msg.channel_id
                .send_message(&ctx.http, |message| {
                    message.embed(|embed| {
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

#[command]
#[usage = "\"[url]\" [id field]"]
#[example = "\"https://example.com/users.csv\" \"User ID\""]
#[example = ""]
#[required_permissions("MANAGE_GUILD")]
/// Fetch whois informaton from the given URL to a CSV file. The ID field will be used to identify
/// which Discord user corresponds to which entry. Both arguments are optional and will use the
/// last given URL/ID field. Requires that you can manage the guild (the MANAGE_GUILD permission).
async fn fetch(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
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
        doc.insert("_id", id_value);
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
