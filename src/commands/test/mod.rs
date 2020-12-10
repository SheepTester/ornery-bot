use crate::error_with_reason::ErrorWithReason;
use serenity::{
    client::Context,
    framework::standard::{
        macros::{command, group},
        Args, CommandResult,
    },
    model::channel::Message,
};

#[group]
#[commands(count)]
#[description = "Random testing commands for Moofy."]
struct Test;

#[command]
#[usage = "<number>"]
#[example = "5"]
/// Adds the given number to the server's count.
async fn count(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let offset = args.single::<usize>().map_err(|_| {
        ErrorWithReason("The given number doesn't seem to be a nonnegative integer.")
    })?;

    Ok(())
}
