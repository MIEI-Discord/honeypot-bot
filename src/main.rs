use serenity::{Client, all::GatewayIntents};
use std::env;

use crate::handler::Handler;

mod config;
mod handler;

#[tokio::main]
async fn main() {
    let token = env::var("DISCORD_TOKEN")
        .expect("Discord bot token not set, make sure to set the `DISCORD_TOKEN` environment variable in your deployment.");

    let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT;

    let mut client = Client::builder(&token, intents)
        .event_handler(Handler)
        .await
        .expect("Error initializing the Discord bot client.");

    if let Err(e) = client.start().await {
        eprintln!("The bot client encountered an error: {e}");
    }
}
