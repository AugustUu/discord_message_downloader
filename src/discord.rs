use serenity_self::all::{ChannelId, CurrentUser, GuildChannel, GuildId, GuildInfo, Message, PrivateChannel, validate_token};
use serenity_self::futures::StreamExt;
use serenity_self::prelude::*;
use std::sync::{Arc, mpsc::Sender};

pub enum DiscordResponse {
    SuccessfulLogin((Client, CurrentUser)),
    GuildList(Vec<GuildInfo>),
    DmList(Vec<PrivateChannel>),
    ChannelList(Vec<GuildChannel>),
    GotMessage(Box<Message>),
    DoneGettingMessages(),
    Error(String),
}

pub fn create_client(token: String, tx: Sender<DiscordResponse>) {
    tokio::spawn(async move {
        if validate_token(&token).is_err() {
            let _ = tx.send(DiscordResponse::Error("Invalid Token".to_string()));
            return;
        }

        if let Ok(client) = Client::builder(token, GatewayIntents::default()).await {
            if let Ok(user) = client.http.get_current_user().await {
                let _ = tx.send(DiscordResponse::SuccessfulLogin((client, user)));
            } else {
                let _ = tx.send(DiscordResponse::Error("Cant Login".to_string()));
            }
        } else {
            let _ = tx.send(DiscordResponse::Error("Cant Login".to_string()));
        }
    });
}

pub fn get_guild_list(client: Arc<Client>, tx: Sender<DiscordResponse>) {
    tokio::spawn(async move {
        if let Ok(guilds) = client.http.get_guilds(None, None).await {
            let _ = tx.send(DiscordResponse::GuildList(guilds));
        }
    });
}

pub fn get_message_list(client: Arc<Client>, tx: Sender<DiscordResponse>) {
    tokio::spawn(async move {
        if let Ok(guilds) = client.http.get_user_dm_channels().await {
            let _ = tx.send(DiscordResponse::DmList(guilds));
        }
    });
}

pub fn get_channel_list(client: Arc<Client>, tx: Sender<DiscordResponse>, guild_id: GuildId) {
    tokio::spawn(async move {
        if let Ok(channels) = client.http.get_channels(guild_id).await {
            let _ = tx.send(DiscordResponse::ChannelList(channels));
        }
    });
}

pub fn get_channel_messages(client: Arc<Client>, tx: Sender<DiscordResponse>, channel: ChannelId) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut messages = channel.messages_iter(&client.http).boxed();

        while let Some(message_result) = messages.next().await {
            match message_result {
                Ok(message) => {
                    let _ = tx.send(DiscordResponse::GotMessage(Box::new(message)));
                }
                Err(error) => {
                    let _ = tx.send(DiscordResponse::Error(error.to_string()));
                    return;
                }
            }
        }
        let _ = tx.send(DiscordResponse::DoneGettingMessages());
    })
}
