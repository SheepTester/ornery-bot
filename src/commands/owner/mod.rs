use serenity::{
    client::Context,
    framework::standard::{
        macros::{command, group},
        Args, CommandResult,
    },
    model::channel::{Channel, Message},
};

#[group]
#[owners_only]
// Limit all commands to be guild-restricted.
#[only_in(guilds)]
#[commands(slow_mode)]
/// Only the creator of this bot can use these commands!
struct Owner;

#[command]
#[usage = "<time>"]
#[example = ""]
#[example = "30"]
/// Sets the slow mode to the number of seconds if given, or simply determines the slow mode time
/// for the channel.
async fn slow_mode(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let say_content = if let Ok(slow_mode_rate_seconds) = args.single::<u64>() {
        if let Err(why) = msg
            .channel_id
            .edit(&ctx.http, |c| c.slow_mode_rate(slow_mode_rate_seconds))
            .await
        {
            println!("Error setting channel's slow mode rate: {:?}", why);

            format!(
                "Failed to set slow mode to `{}` seconds.",
                slow_mode_rate_seconds
            )
        } else {
            format!(
                "Successfully set slow mode rate to `{}` seconds.",
                slow_mode_rate_seconds
            )
        }
    } else if let Some(Channel::Guild(channel)) = msg.channel_id.to_channel_cached(&ctx.cache).await
    {
        format!(
            "Current slow mode rate is `{}` seconds.",
            channel.slow_mode_rate.unwrap_or(0)
        )
    } else {
        "Failed to find channel in cache.".to_string()
    };

    msg.channel_id.say(&ctx.http, say_content).await?;

    Ok(())
}
