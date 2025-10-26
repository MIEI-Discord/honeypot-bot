use serenity::{
    all::{Context, EventHandler, Message, MessageBuilder, Ready},
    async_trait,
};

use crate::config::{Config, ModerationActions};

pub(crate) struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        let ctx_data = ctx.data.read();

        match ctx_data.await.get::<Config>() {
            Some(cfg) => {
                if let Some(serv_id) = msg.guild_id {
                    match cfg.servers.get(&serv_id) {
                        Some(serv) => {
                            if msg.channel_id == serv.honeypot_channel {
                                // CAUGHT ONE! 🐻🍯
                                let log_message = MessageBuilder::new()
                                    .mention(&serv.warn_role)
                                    .push_line_safe("")
                                    .push_safe("User ")
                                    .user(msg.author)
                                    .push_line_safe(" was caught by the honeypot!");

                                match serv.log_channel.to_channel(&ctx).await {
                                    Ok(chan) => {
                                        if serv.tolerant {
                                        } else {
                                        }
                                    }
                                    Err(e) => todo!(),
                                }
                            }
                        }
                        None => {
                            // Bot is in a server that it is not configured to handle, log warning
                            todo!()
                        }
                    }
                } else {
                    // Bot is being DM'd?
                    todo!();
                }
            }
            None => {
                todo!();
            }
        }
    }

    async fn ready(&self, ctx: Context, _: Ready) {
        let ctx_data = ctx.data.read();

        match ctx_data.await.get::<Config>() {
            Some(cfg) => {
                for (server_id, server_config) in &cfg.servers {
                    match server_id.channels(&ctx).await {
                        Ok(serv_chans) => {
                            if !serv_chans.contains_key(&server_config.honeypot_channel)
                                || !serv_chans.contains_key(&server_config.log_channel)
                            {
                                // Channels don't exist in this server
                                todo!();
                                return;
                            }
                        }
                        Err(e) => todo!(),
                    }

                    match server_id.roles(&ctx).await {
                        Ok(serv_roles) => {
                            if !serv_roles.contains_key(&server_config.warn_role) {
                                // Role doesn't exist in this server
                                todo!();
                                return;
                            }
                        }
                        Err(e) => todo!(),
                    }

                    let mut hello_message = MessageBuilder::new();
                    hello_message
                        .push_line("Hello! I'm the beekeeper! 🧑‍🌾")
                        .push_line_safe("")
                        .push_safe("The honeypot has been installed in ")
                        .channel(server_config.honeypot_channel)
                        .push_line_safe(".")
                        .push_safe("Logs will be written to the current channel (")
                        .channel(server_config.log_channel)
                        .push_line_safe(").")
                        .push_line_safe("")
                        .push_safe("Tolerant mode is ")
                        .push_bold_safe(if server_config.tolerant {
                            "enabled"
                        } else {
                            "disabled"
                        })
                        .push_line_safe(".");

                    if server_config.tolerant
                        || server_config
                            .mod_actions
                            .contains(ModerationActions::WarnMods)
                    {
                        hello_message
                            .role(server_config.warn_role)
                            .push_line_safe(" will be warned when the honeypot is triggered.");
                    }

                    if server_config.mod_actions.intersects(
                        ModerationActions::Mute | ModerationActions::Kick | ModerationActions::Ban,
                    ) {
                        hello_message.push_safe("Offending users will be ");
                        let mut first_action = true;

                        if server_config.mod_actions.contains(ModerationActions::Mute) {
                            first_action = false;
                            hello_message.push_bold_safe("muted");
                        }

                        if server_config.mod_actions.contains(ModerationActions::Kick) {
                            if !first_action {
                                hello_message.push_safe(", ");
                            }
                            first_action = false;
                            hello_message.push_bold_safe("kicked");
                        }

                        if server_config.mod_actions.contains(ModerationActions::Ban) {
                            if !first_action {
                                hello_message.push_safe(", ");
                            }
                            hello_message.push_bold_safe("banned");
                        }

                        if server_config
                            .mod_actions
                            .contains(ModerationActions::EraseMessages)
                        {
                            hello_message.push_safe(" and their spam messages will be deleted");
                        }
                        hello_message.push_line_safe(".");
                    } else if server_config
                        .mod_actions
                        .contains(ModerationActions::EraseMessages)
                    {
                        hello_message
                            .push_line_safe("Offending users' spam messages will be deleted.");
                    }

                    if let Err(e) = server_config
                        .log_channel
                        .say(&ctx, hello_message.build())
                        .await
                    {
                        todo!();
                    }
                }
            }
            None => {
                todo!();
            }
        }
    }
}
