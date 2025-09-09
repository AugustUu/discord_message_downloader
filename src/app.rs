use eframe::egui::{self, Color32, Id, Modal, RichText, Vec2};
use egui_extras::{Column, TableBuilder};
use egui_notify::Toasts;
use serenity_self::{
    Client,
    all::{ChannelId, ChannelType, GuildChannel, GuildInfo, Message, PrivateChannel},
};
use std::{
    fs::File,
    io::Write,
    sync::{
        Arc,
        mpsc::{Receiver, Sender},
    },
};

use crate::discord::{self, DiscordResponse};

pub struct App {
    // Userdata
    token: String,
    username: String,

    // Discord Data
    client: Option<Arc<Client>>,
    guild_list: Option<Vec<GuildInfo>>,
    dm_list: Option<Vec<PrivateChannel>>,
    channel_info: Option<Vec<GuildChannel>>,
    logged_messages: Vec<Message>,

    // Async Operations
    message_download_task: Option<tokio::task::JoinHandle<()>>,
    tx: Sender<DiscordResponse>,
    rx: Receiver<DiscordResponse>,

    // UI State
    selected_channel: Option<(ChannelId, String)>,
    server_category: ServerCategory,
    toasts: Toasts,
}

#[derive(PartialEq, Debug)]
enum ServerCategory {
    Mesages,
    Guilds,
}

impl Default for App {
    fn default() -> Self {
        let (tx, rx) = std::sync::mpsc::channel();

        Self {
            tx,
            rx,
            token: String::new(),
            client: None,
            toasts: Toasts::default().with_anchor(egui_notify::Anchor::BottomRight).reverse(true),
            username: String::new(),
            guild_list: None,
            dm_list: None,
            channel_info: None,
            selected_channel: None,
            logged_messages: Vec::new(),
            message_download_task: None,
            server_category: ServerCategory::Guilds,
        }
    }
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        //cc.egui_ctx.set_pixels_per_point(2.0);

        Self::setup_fonts(cc);
        Default::default()
    }

    fn setup_fonts(cc: &eframe::CreationContext<'_>) {
        let mut fonts = egui::FontDefinitions::default();
        fonts
            .font_data
            .insert("Unifont".to_owned(), egui::FontData::from_static(include_bytes!("../assets/UnifontExMono.ttf")).into());
        fonts.families.entry(egui::FontFamily::Proportional).or_default().insert(0, "Unifont".to_owned());

        cc.egui_ctx.set_fonts(fonts);
    }
}

