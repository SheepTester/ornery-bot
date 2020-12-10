use serenity::{
    client::Context,
    framework::standard::{
        help_commands, macros::help, Args, CommandGroup, CommandResult, HelpOptions,
    },
    model::{channel::Message, id::UserId},
};
use std::collections::HashSet;

// The framework provides two built-in help commands for you to use.
// But you can also make your own customized help command that forwards
// to the behaviour of either of them.
#[help]
// This replaces the information that a user can pass
// a command-name as argument to gain specific information about it.
// Some arguments require a `{}` in order to replace it with contextual information.
// In this case our `{}` refers to a command's name.
#[suggestion_text = "I don't know what that is, but it looks similar to `{}`."]
#[no_help_available_text = "I can't help you with that, unfortunately."]
#[usage_label = "How to use"]
#[usage_sample_label = "Example usage"]
#[ungrouped_label = "Not in a group of commands"]
#[grouped_label = "Which group of commands is this part of?"]
#[aliases_label = "Alternative names for this command (aliases)"]
#[description_label = "Description"]
#[guild_only_text = "You can only use this command in servers."]
#[checks_label = "Checks "]
#[sub_commands_label = "Commands in this group"]
#[dm_only_text = "You can only use this command in DMs."]
#[dm_and_guild_text = "You can use this command anywhere (DMs and servers)."]
#[available_text = "Where can you use this?"]
#[command_not_found_text = "I don't know what command you're referring to."]
#[individual_command_tip = "Do `:help <command name>` to learn more about a command."]
#[group_prefix = "To use this group, type the following word and then one of the words below"]
#[strikethrough_commands_tip_in_dm = "If a command is crossed out, you can't use it in DMs."]
#[strikethrough_commands_tip_in_guild = "If a command is crossed out, you can't use it in servers."]
// Define the maximum Levenshtein-distance between a searched command-name
// and commands. If the distance is lower than or equal the set distance,
// it will be displayed as a suggestion.
// Setting the distance to 0 will disable suggestions.
#[max_levenshtein_distance(3)]
// Serenity will automatically analyse and generate a hint/tip explaining the possible
// cases of ::strikethrough-commands::, but only if
// `strikethrough_commands_tip_{dm, guild}` aren't specified.
// If you pass in a value, it will be displayed instead.
pub async fn my_help(
    context: &Context,
    msg: &Message,
    args: Args,
    help_options: &'static HelpOptions,
    groups: &[&'static CommandGroup],
    owners: HashSet<UserId>,
) -> CommandResult {
    let _ = help_commands::with_embeds(
        context,
        msg,
        args,
        &HelpOptions {
            // You can't use custom colours an attribute yet
            embed_success_colour: crate::consts::THEME,
            ..help_options.clone()
        },
        groups,
        owners,
    )
    .await;
    Ok(())
}
