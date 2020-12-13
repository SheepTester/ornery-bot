# ornery-bot

With whois from HarVM and the wisdom of Wise Guy.

To install, go to the [Releases](https://github.com/SheepTester/ornery-bot/releases) page and download ornery-bot.exe. Then, create a file called `.env` with

```
DISCORD_TOKEN=<discord token>
```

in the same folder as ornery-bot.exe, then do

```sh
./ornery-bot
```

## Development

[Rust](https://www.rust-lang.org/) is cool and good.

```sh
cargo run
```

To avoid conflicts with the bot running in production, you can add

```
PREFIX=~
```

to use a different prefix for development.
