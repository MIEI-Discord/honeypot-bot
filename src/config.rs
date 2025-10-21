use std::path::PathBuf;

use clap::Parser;
use serde::Deserialize;
use serenity::all::{ChannelId, GuildId};

#[derive(Parser, Debug)]
#[command(about, long_about = None)]
struct Args {
    #[arg(long = "config", conflicts_with_all = [""], required_unless_present_any = [""])]
    config_file: PathBuf,
}

#[derive(Deserialize)]
struct Config {
    server: Vec<ServerConfig>,
    default_actions: Vec<ModerationActions>,
}

#[derive(Deserialize)]
struct ServerConfig {
    id: GuildId,
    log_channel: ChannelId,
    honeypot_channel: ChannelId,
}

#[derive(Deserialize)]
enum ModerationActions {
    WarnMods,
    EraseMessages,
    Mute,
    Kick,
    Ban,
}
