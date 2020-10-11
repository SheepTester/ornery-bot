use dotenv::dotenv;
use serenity::{
    model::{channel::Message, gateway::Ready, id::GuildId},
    prelude::*,
};
use std::{collections::HashMap, env, sync::Mutex};

type MaybeError = serenity::Result<()>;

#[derive(Default)]
struct GuildData {
    count: u32,
}

struct GuildDataKey;

impl TypeMapKey for GuildDataKey {
    type Value = HashMap<GuildId, GuildData>;
}

struct Handler {
    count: Mutex<u32>,
}

impl Handler {
    fn message(&self, ctx: Context, msg: Message) -> MaybeError {
        let current_user = ctx.http.get_current_user()?;
        if msg.mentions_user_id(&current_user) {
            msg.channel_id
                .say(&ctx.http, "<:ping:719277539113041930>")?;
        } else if msg.content == "ok moofy" {
            // https://doc.rust-lang.org/book/ch16-03-shared-state.html#sharing-a-mutext-between-multiple-threads
            let count = {
                let mut count = self.count.lock().unwrap();
                *count += 1;
                count
            };
            msg.channel_id.say(&ctx.http, count)?;
        } else if msg.content == "kk moofy" {
            // https://docs.rs/serenity/0.8.7/serenity/client/struct.Client.html#structfield.data
            match msg.guild_id {
                Some(guild_id) => {
                    let count = {
                        let mut data = ctx.data.write();
                        let map = data.get_mut::<GuildDataKey>().unwrap();
                        let guild_data =
                            map.entry(guild_id).or_insert_with(|| GuildData::default());
                        guild_data.count += 1;
                        guild_data.count
                    };
                    msg.channel_id.say(&ctx.http, format!("{}ish", count))?;
                }
                None => {
                    msg.channel_id.say(&ctx.http, "server only hehehe")?;
                }
            }
        } else if msg.content == "moofy ponder" {
            msg.channel_id.say(&ctx.http, "let me think")?;
            std::thread::sleep(std::time::Duration::from_millis(5000));
            msg.channel_id.say(&ctx.http, "done")?;
        } else if msg.content.starts_with(":whois") {
            msg.channel_id.say(&ctx.http, "idk lmao")?;
        }
        return Ok(());
    }

    fn ready(&self, _: Context, ready: Ready) -> MaybeError {
        println!("{} is connected!", ready.user.name);
        return Ok(());
    }
}

impl EventHandler for Handler {
    fn message(&self, ctx: Context, msg: Message) {
        if let Err(why) = self.message(ctx, msg) {
            println!("Error from message handler: {:?}", why);
        }
    }

    fn ready(&self, ctx: Context, ready: Ready) {
        if let Err(why) = self.ready(ctx, ready) {
            println!("Error from ready handler: {:?}", why);
        }
    }
}

fn main() {
    dotenv().ok();

    // Configure the client with your Discord bot token in the environment.
    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");

    let mut client = Client::new(
        &token,
        Handler {
            count: Mutex::new(0),
        },
    )
    .expect("Err creating client");

    {
        let mut data = client.data.write();
        data.insert::<GuildDataKey>(HashMap::default());
    }

    // Finally, start a single shard, and start listening to events.
    //
    // Shards will automatically attempt to reconnect, and will perform
    // exponential backoff until it reconnects.
    if let Err(why) = client.start() {
        println!("Client error: {:?}", why);
    }
}
