use mongodb::bson::doc;
use serenity::{
    client::{
        bridge::gateway::{ShardId, ShardManager},
        Context,
    },
    framework::standard::{
        macros::{command, group},
        Args, CommandResult,
    },
    model::{channel::Message, permissions::Permissions},
    prelude::{Mutex, TypeMapKey},
    utils::{content_safe, Colour, ContentSafeOptions},
};
use std::{collections::HashMap, fmt::Write, sync::Arc};
use tokio::stream::StreamExt;

use super::checks::OWNER_CHECK;
use crate::db;

// A container type is created for inserting into the Client's `data`, which
// allows for data to be accessible across all events and framework commands, or
// anywhere else that has a copy of the `data` Arc.
pub struct ShardManagerContainer;

impl TypeMapKey for ShardManagerContainer {
    type Value = Arc<Mutex<ShardManager>>;
}

pub struct CommandCounter;

impl TypeMapKey for CommandCounter {
    type Value = HashMap<String, u64>;
}

#[group]
#[commands(
    about,
    am_i_admin,
    say,
    commands,
    ping,
    latency,
    some_long_command,
    whopinged
)]
#[description = "All the top-level commands you can use without using a quote-unquote \"prefix.\""]
struct General;

// Commands can be created via the attribute `#[command]` macro.
#[command]
// Options are passed via subsequent attributes.
// Make this command use the "complicated" bucket.
#[bucket = "complicated"]
/// Lists the number of times each command has been used since the bot last woke up.
async fn commands(ctx: &Context, msg: &Message) -> CommandResult {
    let mut contents = "Commands used since last restart:\n".to_string();

    let data = ctx.data.read().await;
    let counter = data
        .get::<CommandCounter>()
        .expect("Expected CommandCounter in TypeMap.");

    for (k, v) in counter {
        writeln!(contents, "- {name}: {amount}", name = k, amount = v)?;
    }

    msg.channel_id.say(&ctx.http, &contents).await?;

    Ok(())
}

// Repeats what the user passed as argument but ensures that user and role
// mentions are replaced with a safe textual alternative.
// In this example channel mentions are excluded via the `ContentSafeOptions`.
#[command]
#[usage = "<message>"]
#[example = "Moofy is passable in quality. He is whom I aspire to be."]
/// I repeat your message. Good luck making me ping!
async fn say(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let settings = if let Some(guild_id) = msg.guild_id {
        // By default roles, users, and channel mentions are cleaned.
        ContentSafeOptions::default()
            // We do not want to clean channal mentions as they
            // do not ping users.
            .clean_channel(false)
            // If it's a guild channel, we want mentioned users to be displayed
            // as their display name.
            .display_as_member_from(guild_id)
    } else {
        ContentSafeOptions::default()
            .clean_channel(false)
            .clean_role(false)
    };

    let content = content_safe(&ctx.cache, &args.rest(), &settings).await;

    msg.channel_id.say(&ctx.http, &content).await?;

    Ok(())
}

#[command]
#[usage = "<...arguments>"]
#[example = "The panda eats, shoots, and leaves."]
/// Prints the command arguments
async fn some_long_command(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    msg.channel_id
        .say(&ctx.http, &format!("Arguments: {:?}", args.rest()))
        .await?;

    Ok(())
}

#[command]
/// Allow me to introduce myself.
async fn about(ctx: &Context, msg: &Message) -> CommandResult {
    msg.channel_id
        .send_message(&ctx.http, |message| {
            message.embed(|embed| {
                embed.title("Links");
                embed.description("Like any good bot, I am proudly open-sourced on [Github]\
                (https://github.com/SheepTester/ornery-bot).\n\nIf you really want me on your \
                server, here's [my invite link](https://discord.com/api/oauth2/\
                authorize?client_id=393248490739859458&scope=bot).\n\nCheck out my bot buddy, \
                [RBot](https://github.com/ky28059/RBot/)!");
                embed.colour(Colour::MAGENTA);
                embed
            });
            message.content("Hi! I'm Moofy (he, him, etc.), running ornery-bot 1.0, made with Serenity 0.9 in Rust.");
            message
        })
        .await?;

    Ok(())
}

