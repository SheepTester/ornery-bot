//! Requires the 'framework' feature flag be enabled in your project's
//! `Cargo.toml`.
//!
//! This can be enabled by specifying the feature in the dependency section:
//!
//! ```toml
//! [dependencies.serenity]
//! git = "https://github.com/serenity-rs/serenity.git"
//! features = ["framework", "standard_framework"]
//! ```
use commands::hooks;
use serenity::{
    async_trait,
    client::{Context, EventHandler},
    framework::standard::{macros::command, Args, CommandResult, StandardFramework},
    http::Http,
    model::{
        channel::Message,
        gateway::{Activity, Ready},
        id::GuildId,
    },
    Client,
};
use std::{
    collections::{HashMap, HashSet},
    env,
    sync::Arc,
};

mod commands;
mod db;
mod error_with_reason;

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("ready.");
        if let Some(shard) = ready.shard {
            // Note that array index 0 is 0-indexed, while index 1 is 1-indexed.
            // This may seem unintuitive, but it models Discord's behaviour.
            println!(
                "{} is connected on shard {}/{}!",
                ready.user.name, shard[0], shard[1],
            );
        }
        ctx.set_activity(Activity::listening(":help")).await;
    }

    // https://github.com/Flat/Lupusregina-/blame/6ce8d19e34fac4e8aa573deeaa8af81b2f28dad7/src/main.rs#L51
    async fn cache_ready(&self, ctx: Context, _guilds: Vec<GuildId>) {
        println!("cache_ready.");
        // for guild_id in guilds {
        //     println!("{}", guild_id);
        //     ctx.shard
        //         .chunk_guild(guild_id, None, ChunkGuildFilter::None, None);
        // }
        println!("{} unknown members", ctx.cache.unknown_members().await);
    }
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();

    // Configure the client with your Discord bot token in the environment.
    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");

    let http = Http::new_with_token(&token);

    // We will fetch your bot's owners and id
    let (owners, bot_id) = match http.get_current_application_info().await {
        Ok(info) => {
            let mut owners = HashSet::new();
            if let Some(team) = info.team {
                owners.insert(team.owner_user_id);
            } else {
                owners.insert(info.owner.id);
            }
            match http.get_current_user().await {
                Ok(bot_id) => (owners, bot_id.id),
                Err(why) => panic!("Could not access the bot id: {:?}", why),
            }
        }
        Err(why) => panic!("Could not access application info: {:?}", why),
    };

    let framework = StandardFramework::new()
        .configure(|c| {
            c.with_whitespace(true)
                .on_mention(Some(bot_id))
                .prefix(":")
                // Sets the bot's owners. These will be used for commands that
                // are owners only.
                .owners(owners)
                .on_mention(Some(bot_id))
        })
        // Set a function to be called prior to each command execution. This
        // provides the context of the command, the message that was received,
        // and the full name of the command that will be called.
        //
        // You can not use this to determine whether a command should be
        // executed. Instead, the `#[check]` macro gives you this functionality.
        //
        // **Note**: Async closures are unstable, you may use them in your
        // application if you are fine using nightly Rust.
        // If not, we need to provide the function identifiers to the
        // hook-functions (before, after, normal, ...).
        .before(hooks::before)
        // Similar to `before`, except will be called directly _after_
        // command execution.
        .after(hooks::after)
        // Set a function that's called whenever a command's execution didn't complete for one
        // reason or another. For example, when a user has exceeded a rate-limit or a command
        // can only be performed by the bot owner.
        .on_dispatch_error(hooks::dispatch_error)
        .normal_message(hooks::normal_message)
        .prefix_only(hooks::normal_message)
        // Can't be used more than once per 5 seconds:
        .bucket("emoji", |b| b.delay(5))
        .await
        // Can't be used more than 2 times per 30 seconds, with a 5 second delay:
        .bucket("complicated", |b| b.delay(5).time_span(30).limit(2))
        .await
        // The `#[group]` macro generates `static` instances of the options set for the group.
        // They're made in the pattern: `#name_GROUP` for the group instance and `#name_GROUP_OPTIONS`.
        // #name is turned all uppercase
        .help(&commands::help::MY_HELP)
        .group(&commands::general::GENERAL_GROUP)
        .group(&commands::test::TEST_GROUP)
        .group(&commands::whois::WHOIS_GROUP)
        .group(&commands::emoji::EMOJI_GROUP)
        .group(&commands::math::MATH_GROUP)
        .group(&commands::owner::OWNER_GROUP);

    let mut client = Client::builder(&token)
        .event_handler(Handler)
        .framework(framework)
        // https://github.com/serenity-rs/serenity/blob/current/examples/e11_gateway_intents/src/main.rs#L40
        // .add_intent(GatewayIntents::GUILD_MEMBERS)
        .await
        .expect("Err creating client");

    let db = db::init_db()
        .await
        .expect("Problem connecting to the MongoDB server.");

    // {
    //     let sm = client.shard_manager.lock().await;
    //     for guild_id in client.cache_and_http.cache.guilds().await {
    //         sm.chunk_guild(guild_id, None, ChunkGuildFilter::None, None);
    //     }
    // }

    {
        let mut data = client.data.write().await;
        data.insert::<commands::general::CommandCounter>(HashMap::default());
        data.insert::<commands::general::ShardManagerContainer>(Arc::clone(&client.shard_manager));
        data.insert::<db::Db>(db);
    }

    {
        let shard_manager = client.shard_manager.clone();

        tokio::spawn(async move {
            tokio::signal::ctrl_c()
                .await
                .expect("Could not register ctrl+c handler");
            shard_manager.lock().await.shutdown_all().await;
        });
    }

    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}

#[command]
async fn about_role(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let potential_role_name = args.rest();

    if let Some(guild) = msg.guild(&ctx.cache).await {
        // `role_by_name()` allows us to attempt attaining a reference to a role
        // via its name.
        if let Some(role) = guild.role_by_name(&potential_role_name) {
            if let Err(why) = msg
                .channel_id
                .say(&ctx.http, &format!("Role-ID: {}", role.id))
                .await
            {
                println!("Error sending message: {:?}", why);
            }

            return Ok(());
        }
    }

    msg.channel_id
        .say(
            &ctx.http,
            format!("Could not find role named: {:?}", potential_role_name),
        )
        .await?;

    Ok(())
}
