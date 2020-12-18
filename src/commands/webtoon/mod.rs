use crate::{db, error_with_reason::ErrorWithReason};
use lazy_static::lazy_static;
use mongodb::bson::doc;
use rand::seq::SliceRandom;
use regex::Regex;
use select::{
    document::Document as HtmlDocument,
    predicate::{Class, Name, Predicate},
};
use serenity::{
    client::Context,
    framework::standard::{
        macros::{command, group},
        Args, CommandResult,
    },
    model::channel::Message,
    utils::Colour,
};
use tokio::stream::StreamExt;

#[group]
#[prefixes("webtoon", "webtoons")]
#[only_in(guilds)]
#[commands(add, remove, check, list)]
#[description = "Quickly fetch the latest Webtoons."]
struct Webtoon;

#[command]
#[usage = "<id> <webtoon URL>"]
#[example = "weakhero https://www.webtoons.com/en/action/weakhero/list?title_no=1726"]
#[required_permissions("MANAGE_GUILD")]
/// Introduce Moofy to a Webtoon. You can then check on the Webtoon using `:webtoon check <id>`.
async fn add(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let guild_id = match msg.guild_id {
        Some(id) => id.as_u64().to_owned(),
        None => {
            msg.channel_id
                .say(&ctx.http, "You aren't in a server.")
                .await?;
            return Ok(());
        }
    };

    let webtoon_id = args.single::<String>()?;
    let webtoon_url = args.rest();

    lazy_static! {
        // Unsure how strict the regex needs to be. I know that the Korean site is hosted on Naver
        // though.
        static ref VALID_ID: Regex = Regex::new(r"^[\w-]+$").unwrap();
        static ref VALID_URL: Regex = Regex::new(r"^https?://(\w+\.)?webtoons.com/").unwrap();
    }
    if !VALID_ID.is_match(&webtoon_id) {
        Err(ErrorWithReason::from("The given ID has too many special characters. Please just stick to letters, numbers, and hyphens."))?;
    }
    if !VALID_URL.is_match(webtoon_url) {
        Err(ErrorWithReason::from(
            r#"The given "URL" doesn't seem to be a Webtoons URL."#,
        ))?;
    }

    let data = ctx.data.read().await;
    let db = data.get::<db::Db>().expect("Expected Db in TypeMap.");
    let webtoons = db.collection("webtoons");

    if let Some(_) = webtoons
        .find_one(
            doc! {
                "guild": guild_id,
                "id": &webtoon_id,
            },
            None,
        )
        .await?
    {
        Err(format!(
            "A Webtoon with the given ID already exists! Try it: `:webtoon check {}`",
            webtoon_id
        ))?;
    }

    webtoons
        .insert_one(
            doc! {
                "guild": guild_id,
                "id": &webtoon_id,
                "url": &webtoon_url,
            },
            None,
        )
        .await?;

    msg.react(&ctx.http, '👌').await?;

    Ok(())
}

#[command]
#[usage = "<id>"]
#[example = "weakhero"]
#[required_permissions("MANAGE_GUILD")]
/// Remove a Webtoon by its ID.
async fn remove(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let guild_id = match msg.guild_id {
        Some(id) => id.as_u64().to_owned(),
        None => {
            msg.channel_id
                .say(&ctx.http, "You aren't in a server.")
                .await?;
            return Ok(());
        }
    };

    let webtoon_id = args.single::<String>()?;

    let data = ctx.data.read().await;
    let db = data.get::<db::Db>().expect("Expected Db in TypeMap.");
    let webtoons = db.collection("webtoons");

    let result = webtoons
        .delete_one(
            doc! {
                "guild": guild_id,
                "id": &webtoon_id,
            },
            None,
        )
        .await?;

    msg.channel_id
        .say(
            &ctx.http,
            match result.deleted_count {
                0 => "I didn't delete anything since there was nothing to delete.",
                1 => "The Webtoon has been deleted. 😢",
                _ => r"Strangely, I deleted more than one Webtoon with that ID. ¯\_(ツ)_/¯",
            },
        )
        .await?;

    Ok(())
}

const RANDOM_MESSAGE: [&str; 3] = [
    "I already read this...",
    "The last episode was pretty epic; you should read it!",
    "I approve.",
];

