use std::str::FromStr;

use chrono::{Days, Local};
use serenity::{
    all::{
        ButtonStyle, Channel, ComponentInteractionDataKind, Context, CreateActionRow,
        CreateAttachment, CreateButton, CreateEmbed, CreateEmbedAuthor, CreateInteractionResponse,
        CreateInteractionResponseMessage, CreateMessage, EditInteractionResponse, EditMember,
        EmbedMessageBuilding, EventHandler, GetMessages, GuildId, Interaction, MESSAGE_CODE_LIMIT,
        Message, MessageBuilder, Ready, Timestamp, UserId,
    },
    async_trait,
};
use strsim::jaro_winkler;
use tracing::{error, info, instrument, warn};

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

                                let author = msg.member(&ctx).await;
                                if let Err(e) = author {
                                    warn!(
                                        "There was an issue retrieving the message author member; could this be a DM? - {}",
                                        e
                                    );
                                } else if author.unwrap().roles.contains(&serv.mod_role) {
                                    info!(
                                        "Message was posted by moderator {}; ignoring...",
                                        msg.author.id
                                    );
                                    return;
                                }

                                let mut mod_log = CreateMessage::new();

                                let mut log_message = MessageBuilder::new();

                                if serv.warn_mods {
                                    log_message.mention(&serv.mod_role);
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
                                        CreateButton::new(format!(
                                            "{}:{}",
                                            Self::ACCEPT_BUTTON_ID,
                                            msg.author.id
                                        ))
                                        .label("Perform Mod Actions")
                                        .style(ButtonStyle::Danger),
                                        CreateButton::new(Self::REJECT_BUTTON_ID)
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
                                    Self::apply_mod_actions(
                                        &ctx,
                                        msg.author.id,
                                        &msg.content,
                                        log_message.push_line_safe("").push_line_safe(""),
                                        (serv_id, serv),
                                    )
                                    .await;

                                    let log_content = log_message.build();

                                    if log_content.chars().count() > MESSAGE_CODE_LIMIT {
                                        // Log message is too big, send as file instead
                                        mod_log = mod_log.add_file(CreateAttachment::bytes(
                                            log_content.into_bytes(),
                                            "mod_log.txt",
                                        ).description("Actions were taken, please check the file for the logs."));
                                    } else {
                                        mod_log = mod_log.content(log_content);
                                    }

                                    if let Err(e) =
                                        serv.log_channel.send_message(&ctx, mod_log).await
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

                    if msg
                        .channel(&ctx)
                        .await
                        .is_ok_and(|chan| matches!(chan, Channel::Private(_)))
                    {
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
            }
            None => {
                error!(
                    "There was an issue loading the bot's configuration. Please check if the configuration file is available and contains all the necessary configuration options."
                );
            }
        }
    }

    #[instrument]
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::Component(inter) = interaction {
            if let Err(e) = inter.defer(&ctx).await {
                error!(
                    "There was an issue deferring the interaction {:#?}: {}",
                    inter, e
                );
                return;
            }

            let self_id = match ctx.http.application_id() {
                Some(id) => id,
                None => {
                    error!(
                        "Error obtaining the application ID for this bot; the Discord application may be misconfigured or there may be a connectivity issue."
                    );
                    return;
                }
            };

            if inter.application_id != self_id {
                // Interaction doesn't belong to our application, leave it for other bots to deal with
                info!(
                    "Interaction {:#?} does not belong to this application, pass it on",
                    inter
                );

                return;
            }

            let ctx_data = ctx.data.read();

            match ctx_data.await.get::<Config>() {
                Some(cfg) => {
                    if inter.guild_id.is_none()
                        || !cfg.servers.contains_key(&inter.guild_id.unwrap())
                    {
                        warn!(
                            "Somehow, an interaction was spawned for this application which doesn't belong to a configured server; please check if the bot is configured properly; there may be outdated configuration or a server which was once configured and no longer is: {:#?}",
                            inter
                        );

                        return;
                    }

                    let inter_serv = inter.guild_id.unwrap();

                    let serv_cfg = cfg.servers.get(&inter_serv).unwrap();

                    if inter.channel_id != serv_cfg.log_channel {
                        warn!(
                            "Somehow, an interaction was spawned for this application in server {} outside of its configured log channel (configured channel: {}; actual channel: {})",
                            inter_serv, serv_cfg.log_channel, inter.channel_id
                        );

                        return;
                    }

                    if inter
                        .member
                        .as_ref()
                        .unwrap()
                        .roles
                        .contains(&serv_cfg.mod_role)
                    {
                        match inter.data.kind {
                            ComponentInteractionDataKind::Button => {
                                match inter.data.custom_id.as_str() {
                                    Self::REJECT_BUTTON_ID => {
                                        let response_mesg = MessageBuilder::new()
                                            .push(&inter.message.content)
                                            .push_line_safe("")
                                            .push_line_safe("")
                                            .push_italic_line_safe("No action was taken.")
                                            .build();

                                        let mod_actions = CreateActionRow::Buttons(vec![
                                            CreateButton::new(Self::ACCEPT_BUTTON_ID)
                                                .label("Perform Mod Actions")
                                                .style(ButtonStyle::Danger)
                                                .disabled(true),
                                            CreateButton::new(Self::REJECT_BUTTON_ID)
                                                .label("Dismiss")
                                                .style(ButtonStyle::Secondary)
                                                .disabled(true),
                                        ]);

                                        if let Err(e) = inter
                                            .edit_response(
                                                &ctx,
                                                EditInteractionResponse::new()
                                                    .content(&response_mesg)
                                                    .components(vec![mod_actions]),
                                            )
                                            .await
                                        {
                                            error!(
                                                "There was an issue editing the log message to acknowledge the dismissal: {}",
                                                e
                                            );

                                            if let Err(e) = inter
                                                .create_response(
                                                    &ctx,
                                                    CreateInteractionResponse::Message(
                                                        CreateInteractionResponseMessage::new()
                                                            .content(response_mesg),
                                                    ),
                                                )
                                                .await
                                            {
                                                error!(
                                                    "There was an issue sending the dismissal message as a response: {}",
                                                    e
                                                );
                                            }
                                        }
                                    }
                                    but_id if but_id.starts_with(Self::ACCEPT_BUTTON_ID) => {
                                        let target_user_id = match UserId::from_str(
                                            &but_id[Self::ACCEPT_BUTTON_ID.len()..],
                                        ) {
                                            Ok(id) => id,
                                            Err(e) => {
                                                error!(
                                                    "There was an issue retrieving the original user's ID from the button ID {}: {}",
                                                    but_id, e
                                                );

                                                if let Err(e) = inter
                                                    .create_response(
                                                        &ctx,
                                                        CreateInteractionResponse::Message(
                                                            CreateInteractionResponseMessage::new()
                                                                .content("⚠️ I was unable to retrieve the original user's ID; Please check if the original message is still present and try again."),
                                                        ),
                                                    )
                                                    .await
                                                {
                                                    error!("There was an issue sending the user ID failure warning message: {}", e);
                                                }

                                                return;
                                            }
                                        };

                                        let original_msg_content = match inter.message.embeds[0]
                                            .description
                                        {
                                            Some(ref msg) => msg,
                                            None => {
                                                error!(
                                                    "There was an issue retrieving the original message's content: {:#?}",
                                                    inter.message
                                                );

                                                if let Err(e) = inter
                                                    .create_response(
                                                        &ctx,
                                                        CreateInteractionResponse::Message(CreateInteractionResponseMessage::new()
                                                            .content("⚠️ I was unable to retrieve the original message caught by the honeypot; Please check if the original message is still present and try again.")
                                                        )
                                                    )
                                                    .await {
                                                    error!("There was an issue sending the message retrieval failure warning: {}", e);
                                                }

                                                return;
                                            }
                                        };

                                        let mut response_mesg = MessageBuilder::new();
                                        response_mesg
                                            .push(&inter.message.content)
                                            .push_line_safe("")
                                            .push_line_safe("");

                                        Self::apply_mod_actions(
                                            &ctx,
                                            target_user_id,
                                            original_msg_content,
                                            &mut response_mesg,
                                            (inter_serv, serv_cfg),
                                        )
                                        .await;

                                        let mod_actions = CreateActionRow::Buttons(vec![
                                            CreateButton::new(Self::ACCEPT_BUTTON_ID)
                                                .label("Perform Mod Actions")
                                                .style(ButtonStyle::Danger)
                                                .disabled(true),
                                            CreateButton::new(Self::REJECT_BUTTON_ID)
                                                .label("Dismiss")
                                                .style(ButtonStyle::Secondary)
                                                .disabled(true),
                                        ]);

                                        let response_content = response_mesg.build();

                                        let mut edit_response = EditInteractionResponse::new();

                                        if response_content.chars().count() > MESSAGE_CODE_LIMIT {
                                            // Log message is too big, send as file instead
                                            edit_response = edit_response.new_attachment(CreateAttachment::bytes(
                                                response_content.clone().into_bytes(),
                                                "mod_log.txt"
                                            ).description("Actions were taken, please check the file for the logs."));
                                        } else {
                                            edit_response =
                                                edit_response.content(&response_content);
                                        }

                                        if let Err(e) = inter
                                            .edit_response(
                                                &ctx,
                                                edit_response.components(vec![mod_actions]),
                                            )
                                            .await
                                        {
                                            error!(
                                                "There was an issue editing the log message to acknowledge the actions taken: {}",
                                                e
                                            );

                                            let mut fallback_response =
                                                CreateInteractionResponseMessage::new();

                                            if response_content.chars().count() > MESSAGE_CODE_LIMIT
                                            {
                                                fallback_response = fallback_response.add_file(CreateAttachment::bytes(
                                                    response_content.into_bytes(),
                                                    "mod_log.txt"
                                                ).description("Actions were taken, please check the file for the logs."));
                                            } else {
                                                fallback_response =
                                                    fallback_response.content(response_content);
                                            }

                                            if let Err(e) = inter
                                                .create_response(
                                                    &ctx,
                                                    CreateInteractionResponse::Message(
                                                        fallback_response,
                                                    ),
                                                )
                                                .await
                                            {
                                                error!(
                                                    "There was an issue sending the fallback message to acknowledge the actions taken: {}",
                                                    e
                                                );
                                            }
                                        }
                                    }
                                    unknown_id => {
                                        warn!(
                                            "A button registered with an unknown ID was clicked; the bot or Discord application may be misconfigured or outdated: {}",
                                            unknown_id
                                        );
                                    }
                                }
                            }
                            _ => {
                                // WTF
                                warn!(
                                    "Somehow, an interaction was spawned with the correct app ID but the wrong kind of component... I don't even know... {:#?}",
                                    inter.data
                                );
                                return;
                            }
                        }
                    } else {
                        // User doesn't have the right permissions to use this interaction
                        warn!(
                            "User {} tried to use the bot without the proper permissions; please check if this was a mistake.",
                            inter.user.id
                        );

                        let forbidden_response = CreateInteractionResponseMessage::new().content(
                            MessageBuilder::new()
                                .push_safe("🚫 You don't have permission to use this bot's functionality; Please contact ")
                                .role(serv_cfg.mod_role)
                                .push_line_safe("if you believe this is a mistake.")
                                .build()
                        ).ephemeral(true);

                        if let Err(e) = inter
                            .create_response(
                                &ctx,
                                CreateInteractionResponse::Message(forbidden_response),
                            )
                            .await
                        {
                            error!(
                                "There was an issue sending the forbidden message to the user {}: {}",
                                inter.user.id, e
                            );
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
                            if !serv_roles.contains_key(&server_config.mod_role) {
                                // Role doesn't exist in this server
                                warn!(
                                    "The provided role ({}) does not correspond to any roles available on the server {}",
                                    &server_config.mod_role, server_id
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

                    if server_config.warn_mods {
                        hello_message
                            .role(server_config.mod_role)
                            .push_line_safe(" will be warned when the honeypot is triggered.");
                    }

                    hello_message.push_safe("Offending users will be ");

                    match server_config.mod_actions {
                        ModerationActions::Mute => hello_message.push_bold_safe("timed out"),
                        ModerationActions::Kick => hello_message.push_bold_safe("kicked"),
                        ModerationActions::Ban => hello_message.push_bold_safe("banned"),
                    };

                    if server_config.erase_messages
                        || matches!(server_config.mod_actions, ModerationActions::Ban)
                    {
                        hello_message.push_line_safe(" and their spam messages will be deleted.");
                    } else {
                        hello_message.push_line_safe(".");
                    }

                    match server_config
                        .log_channel
                        .say(&ctx, hello_message.build())
                        .await
                    {
                        Ok(msg) => {
                            if let Err(e) = msg.pin(&ctx).await {
                                error!("There was an issue pinning the welcome message: {}", e);
                            }
                        }
                        Err(e) => {
                            error!("There was an issue sending the welcome message: {}", e);
                        }
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
    const ACCEPT_BUTTON_ID: &str = "honeypot-bot:mod_act";
    const REJECT_BUTTON_ID: &str = "honeypot-bot:mod_dismiss";
    const AUDIT_MESSAGE: &str = "Caught spamming by the honeypot; the account may be compromised.";

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
                        match chan.messages(ctx, GetMessages::new().limit(10)).await {
                            Ok(mesgs) => {
                                matched_mesgs.extend(mesgs.into_iter().filter(|mesg| {
                                    mesg.author.id == user
                                        && jaro_winkler(&mesg.content_safe(ctx), content) >= 0.75
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

    #[instrument]
    async fn apply_mod_actions(
        ctx: &Context,
        target_user: UserId,
        msg_content: &str,
        log_message: &mut MessageBuilder,
        server: (GuildId, &ServerConfig),
    ) {
        match server.1.mod_actions {
            ModerationActions::Mute => {
                // 1-day mute hardcoded for now
                let duration = Local::now().checked_add_days(Days::new(1));

                match duration {
                    Some(time) => {
                        if let Err(e) = server
                            .0
                            .edit_member(
                                ctx,
                                target_user,
                                EditMember::new()
                                    .disable_communication_until_datetime(Timestamp::from(time))
                                    .audit_log_reason(Self::AUDIT_MESSAGE),
                            )
                            .await
                        {
                            error!(
                                "There was an issue timing the user {} out: {}",
                                target_user, e
                            );

                            log_message
                                .push("⚠️ I was unable to time the user ")
                                .user(target_user)
                                .push_line_safe(
                                    " out; Please make sure the compromised account is dealt with.",
                                );
                        } else {
                            log_message
                                .push_line_safe("")
                                .push_italic_safe("The user was timed out for 1 day");
                        }
                    }
                    None => {
                        error!(
                            "There was an issue timing the user {} out: Timeout duration returned None.",
                            target_user
                        );

                        log_message
                            .push("⚠️ I was unable to time the user ")
                            .user(target_user)
                            .push_line_safe(
                                " out; Please make sure the compromised account is dealt with.",
                            );
                    }
                }
            }
            ModerationActions::Kick => {
                if let Err(e) = server
                    .0
                    .kick_with_reason(ctx, target_user, Self::AUDIT_MESSAGE)
                    .await
                {
                    error!("There was an issue kicking the user {}: {}", target_user, e);

                    log_message
                        .push("⚠️ I was unable to kick the user ")
                        .user(target_user)
                        .push_line_safe(
                            "; Please make sure the compromised account is dealt with.",
                        );
                } else {
                    log_message
                        .push_line_safe("")
                        .push_italic_safe("The user was kicked");
                }
            }
            ModerationActions::Ban => {
                if let Err(e) = server
                    .0
                    .ban_with_reason(ctx, target_user, 1, Self::AUDIT_MESSAGE)
                    .await
                {
                    error!("There was an issue banning the user {}: {}", target_user, e);

                    log_message
                        .push("⚠️ I was unable to ban the user ")
                        .user(target_user)
                        .push_line_safe(
                            "; Please make sure the compromised account is dealt with.",
                        );
                } else {
                    log_message
                        .push_line_safe("")
                        .push_italic_safe("The user was banned");
                }
            }
        }

        if server.1.erase_messages && !matches!(server.1.mod_actions, ModerationActions::Ban) {
            let identical_messages =
                Self::search_for_spam_messages(ctx, target_user, msg_content, server).await;

            let mut deleted_count = 0usize;

            for ident_msg in identical_messages {
                if let Err(e) = ident_msg.delete(ctx).await {
                    error!(
                        "Unable to delete message {} in channel {}: {}",
                        ident_msg.id, ident_msg.channel_id, e
                    );

                    log_message
                        .push("⚠️ I was unable to delete the message in channel ")
                        .channel(ident_msg.channel_id)
                        .push_line_safe("; Please make sure any spam messages are deleted.");
                } else {
                    deleted_count += 1;
                }
            }

            log_message.push_italic_line_safe(format!(
                " and {deleted_count} repeated (spam) messages were deleted."
            ));
        } else {
            log_message.push_italic_line_safe(".");
        }
    }
}
