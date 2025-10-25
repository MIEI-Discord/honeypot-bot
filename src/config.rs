use std::{collections::HashMap, fs, path::PathBuf};

use bitflags::bitflags;
use clap::{Parser, ValueHint, value_parser};
use serde::Deserialize;
use serenity::all::{ChannelId, GuildId};

#[derive(Parser, Debug)]
#[command(about, long_about = None)]
pub(crate) struct Args {
    #[arg(
        long = "config",
        value_hint = ValueHint::FilePath,
        default_value = "config.toml",
        value_parser = value_parser!(PathBuf)
    )]
    config_file: PathBuf,
}

impl Args {
    pub(crate) fn to_config(&self) -> Config {
        match fs::read_to_string(&self.config_file) {
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
    }
}

#[derive(Deserialize)]
pub(crate) struct Config {
    servers: HashMap<GuildId, ServerConfig>,
}

#[derive(Deserialize)]
struct ServerConfig {
    log_channel: ChannelId,
    honeypot_channel: ChannelId,
    #[serde(default)]
    mod_actions: ModerationActions,
    tolerant: bool,
}

bitflags! {
    #[derive(Deserialize)]
    struct ModerationActions: u8 {
        const WarnMods = 0b00001;
        const EraseMessages = 0b00010;
        const Mute = 0b00100;
        const Kick = 0b01000;
        const Ban = 0b10000;
    }
}

impl Default for ModerationActions {
    fn default() -> Self {
        Self::WarnMods | Self::EraseMessages | Self::Mute
    }
}
