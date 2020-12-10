use serenity::{
    client::Context,
    framework::standard::{
        macros::{command, group},
        Args, CommandResult,
    },
    model::channel::Message,
};

#[group]
// Sets a single prefix for this group.
// So one has to call commands in this group
// via `:math` instead of just `:`.
#[prefix = "math"]
#[commands(multiply)]
/// A very limited set of calculator commands. See `:help math multiply` for more info.
struct Math;

#[command]
// Lets us also call `:math *` instead of just `:math multiply`.
#[aliases("*")]
#[usage = "<a> <b>"]
#[example = "3 4"]
/// Multiplies two floats.
async fn multiply(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let first = args.single::<f64>()?;
    let second = args.single::<f64>()?;

    let res = first * second;

    msg.channel_id.say(&ctx.http, &res.to_string()).await?;

    Ok(())
}