#[command]
/// Gets super technical information about these newfangled "shards."
async fn latency(ctx: &Context, msg: &Message) -> CommandResult {
    // The shard manager is an interface for mutating, stopping, restarting, and
    // retrieving information about shards.
    let data = ctx.data.read().await;

    let shard_manager = match data.get::<ShardManagerContainer>() {
        Some(v) => v,
        None => {
            msg.reply(ctx, "There was a problem getting the shard manager")
                .await?;

            return Ok(());
        }
    };

    let manager = shard_manager.lock().await;
    let runners = manager.runners.lock().await;

    // Shards are backed by a "shard runner" responsible for processing events
    // over the shard, so we'll get the information about the shard runner for
    // the shard this command was sent over.
    let runner = match runners.get(&ShardId(ctx.shard_id)) {
        Some(runner) => runner,
        None => {
            msg.reply(ctx, "No shard found").await?;

            return Ok(());
        }
    };

    msg.reply(ctx, &format!("The shard latency is {:?}", runner.latency))
        .await?;

    Ok(())
}

#[command]
// Limit command usage to guilds.
#[only_in(guilds)]
#[checks(Owner)]
/// Responds with pong.
async fn ping(ctx: &Context, msg: &Message) -> CommandResult {
    msg.channel_id.say(&ctx.http, "PONG.").await?;

    Ok(())
}

// We could also use
// #[required_permissions(ADMINISTRATOR)]
// but that would not let us reply when it fails.
#[command]
/// Says if you have administrator permissions or not.
async fn am_i_admin(ctx: &Context, msg: &Message, _args: Args) -> CommandResult {
    if let Some(member) = &msg.member {
        for role in &member.roles {
            if role
                .to_role_cached(&ctx.cache)
                .await
                .map_or(false, |r| r.has_permission(Permissions::ADMINISTRATOR))
            {
                msg.channel_id.say(&ctx.http, "Yes, you are.").await?;

                return Ok(());
            }
        }
    }

    msg.channel_id.say(&ctx.http, "No, you are not.").await?;

    Ok(())
}

#[command]
#[only_in(guilds)]
#[aliases("whoping", "quienmehahechoping")]
#[usage = ""]
#[example = ""]
/// Lists your last pings (assuming Moofy has been paying attention).
async fn whopinged(ctx: &Context, msg: &Message) -> CommandResult {
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
    let past_pings = db.collection("past-pings");

    // TODO: Role pings?
    let mut settings = past_pings
        .find(
            doc! {
                "guild": guild_id,
                "$or": [
                    { "everyone": true },
                    { "user": &msg.author.id.as_u64() },
                ]
            },
            None,
        )
        .await?;

    let mut fields: Vec<(&str, String)> = Vec::new();
    while let Some(doc_result) = settings.next().await {
        let doc = doc_result?;
        let trimmed_content = {
            // Insert zero width space between ] and ( to prevent hiding messages in link URLs
            let content = doc.get_str("content")?.replace("](", "]\u{200b}(");
            if content.len() < 2000 - 70 {
                content
            } else {
                String::from(&content[0..(2000 - 70)])
            }
        };
        let author_id = doc.get_i64("author").unwrap_or(0);
        let channel_id = doc.get_i64("channel_id").unwrap_or(0);
        let message_id = doc.get_i64("message_id").unwrap_or(0);
        if let Some(_) = doc.get("everyone") {
            fields.push((
                "Last @everyone",
                format!(
                    "[<@{}> pinged you](https://discord.com/channels/{}/{}/{})\n\n{}",
                    author_id, guild_id, channel_id, message_id, trimmed_content
                ),
            ));
        } else if let Some(_) = doc.get("user") {
            fields.push((
                "Last direct @mention",
                format!(
                    "[<@{}> pinged you](https://discord.com/channels/{}/{}/{})\n\n{}",
                    author_id, guild_id, channel_id, message_id, trimmed_content
                ),
            ));
        }
    }

    msg.channel_id
        .send_message(&ctx.http, |message| {
            message.embed(|embed| {
                embed.title("Who DARED to ping thee?");
                embed.colour(Colour::MAGENTA);
                for (title, value) in fields {
                    embed.field(title, value, false);
                }
                embed
            });
            message.content(
                "Tip: Discord has an inbox (ctrl/command + i) with a list of your past mentions.",
            );
            message
        })
        .await?;

    Ok(())
}
