use crate::{db, error_with_reason::ErrorWithReason};
use mongodb::{
    bson::doc,
    options::{FindOneAndUpdateOptions, ReturnDocument},
    Collection,
};
use serenity::{
    client::{bridge::gateway::ChunkGuildFilter, Context},
    framework::standard::{
        macros::{command, group},
        Args, CommandResult,
    },
    model::channel::Message,
};

#[group]
#[commands(count, get_user_id)]
#[description = "Random testing commands for Moofy."]
struct Test;

#[command]
#[usage = "<number>"]
#[example = "5"]
#[example = "-50"]
/// Adds the given number to the server's count.
async fn count(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let data = ctx.data.read().await;
    let db = data.get::<db::Db>().expect("Expected Db in TypeMap.");

    let collection: Collection = db.collection("test-count");

    let offset = args
        .single::<i32>()
        .map_err(|_| ErrorWithReason("The given number doesn't seem to be an i32."))?;

    let guild_id = msg
        .guild_id
        .map_or_else(|| "dm".to_string(), |id| id.to_string());
    // https://stackoverflow.com/a/24747475
    let maybe_doc = collection
        .find_one_and_update(
            doc! {
                "guild": &guild_id,
            },
            doc! {
                "$inc": {
                    // can only be [fiu](32|64)
                    "count": offset,
                },
                "$setOnInsert": {
                    "guild": &guild_id,
                },
            },
            FindOneAndUpdateOptions::builder()
                .upsert(true)
                .return_document(Some(ReturnDocument::After))
                .build(),
        )
        .await?;
    if let Some(doc) = maybe_doc {
        if let Ok(new_count) = doc
            .get_i64("count")
            .map(|n| n.to_string())
            .or_else(|_| doc.get_i32("count").map(|n| n.to_string()))
        {
            msg.channel_id
                .say(&ctx.http, format!("The new count is {}.", new_count))
                .await?;
        } else {
            msg.channel_id
                .say(&ctx.http, format!("You counted too much! Use `:count <negative number>` to clean up your mess.```rs\n{:?}\n```", doc))
                .await?;
        }
    } else {
        Err(ErrorWithReason("No document was returned...?"))?;
    }

    Ok(())
}

#[command]
/// Gets the user ID by name
async fn get_user_id(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let potential_user_name = args.rest();

    if let Some(guild) = msg.guild(&ctx.cache).await {
        ctx.shard
            .chunk_guild(guild.id, None, ChunkGuildFilter::None, None);
        println!("{} members", guild.member_count);
        if let Some(member) = guild.member_named(potential_user_name) {
            msg.channel_id
                .say(&ctx.http, format!("ok ```rs\n{:?}```", member))
                .await?;
        } else {
            msg.channel_id
                .say(
                    &ctx.http,
                    guild
                        .members_containing(potential_user_name, false, true)
                        .await
                        .iter()
                        .map(|(member, str)| format!("(```rs\n{:?}```, {})", member, str))
                        .collect::<Vec<String>>()
                        .join("\n"),
                )
                .await?;
        }
    } else {
        msg.channel_id
            .say(&ctx.http, "You aren't in a server.")
            .await?;
    }

    Ok(())
}
