use crate::db;
use lazy_static::lazy_static;
use mongodb::{bson::doc, options::UpdateOptions};
use regex::{Regex, RegexBuilder};
use serenity::{
    client::Context,
    framework::standard::{macros::hook, CommandResult, DispatchError},
    model::channel::Message,
    utils::Colour,
};

#[hook]
pub async fn before(ctx: &Context, _: &Message, command_name: &str) -> bool {
    // println!("Got command '{}' by user '{}'", command_name, msg.author.name);

    // Increment the number of times this command has been run once. If
    // the command's name does not exist in the counter, add a default
    // value of 0.
    let mut data = ctx.data.write().await;
    let counter = data
        .get_mut::<crate::commands::general::CommandCounter>()
        .expect("Expected CommandCounter in TypeMap.");
    let entry = counter.entry(command_name.to_string()).or_insert(0);
    *entry += 1;

    true // if `before` returns false, command processing doesn't happen.
}

#[hook]
pub async fn after(
    ctx: &Context,
    msg: &Message,
    command_name: &str,
    command_result: CommandResult,
) {
    if let Err(why) = command_result {
        let _ = msg
            .channel_id
            .send_message(&ctx.http, |message| {
                message.embed(|embed| {
                    embed.description(format!("```rs\n{:?}\n```", why));
                    embed.colour(Colour::RED);
                    embed
                });
                message.content(format!("Command '{}' returned error", command_name));
                message
            })
            .await;
    }
    if let Err(why) = check_mentions(ctx, msg).await {
        println!("Checking mentions had an error: {:?}", why);
    }
}

// #[hook]
// pub async fn unknown_command(_ctx: &Context, _msg: &Message, unknown_command_name: &str) {
//     println!("Could not find command named '{}'", unknown_command_name);
// }

#[hook]
pub async fn normal_message(ctx: &Context, msg: &Message) {
    lazy_static! {
        static ref MENTIONED_MOOFY: Regex = RegexBuilder::new(r"\bmoofy\b")
            .case_insensitive(true)
            .build()
            .unwrap();
    }
    if MENTIONED_MOOFY.is_match(&msg.content) {
        let _ = msg.react(&ctx.http, '👀').await;
    }
    if let Ok(true) = msg.mentions_me(&ctx.http).await {
        let _ = msg
            .channel_id
            .say(&ctx.http, "<:ping:719277539113041930>")
            .await;
    }
    if let Err(why) = check_mentions(ctx, msg).await {
        println!("Checking mentions had an error: {:?}", why);
    }
}

async fn check_mentions(ctx: &Context, msg: &Message) -> CommandResult {
    let guild_id = match msg.guild_id {
        Some(id) => id.as_u64().to_owned(),
        None => return Ok(()),
    };

    let mentioned_everyone = msg.mention_everyone;
    let mentioned_roles = &msg.mention_roles;
    let mentioned_users = &msg.mentions;

    if !mentioned_everyone && mentioned_roles.len() == 0 && mentioned_users.len() == 0 {
        return Ok(());
    }

    let data = ctx.data.read().await;
    let db = data.get::<db::Db>().expect("Expected Db in TypeMap.");
    let past_pings = db.collection("past-pings");

    if mentioned_everyone {
        past_pings
            .update_one(
                doc! {
                    "guild": guild_id,
                    "everyone": true,
                },
                doc! {
                    "$set": {
                        "content": &msg.content,
                        "author": msg.author.id.as_u64(),
                        "channel_id": msg.channel_id.as_u64(),
                        "message_id": msg.id.as_u64(),
                    },
                    "$setOnInsert": {
                        "guild": &guild_id,
                        "everyone": true,
                    },
                },
                UpdateOptions::builder().upsert(true).build(),
            )
            .await?;
    }
    if mentioned_roles.len() > 0 {
        for role_id in mentioned_roles {
            past_pings
                .update_one(
                    doc! {
                        "guild": &guild_id,
                        "role": role_id.as_u64(),
                    },
                    doc! {
                        "$set": {
                            "content": &msg.content,
                            "author": msg.author.id.as_u64(),
                            "channel_id": msg.channel_id.as_u64(),
                            "message_id": msg.id.as_u64(),
                        },
                        "$setOnInsert": {
                            "guild": &guild_id,
                            "role": role_id.as_u64(),
                        },
                    },
                    UpdateOptions::builder().upsert(true).build(),
                )
                .await?;
        }
    }
    if mentioned_users.len() > 0 {
        for user in mentioned_users {
            let user_id = user.id.as_u64();
            past_pings
                .update_one(
                    doc! {
                        "guild": &guild_id,
                        "user": &user_id,
                    },
                    doc! {
                        "$set": {
                            "content": &msg.content,
                            "author": msg.author.id.as_u64(),
                            "channel_id": msg.channel_id.as_u64(),
                            "message_id": msg.id.as_u64(),
                        },
                        "$setOnInsert": {
                            "guild": &guild_id,
                            "user": &user_id,
                        },
                    },
                    UpdateOptions::builder().upsert(true).build(),
                )
                .await?;
        }
    }

    Ok(())
}

#[hook]
pub async fn dispatch_error(ctx: &Context, msg: &Message, error: DispatchError) {
    if let DispatchError::Ratelimited(duration) = error {
        let _ = msg
            .channel_id
            .say(
                &ctx.http,
                &format!("Try this again in {} seconds.", duration.as_secs()),
            )
            .await;
    }
}
