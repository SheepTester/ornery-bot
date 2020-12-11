use serenity::{
    client::Context,
    framework::standard::{
        macros::{command, group},
        Args, CommandResult,
    },
    model::channel::Message,
};

#[group]
// Sets multiple prefixes for a group.
// This requires us to call commands in this group
// via `:emoji` (or `:emo`) instead of just `:`.
#[prefixes("emoji", "emo")]
// Set a description to appear if a user wants to display a single group
// e.g. via help using the group-name or one of its prefixes.
// Sets a command that will be executed if only a group-prefix was passed.
#[default_command(bird)]
#[commands(cat, sheep)]
#[description = "A group with commands providing from a limited set of emoji in response."]
struct Emoji;

#[command]
// Adds multiple aliases
#[aliases("kitty", "neko")]
// Make this command use the "emoji" bucket.
#[bucket = "emoji"]
// Allow only administrators to call this:
#[required_permissions("ADMINISTRATOR")]
/// Sends a cat emoji.
async fn cat(ctx: &Context, msg: &Message) -> CommandResult {
    msg.channel_id.say(&ctx.http, ":cat:").await?;

    Ok(())
}

#[command]
#[bucket = "emoji"]
/// Sends an emoji with a sheep.
async fn sheep(ctx: &Context, msg: &Message) -> CommandResult {
    msg.channel_id.say(&ctx.http, ":sheep:").await?;

    Ok(())
}

#[command]
/// The emoji-finding bird introduces itself.
async fn bird(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let say_content = if args.is_empty() {
        ":bird: can find animals for you.".to_string()
    } else {
        format!(":bird: could not find animal named: `{}`.", args.rest())
    };

    msg.channel_id.say(&ctx.http, say_content).await?;

    Ok(())
}
