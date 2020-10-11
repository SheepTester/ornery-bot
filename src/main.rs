use std::{env, sync::Mutex};
use dotenv::dotenv;
use serenity::{
    model::{channel::Message, gateway::Ready},
    prelude::*,
};

struct Handler {
    count: Mutex<u32>,
}

impl EventHandler for Handler {
    fn message(&self, ctx: Context, msg: Message) {
        if msg.content == "ok moofy" {
            // https://doc.rust-lang.org/book/ch16-03-shared-state.html#sharing-a-mutext-between-multiple-threads
            let count = {
                let mut count = self.count.lock().unwrap();
                *count += 1;
                count
            };
            if let Err(why) = msg.channel_id.say(&ctx.http, count) {
                println!("Error sending message: {:?}", why);
            }
        } else if msg.content == "moofy ponder" {
            if let Err(why) = msg.channel_id.say(&ctx.http, "let me think") {
                println!("Error sending message: {:?}", why);
            }
            std::thread::sleep(std::time::Duration::from_millis(5000));
            if let Err(why) = msg.channel_id.say(&ctx.http, "done") {
                println!("Error sending message: {:?}", why);
            }
        }
    }

    fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

fn main() {
    dotenv().ok();

    // Configure the client with your Discord bot token in the environment.
    let token = env::var("DISCORD_TOKEN")
        .expect("Expected a token in the environment");

    // Create a new instance of the Client, logging in as a bot. This will
    // automatically prepend your bot token with "Bot ", which is a requirement
    // by Discord for bot users.
    let mut client = Client::new(&token, Handler {
        count: Mutex::new(0),
    }).expect("Err creating client");

    // Finally, start a single shard, and start listening to events.
    //
    // Shards will automatically attempt to reconnect, and will perform
    // exponential backoff until it reconnects.
    if let Err(why) = client.start() {
        println!("Client error: {:?}", why);
    }
}
