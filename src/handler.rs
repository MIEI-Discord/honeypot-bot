use chrono::{Days, Local};
use serenity::{
    all::{
        ButtonStyle, Context, CreateActionRow, CreateButton, CreateEmbed, CreateEmbedAuthor,
        CreateMessage, EditMember, EmbedMessageBuilding, EventHandler, GetMessages, GuildId,
        Message, MessageBuilder, Ready, Timestamp, UserId,
    },
    async_trait,
};
use tracing::{error, instrument, warn};

use crate::config::{Config, ModerationActions, ServerConfig};

#[derive(Debug)]
pub(crate) struct Handler;

#[async_trait]
impl EventHandler for Handler {
    #[instrument]
    async fn message(&self, ctx: Context, msg: Message) {
        let ctx_data = ctx.data.read();

        match ctx_data.await.get::<Config>() {
            Some(cfg) => {
                if let Some(serv_id) = msg.guild_id {
                    match cfg.servers.get(&serv_id) {
                        Some(serv) => {
                            if msg.channel_id == serv.honeypot_channel {
                                // CAUGHT ONE! 🐻🍯

                                let mut mod_log = CreateMessage::new();

                                let mut log_message = MessageBuilder::new();

                                if serv.mod_actions.contains(ModerationActions::WarnMods) {
                                    log_message.mention(&serv.warn_role);
                                }

                                log_message
                                    .push_line_safe("")
                                    .push("🐻🍯 User ")
                                    .user(&msg.author)
                                    .push_line_safe(" was caught by the honeypot!");

                                let author_name = match msg.author.nick_in(&ctx, serv_id).await {
                                    Some(nick) => nick,
                                    None => msg.author.display_name().to_owned(),
                                };

                                let usr_msg = msg.content_safe(&ctx);

                                let user_message = CreateEmbed::new()
                                    .author(
                                        CreateEmbedAuthor::new(author_name)
                                            .icon_url(msg.author.face()),
                                    )
                                    .title("Original message")
                                    .description(&usr_msg)
                                    .timestamp(msg.timestamp);

                                mod_log = mod_log.embed(user_message);

                                let identical_messages = Self::search_for_spam_messages(
                                    &ctx,
                                    msg.author.id,
                                    &usr_msg,
                                    (serv_id, serv),
                                )
                                .await;

                                // Tolerant mode is on -- if there are no other identical messages, we might just be dealing with someone who did not read the honeypot channel warning... 🙃
                                // Let the mods decide for themselves
                                if serv.tolerant && identical_messages.is_empty() {
                                    let mod_actions = CreateActionRow::Buttons(vec![
                                        CreateButton::new("mod_act")
                                            .label("Perform Mod Actions")
                                            .style(ButtonStyle::Danger),
                                        CreateButton::new("mod_dismiss")
                                            .label("Dismiss")
                                            .style(ButtonStyle::Secondary),
                                    ]);

                                    mod_log = mod_log.components(vec![mod_actions]);

                                    log_message
                                        .push_line_safe("")
                                        .push_bold_safe("Tolerant mode:")
                                        .push_line_safe(" The honeypot may have been triggered by mistake, please verify it and press the buttons to take action or dismiss the report.");

                                    if let Err(e) = serv
                                        .log_channel
                                        .send_message(&ctx, mod_log.content(log_message.build()))
                                        .await
                                    {
                                        error!(
                                            "There was an error sending the action log message to the log channel {}: {}",
                                            serv.log_channel, e
                                        );
                                    }
                                } else {
                                    let mut first_action = true;
                                    if serv.mod_actions.contains(ModerationActions::Ban) {
                                        first_action = false;
                                        log_message
                                            .push_line_safe("")
                                            .push_safe("The user was banned");

                                        if let Err(e) = serv_id.ban_with_reason(&ctx, msg.author.id, 1, "Caught spamming by the honeypot; account may be compromised").await {
                                            error!("There was an issue banning the user {}: {}", msg.author.id, e);

                                            let warning_message = MessageBuilder::new()
                                                .push("⚠️ I was unable to ban the user ")
                                                .user(msg.author.id)
                                                .push_line_safe("; Please make sure the compromised account is dealt with.")
                                                .build();

                                            if let Err(e) = serv.log_channel.say(&ctx, warning_message).await {
                                                error!("There was an issue sending the ban failure warning to the channel {}: {}", serv.log_channel, e);
                                            }
                                        }
                                    }

                                    if serv.mod_actions.contains(ModerationActions::Kick) {
                                        if first_action {
                                            first_action = false;

                                            log_message
                                                .push_line_safe("")
                                                .push_safe("The user was kicked");
                                        }

                                        if let Err(e) = serv_id.kick_with_reason(&ctx, msg.author.id, "Caught spamming by the honeypot; account may be compromised").await {
                                            error!("There was an issue kicking the user {}: {}", msg.author.id, e);

                                            let warning_message = MessageBuilder::new()
                                                .push("⚠️ I was unable to kick the user ")
                                                .user(msg.author.id)
                                                .push_line_safe("; Please make sure the compromised account is dealt with.")
                                                .build();

                                            if let Err(e) = serv.log_channel.say(&ctx, warning_message).await {
                                                error!("There was an issue sending the kick failure warning to the channel {}: {}", serv.log_channel, e);
                                            }
                                        }
                                    }

                                    if serv.mod_actions.contains(ModerationActions::Mute) {
                                        if first_action {
                                            first_action = false;

                                            log_message
                                                .push_line_safe("")
                                                .push_safe("The user was timed out for 1 day");
                                        }

                                        // 1-day mute hardcoded for now
                                        let duration = Local::now().checked_add_days(Days::new(1));

                                        match duration {
                                            Some(time) => {
                                                if let Err(e) = serv_id.edit_member(
                                                    &ctx,
                                                    msg.author.id,
                                                    EditMember::new()
                                                        .disable_communication_until_datetime(Timestamp::from(time))
                                                        .audit_log_reason("Caught spamming by the honeypot; account may be compromised")
                                                ).await {
                                                    error!("There was an issue timing the user {} out: {}", msg.author.id, e);

                                                    let warning_message = MessageBuilder::new()
                                                        .push("⚠️ I was unable to time the user ")
                                                        .user(msg.author.id)
                                                        .push_line_safe(" out; Please make sure the compromised account is dealt with.")
                                                        .build();

                                                    if let Err(e) = serv.log_channel.say(&ctx, warning_message).await {
                                                        error!("There was an issue sending the timeout failure warning to the channel {}: {}", serv.log_channel, e);
                                                    }
                                                }
                                            },
                                            None => {
                                                error!("There was an issue timing the user {} out: Timeout duration returned None.", msg.author.id);

                                                let warning_message = MessageBuilder::new()
                                                    .push("⚠️ I was unable to time the user ")
                                                    .user(msg.author.id)
                                                    .push_line_safe(" out; Please make sure the compromised account is dealt with.")
                                                    .build();

                                                if let Err(e) = serv.log_channel.say(&ctx, warning_message).await {
                                                    error!("There was an issue sending the timeout failure warning to the channel {}: {}", serv.log_channel, e);
                                                }
                                            }
                                        }
                                    }

                                    if serv.mod_actions.contains(ModerationActions::EraseMessages)
                                        && !serv.mod_actions.contains(ModerationActions::Ban)
                                    {
                                        if first_action {
                                            first_action = false;

                                            log_message
                                                .push_line_safe("")
                                                .push_safe("The user's spam messages were deleted");
                                        } else {
                                            log_message.push_safe(
                                                " and the user's spam messages were deleted",
                                            );
                                        }

                                        for ident_msg in identical_messages {
                                            if let Err(e) = ident_msg.delete(&ctx).await {
                                                error!(
                                                    "Unable to delete message {} in channel {}: {}",
                                                    ident_msg.id, ident_msg.channel_id, e
                                                );

                                                let warning_message = MessageBuilder::new()
                                                    .push("⚠️ I was unable to delete the message in channel ")
                                                    .channel(ident_msg.channel_id)
                                                    .push_line_safe("; Please make sure any spam messages are deleted.")
                                                    .build();

                                                if let Err(e) = serv
                                                    .log_channel
                                                    .say(&ctx, warning_message)
                                                    .await
                                                {
                                                    error!(
                                                        "There was an issue sending the message deletion failure warning regarding message {} to the channel {}: {}",
                                                        ident_msg.id, serv.log_channel, e
                                                    );
                                                }
                                            }
                                        }
                                    }

                                    log_message.push_line_safe(".");
                                    if let Err(e) = serv
                                        .log_channel
                                        .send_message(&ctx, mod_log.content(log_message.build()))
                                        .await
                                    {
                                        error!(
                                            "There was an error sending the action log message to the log channel {}: {}",
                                            serv.log_channel, e
                                        );
                                    }
                                }
                            }
                        }
                        None => {
                            // Bot is in a server that it is not configured to handle, log warning
                            warn!(
                                "It appears the bot was added to a server it was not configured to handle; please add the appropriate configuration for server {} or remove the bot from the server, if this was unintentional.",
                                serv_id
                            )
                        }
                    }
                } else {
                    // Bot is being DM'd?
                    warn!(
                        "Bot received a message outside of a server, possibly a DM - sending help message."
                    );

                    let warning_message = MessageBuilder::new()
                        .push_line_safe("🍯 Hi! I am a honeypot bot designed to catch compromised accounts spamming phishing URLs in servers.")
                        .push_line_safe("I have no functionality to offer you outside of this.")
                        .push_line("- If you think you were unfairly banned by this bot and would like to appeal, please contact the moderators of the server in question.")
                        .push_line("- If you would like to submit an issue regarding the bot (bug report, improvement request, etc.), please do so at ")
                        .push_named_link_safe("the official repository", "https://github.com/MIEI-Discord/honeypot-bot")
                        .push_line_safe(".")
                        .build();

                    if let Err(e) = msg.reply(&ctx, warning_message).await {
                        error!("There was an issue sending the help message: {}", e);
                    }
                }
            }
            None => {
                error!(
                    "There was an issue loading the bot's configuration. Please check if the configuration file is available and contains all the necessary configuration options."
                );
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
                                warn!(
                                    "The provided channels ({}, {}) do not correspond to any channels available on the server {}",
                                    &server_config.honeypot_channel,
                                    &server_config.log_channel,
                                    server_id
                                );
                                return;
                            }
                        }
                        Err(e) => {
                            error!(
                                "The bot was unable to retrieve the channels from server {}: {}",
                                server_id, e
                            )
                        }
                    }

