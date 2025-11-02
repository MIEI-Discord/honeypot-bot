use std::{collections::HashMap, fs, path::PathBuf};

use clap::{Parser, ValueHint, value_parser};
use serde::Deserialize;
use serenity::{
    all::{ChannelId, GuildId, RoleId},
    prelude::TypeMapKey,
};

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
    pub(crate) servers: HashMap<GuildId, ServerConfig>,
}

impl TypeMapKey for Config {
    type Value = Self;
}

#[derive(Deserialize, Debug)]
pub(crate) struct ServerConfig {
    pub(crate) log_channel: ChannelId,
    pub(crate) honeypot_channel: ChannelId,
    pub(crate) mod_role: RoleId,
    #[serde(default)]
    pub(crate) mod_actions: ModerationActions,
    #[serde(default = "ServerConfig::default_warn_mods")]
    pub(crate) warn_mods: bool,
    #[serde(default = "ServerConfig::default_erase_messages")]
    pub(crate) erase_messages: bool,
    pub(crate) tolerant: bool,
}

impl ServerConfig {
    fn default_warn_mods() -> bool {
        true
    }

    fn default_erase_messages() -> bool {
        true
    }
}

#[derive(Deserialize, Debug, Default)]
pub(crate) enum ModerationActions {
    #[default]
    Mute,
    Kick,
    Ban,
}