/// Returns `Ok(true)` if the Webtoon ID doesn't exist. This way, the error is only triggered when
/// using the long form `:webtoon check`.
pub async fn check_webtoon(
    ctx: &Context,
    msg: &Message,
    webtoon_id: &String,
) -> CommandResult<bool> {
    let guild_id = match msg.guild_id {
        Some(id) => id.as_u64().to_owned(),
        None => {
            msg.channel_id
                .say(&ctx.http, "You aren't in a server.")
                .await?;
            return Ok(false);
        }
    };

    let data = ctx.data.read().await;
    let db = data.get::<db::Db>().expect("Expected Db in TypeMap.");
    let webtoons = db.collection("webtoons");

    if let Some(doc) = webtoons
        .find_one(
            doc! {
                "guild": guild_id,
                "id": webtoon_id,
            },
            None,
        )
        .await?
    {
        let url = doc.get_str("url")?;
        // https://rust-lang-nursery.github.io/rust-cookbook/web/scraping.html
        let response = reqwest::get(url).await?.text().await?;
        let (title, first_image, episodes) = {
            // In a block because HtmlDocument is not Send and this pisses the async function off.
            let html = HtmlDocument::from(response.as_str());
            let title = html
                .find(Name("h1").and(Class("subj")))
                .next()
                .map_or_else(|| String::from("[Couldn't get title]"), |node| node.text());
            // Sadly, Webtoons checks for the Referer header for image URLs.
            let first_image = html
                .find(
                    Class("detail_lst")
                        .descendant(Class("thmb"))
                        .child(Name("img")),
                )
                .next()
                .and_then(|node| node.attr("src"))
                .map(|src| String::from(src));
            let episodes = html
                .find(Class("detail_lst").descendant(Name("li")))
                .take(5)
                .map(|episode| {
                    let name = episode
                        .find(Class("subj").child(Name("span")))
                        .next()
                        .map_or_else(
                            || String::from("[Couldn't get episode name]"),
                            |node| node.text(),
                        );
                    let date = episode.find(Class("date")).next().map_or_else(
                        || String::from("[Couldn't get episode date]"),
                        |node| node.text(),
                    );
                    let link = String::from(
                        episode
                            .find(Class("date"))
                            .next()
                            .and_then(|node| node.attr("href"))
                            .unwrap_or(url),
                    );
                    let up = episode.find(Class("tx_up")).next().is_some();
                    (name, date, link, up)
                })
                .collect::<Vec<(String, String, String, bool)>>();
            (title, first_image, episodes)
        };
        msg.channel_id
            .send_message(&ctx.http, |message| {
                message.embed(|embed| {
                    embed.title(title);
                    embed.url(url);
                    embed.colour(Colour::MAGENTA);
                    for (name, date, link, up) in episodes {
                        embed.field(
                            name,
                            format!("[{}]({}) {}", date, link, if up { " **UP**" } else { "" }),
                            false,
                        );
                    }
                    if let Some(image) = first_image {
                        embed.image(image);
                    }
                    embed
                });
                // https://stackoverflow.com/a/34215930
                message.content(
                    RANDOM_MESSAGE
                        .choose(&mut rand::thread_rng())
                        .unwrap_or(&""),
                );
                message
            })
            .await?;
        Ok(false)
    } else {
        Ok(true)
    }
}

#[command]
#[usage = "<id>"]
#[example = "weakhero"]
/// Check to see if a new episode for a Webtoon has been uploaded.
async fn check(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let webtoon_id = args.single::<String>()?;

    if check_webtoon(ctx, msg, &webtoon_id).await? {
        msg.channel_id
            .send_message(&ctx.http, |message| {
                message.embed(|embed| {
                    embed.description(format!("Hint: Request the mods to do `:webtoon add {} <url>` first.", webtoon_id));
                    embed
                });
                message.content("I don't know which Webtoon you're referring to; a Webtoon doesn't exist with that ID.");
                message
            })
            .await?;
    }

    Ok(())
}

#[command]
#[usage = ""]
#[example = ""]
/// Lists some Webtoon IDs added in the server.
async fn list(ctx: &Context, msg: &Message) -> CommandResult {
    let guild_id = match msg.guild_id {
        Some(id) => id.as_u64().to_owned(),
        None => {
            msg.channel_id
                .say(&ctx.http, "You aren't in a server.")
                .await?;
            return Ok(());
        }
    };

    let data = ctx.data.read().await;
    let db = data.get::<db::Db>().expect("Expected Db in TypeMap.");
    let webtoons = db.collection("webtoons");

    let mut docs_cursor = webtoons
        .find(
            doc! {
                "guild": guild_id,
            },
            None,
        )
        .await?;
    let mut webtoon_ids = Vec::new();
    while let Some(doc_result) = docs_cursor.next().await {
        let doc = doc_result?;
        if let (Ok(id), Ok(url)) = (doc.get_str("id"), doc.get_str("url")) {
            webtoon_ids.push(format!("[`{}`]({})", id, url));
        }
    }
    msg.channel_id.send_message(&ctx.http, |message| {
        message.embed(|embed| {
            let ids = webtoon_ids.join("\n");
            embed.description(if ids.is_empty() {
                String::from("No Webtoons have been added. The server's mods should do `:webtoon add <id> <url>` to add some Webtoons.")
            } else if ids.len() > 2000 {
                format!("{}\n[...]", &ids[0..(2000 - 6)])
            } else {
                ids
            });
            embed
        });
        message.content("Use `:webtoon check <id>` to check on an individual Webtoon by ID.\n\nTip: For most Webtoon IDs, you can simply just do `:<id>`.");
        message
    }).await?;

    Ok(())
}
