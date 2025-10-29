use clap::Parser;
use serenity::{Client, all::GatewayIntents};
use std::env;
use tracing::instrument;

use crate::{
    config::{Args, Config},
    handler::Handler,
};

mod config;
mod handler;

#[tokio::main]
#[instrument]
async fn main() {
    tracing_subscriber::fmt::init();

    #[cfg(all(not(debug_assertions), feature = "dev_env"))]
    compile_error!(
        "Loading the Discord token from a `.env` file through the `dev_env` feature is only supported for debug builds, for security reasons. Please compile in debug mode or remove the `dev_env` feature."
    );

    #[cfg(all(debug_assertions, feature = "dev_env"))]
    dotenvy::dotenv().ok();

    let token = env::var("DISCORD_TOKEN")
        .expect("Discord bot token not set, make sure to set the `DISCORD_TOKEN` environment variable in your deployment.");

    let config = Args::parse().to_config();
    let num_servers = config.servers.len();

    let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT;

    let mut client = Client::builder(&token, intents)
        .event_handler(Handler)
        .type_map_insert::<Config>(config)
        .await
        .expect("Error initializing the Discord bot client.");

    // Simple case - no need for sharding
    if num_servers < 2500 {
        if let Err(e) = client.start().await {
            eprintln!("The bot client encountered an error: {e}");
        }
    } else {
        // Handling large servers - use case unsupported (for now)
        unimplemented!();
    }
}