                    match server_id.roles(&ctx).await {
                        Ok(serv_roles) => {
                            if !serv_roles.contains_key(&server_config.warn_role) {
                                // Role doesn't exist in this server
                                warn!(
                                    "The provided role ({}) does not correspond to any roles available on the server {}",
                                    &server_config.warn_role, server_id
                                );
                                return;
                            }
                        }
                        Err(e) => {
                            error!(
                                "The bot was unable to retrieve the roles from server {}: {}",
                                server_id, e
                            )
                        }
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
                        error!("There was an issue sending the welcome message: {}", e);
                    }
                }
            }
            None => {
                error!(
                    "There was an issue loading the bot's configuration. Please check if the configuration file is available and contains all the necessary configuration options."
                );
            }
        }
    }
}

impl Handler {
    #[instrument]
    async fn search_for_spam_messages(
        ctx: &Context,
        user: UserId,
        content: &str,
        server: (GuildId, &ServerConfig),
    ) -> Vec<Message> {
        match server.0.channels(ctx).await {
            Ok(chans) => {
                let mut matched_mesgs: Vec<Message> = Vec::with_capacity(chans.len());
                for (_, chan) in chans {
                    if chan.id != server.1.honeypot_channel {
                        match chan.messages(ctx, GetMessages::new().limit(100)).await {
                            Ok(mesgs) => {
                                matched_mesgs.extend(mesgs.into_iter().filter(|mesg| {
                                    mesg.author.id == user && mesg.content_safe(ctx) == content
                                }));
                            }
                            Err(e) => {
                                error!(
                                    "There was an issue obtaining the messages from user {} in channel {}: {}",
                                    user, chan.id, e
                                );

                                let warning_message = MessageBuilder::new()
                                    .push("⚠️ I was unable to retrieve the messages from ")
                                    .user(user)
                                    .push_safe(" in ")
                                    .channel(chan.id)
                                    .push_line_safe("; please make sure to check for any left-over spam in that channel.")
                                    .build();

                                if let Err(e) = server.1.log_channel.say(ctx, warning_message).await
                                {
                                    error!("There was an issue sending the warning message: {}", e);
                                }
                            }
                        }
                    }
                }
                matched_mesgs
            }
            Err(e) => {
                error!(
                    "There was an issue obtaining the list of channels in server {} to search for potential spam from user {}: {}",
                    server.0, user, e
                );

                let warning_message = MessageBuilder::new()
                    .push_line("⚠️ I was unable to retrieve the list of channels.")
                    .push_safe("Please make sure to check the server for any left-over spam from ")
                    .user(user)
                    .push_line_safe(".")
                    .build();

                if let Err(e) = server.1.log_channel.say(ctx, warning_message).await {
                    error!("There was an issue sending the warning message: {}", e);
                }

                Vec::new()
            }
        }
    }
}
