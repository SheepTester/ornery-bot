use dotenv::dotenv;
use serenity::{
    model::{channel::Message, gateway::Ready, id::GuildId},
    prelude::*,
};
use std::{env, sync::{Mutex, MutexGuard, mpsc::{channel, Sender, Receiver}}, collections::HashMap};

type MaybeError = serenity::Result<()>;

struct GuildData {
    count: u32,
}

struct Handler {
    count: Mutex<u32>,
    // guild_data_load: (Sender<GuildId>, Receiver<GuildData>),
    // guild_data: Mutex<HashMap<GuildId, GuildData>>,
}

impl Handler {
    // fn access_guild_data(&self, id: GuildId) -> &mut GuildData {
    //     {
    //         let mut map = self.guild_data.lock().unwrap();
    //         match map.get_mut(id) {
    //             Some(data) => data,
    //             None => (),
    //         }
    //     }
    //     let (requestGuildData, guildDataDeliveries) = self.guild_data_load;
    //     requestGuildData(id);
    //     let guild_data =
    //     let mut map = self.guild_data.lock().unwrap();
    // }

    fn message(&self, ctx: Context, msg: Message) -> MaybeError {
        let current_user = ctx.http.get_current_user()?;
        if msg.mentions_user_id(&current_user) {
            msg.channel_id.say(&ctx.http, "<:ping:719277539113041930>")?;
        } else if msg.content == "ok moofy" {
            // https://doc.rust-lang.org/book/ch16-03-shared-state.html#sharing-a-mutext-between-multiple-threads
            let count = {
                let mut count = self.count.lock().unwrap();
                *count += 1;
                count
            };
            msg.channel_id.say(&ctx.http, count)?;
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

    // let (requestGuildData, guildDataRequests) = mpsc::channel();
    // let (sendGuildData, guildDataDeliveries) = mpsc::channel();
    // thread::spawn(move || {
    //     for request in guildDataRequests {
    //         // TODO: Read from or save to file
    //         guildDataDeliveries.send(GuildData {
    //             count: 0,
    //         }).unwrap();
    //     }
    // });

    // Create a new instance of the Client, logging in as a bot. This will
    // automatically prepend your bot token with "Bot ", which is a requirement
    // by Discord for bot users.
    let mut client = Client::new(
        &token,
        Handler {
            count: Mutex::new(0),
            // guild_data_load: (requestGuildData, guildDataDeliveries),
            // guild_data: Mutex::new(HashMap::new()),
        },
    )
    .expect("Err creating client");

    // Finally, start a single shard, and start listening to events.
    //
    // Shards will automatically attempt to reconnect, and will perform
    // exponential backoff until it reconnects.
    if let Err(why) = client.start() {
        println!("Client error: {:?}", why);
    }
}