impl App {
    fn handle_discord_responses(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                DiscordResponse::SuccessfulLogin((client, user)) => {
                    self.client = Some(Arc::new(client));
                    self.username = user.name.clone();
                    self.toasts.success(format!("Logged In as: {}", self.username));

                    if let Some(client) = &self.client {
                        discord::get_guild_list(client.clone(), self.tx.clone());
                        discord::get_message_list(client.clone(), self.tx.clone());
                    }
                }
                DiscordResponse::GuildList(guild_list) => {
                    self.toasts.success(format!("Loaded: {} Servers", guild_list.len()));

                    self.guild_list = Some(guild_list);
                }
                DiscordResponse::DmList(private_channels) => {
                    self.toasts.success(format!("Loaded: {} DMs", private_channels.len()));

                    self.dm_list = Some(private_channels);
                }
                DiscordResponse::ChannelList(guild_channels) => {
                    self.toasts.success(format!("Loaded: {} Channels", guild_channels.len()));
                    self.channel_info = Some(guild_channels);
                }
                DiscordResponse::GotMessage(message) => {
                    self.logged_messages.push(*message);
                }
                DiscordResponse::DoneGettingMessages() => {
                    self.message_download_task = None;

                    self.toasts.success(format!("Finished Downloading: {} Mesages", self.logged_messages.len()));
                }
                DiscordResponse::Error(msg) => {
                    self.toasts.error(format!("Error: {}", msg));
                }
            }
        }
    }

    fn render_top_panel(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if self.client.is_none() {
                    if ui.button(RichText::new("Connect").heading().strong()).clicked() {
                        discord::create_client(self.token.clone(), self.tx.clone());
                    }
                    ui.label(RichText::new("Your Discord Token: ").heading().strong());

                    ui.text_edit_singleline(&mut self.token);
                } else {
                    if ui.button("Disconnect").clicked() {
                        self.client = None
                    }
                    ui.label("Logged in as: ");
                    ui.label(RichText::new(&self.username).color(Color32::from_rgb(37, 150, 190)));
                }
            });
        });
    }

    fn render_server_list(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("server_list").default_width(200.0).show(ctx, |ui| {
            egui::ComboBox::from_label("")
                .selected_text(RichText::new(format!("{:?}", self.server_category)).heading().strong())
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.server_category, ServerCategory::Guilds, "Guilds");
                    ui.selectable_value(&mut self.server_category, ServerCategory::Mesages, "Messages");
                });

            ui.separator();

            if self.server_category == ServerCategory::Guilds {
                if let Some(guilds) = &self.guild_list {
                    egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                        for guild in guilds {
                            if ui.add(egui::Button::new(&guild.name).truncate().min_size(Vec2::new(100.0, 0.0))).clicked() {
                                discord::get_channel_list(self.client.clone().unwrap(), self.tx.clone(), guild.id);
                            };
                        }
                    });
                }
            } else if let Some(dms) = &self.dm_list {
                egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                    for dm in dms {
                        let mut name = dm.recipient.name.clone();
                        if dm.kind == ChannelType::GroupDm {
                            name = format!("Groupchat with {}", dm.recipient.name)
                        }

                        if ui.add(egui::Button::new(name).truncate().min_size(Vec2::new(100.0, 0.0))).clicked() {
                            self.selected_channel = Some((dm.id, dm.name()));
                        };
                    }
                });
            }
        });
    }

    fn render_server_channels(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("channel_list").default_width(400.0).show(ctx, |ui| {
            ui.label(RichText::new("Channels").heading().strong());
            ui.separator();
            if let Some(channels) = &self.channel_info {
                egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                    for channel in channels {
                        if channel.kind == ChannelType::Category {
                            ui.heading(&channel.name);
                        }

                        if channel.is_text_based() {
                            ui.horizontal(|ui| {
                                ui.add_space(20.0);

                                if ui.add(egui::Button::new(&channel.name).truncate().min_size(Vec2::new(100.0, 0.0))).clicked() {
                                    self.selected_channel = Some((channel.id, channel.name.clone()))
                                };
                            });
                        }
                    }
                });
            }
        });
    }

    fn render_download_modal(&mut self, ctx: &egui::Context) {
        if let Some((channel, channel_name)) = self.selected_channel.clone() {
            let modal = Modal::new(Id::new("Download Modal")).show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading(format!("Download: {}", channel_name));
                    ui.spacing();
                    if self.message_download_task.is_none() {
                        if ui.button("Start").clicked() {
                            self.message_download_task = Some(discord::get_channel_messages(self.client.clone().unwrap(), self.tx.clone(), channel));
                        }
                    } else {
                        if ui.button("Cancel").clicked()
                            && let Some(task) = &self.message_download_task
                        {
                            task.abort();
                            self.message_download_task = None;
                            self.toasts.success(format!("Canceled Download With: {} Mesages", self.logged_messages.len()));
                        }
                        ui.label("Downloaded: ");
                        ui.label(RichText::new(self.logged_messages.len().to_string()).color(Color32::from_rgb(37, 150, 190)));
                    }
                    ui.spacing();

                    if !self.logged_messages.is_empty() && ui.button("Save Plaintext").clicked() {
                        self.save_messages_plain_text(&channel_name);
                    }

                    if !self.logged_messages.is_empty() && ui.button("Save Verbose").clicked() {
                        self.save_messages_verbose(&channel_name);
                    }
                });

                ui.separator();

                let table = TableBuilder::new(ui)
                    .striped(true)
                    .resizable(true)
                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                    .column(Column::auto().clip(true))
                    .column(Column::auto())
                    .column(Column::remainder());

                table
                    .header(20.0, |mut header| {
                        header.col(|ui| {
                            ui.strong("Time");
                        });
                        header.col(|ui| {
                            ui.strong("Name");
                        });
                        header.col(|ui| {
                            ui.strong("Content");
                        });
                    })
                    .body(|body| {
                        body.rows(30.0, self.logged_messages.len(), |mut row| {
                            let message = self.logged_messages.get(row.index()).unwrap();

                            row.col(|ui| {
                                ui.label(message.timestamp.to_string());
                            });
                            row.col(|ui| {
                                ui.label(&message.author.name);
                            });
                            row.col(|ui| {
                                ui.add(egui::Label::new(&message.content).truncate());
                            });
                        });
                    });
            });
            if modal.should_close() {
                self.selected_channel = None;
                self.logged_messages.clear();
                if let Some(task) = &self.message_download_task {
                    task.abort();
                }
                self.message_download_task = None
            }
        }
    }

    fn save_messages_plain_text(&mut self, channel_name: &String) {
        let path = std::env::current_dir().unwrap();

        let res = rfd::FileDialog::new()
            .set_file_name(format!("{} Messages.txt", channel_name))
            .set_directory(&path)
            .save_file();

        if let Some(save_path) = res {
            let mut out: String = String::new();

            for message in &self.logged_messages {
                out.push_str("᎗᎗᎗᎗᎗᎗᎗᎗᎗᎗᎗᎗᎗᎗᎗᎗᎗᎗᎗᎗᎗᎗᎗\n");
                out.push_str(&format!("{}: {} ({})", message.author.name, message.content, message.timestamp));
                out.push_str("\n᎗᎗᎗᎗᎗᎗᎗᎗᎗᎗᎗᎗᎗᎗᎗᎗᎗᎗᎗᎗᎗᎗᎗\n");
            }

            if let Ok(mut file) = File::create(save_path) {
                if file.write(out.as_bytes()).is_ok() {
                    self.toasts.success("Saved");
                } else {
                    self.toasts.error("Error Saving");
                }
            } else {
                self.toasts.error("Error Saving");
            }
        }
    }

    fn save_messages_verbose(&mut self, channel_name: &String) {
        let path = std::env::current_dir().unwrap();

        let res = rfd::FileDialog::new()
            .set_file_name(format!("{} Messages Verbose.txt", channel_name))
            .set_directory(&path)
            .save_file();

        if let Some(save_path) = res {
            if let Ok(mut file) = File::create(save_path) {
                if file.write(format!("{:?}", self.logged_messages).as_bytes()).is_ok() {
                    self.toasts.success("Saved");
                } else {
                    self.toasts.error("Error Saving");
                }
            } else {
                self.toasts.error("Error Saving");
            }
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_discord_responses();

        self.render_top_panel(ctx);

        //logged in ui
        if self.client.is_some() {
            self.render_server_list(ctx);

            if self.server_category == ServerCategory::Guilds {
                self.render_server_channels(ctx);
            }

            if self.selected_channel.is_some() {
                self.render_download_modal(ctx);
            }
        }

        self.toasts.show(ctx);

        egui::CentralPanel::default().show(ctx, |_ui| {});
    }
}
