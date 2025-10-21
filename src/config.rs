use std::{fs, path::PathBuf};

use clap::Parser;
use serde::Deserialize;
use serenity::all::{ChannelId, GuildId};

#[derive(Parser, Debug)]
#[command(about, long_about = None)]
pub(crate) struct Args {
    #[arg(
        long = "config",
        conflicts_with_all = ["server", "log_channel", "honeypot_channel", "idiot_proof"],
        required_unless_present_any = ["server", "log_channel", "honeypot_channel", "idiot_proof"]
    )]
    config_file: Option<PathBuf>,

    #[arg(
        long,
        conflicts_with = "config_file",
        required_unless_present = "config_file"
    )]
    server: Option<GuildId>,
    #[arg(
        long,
        conflicts_with = "config_file",
        required_unless_present = "config_file"
    )]
    log_channel: Option<ChannelId>,
    #[arg(
        long,
        conflicts_with = "config_file",
        required_unless_present = "config_file"
    )]
    honeypot_channel: Option<ChannelId>,
    #[arg(
        long = "tolerant",
        short = 't',
        conflicts_with = "config_file",
        required_unless_present = "config_file"
    )]
    idiot_proof: bool,
}

impl Args {
    pub(crate) fn to_config(&self) -> Config {
        if let Some(ref cfg_path) = self.config_file {
            match fs::read_to_string(cfg_path) {
                Ok(cfg_str) => match toml::from_str(&cfg_str) {
                    Ok(config) => config,
                    Err(e) => {
                        panic!("Error parsing the provided config file: {e}");
                    }
                },
                Err(e) => {
                    panic!("Error reading the provided config file: {e}");
                }
            }
        } else {
            let server_config = ServerConfig {
                id: self.server.unwrap(),
                log_channel: self.log_channel.unwrap(),
                honeypot_channel: self.honeypot_channel.unwrap(),
            };

            Config {
                server: vec![server_config],
                default_actions: vec![ModerationActions::WarnMods], // TODO
                tolerant: self.idiot_proof,
            }
        }
    }
}

#[derive(Deserialize)]
pub(crate) struct Config {
    server: Vec<ServerConfig>,
    default_actions: Vec<ModerationActions>, // TODO: convert into bitflag
    tolerant: bool,
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
