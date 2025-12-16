use gpui::*;
use std::sync::Arc;
use crate::scanner::VideoScanner;
use crate::downloader_queue::DownloadQueue;
use serde::{Deserialize, Serialize};

mod text_input;
use text_input::TextInputView;

pub struct NDownloadApp {
    url_input: Entity<TextInputView>,
    channels: Vec<Channel>,
    selected_channel: Option<usize>,
    videos: Vec<VideoInfo>,
    scanner: Arc<VideoScanner>,
    download_queue: Arc<DownloadQueue>,
    loading: bool,
    scroll_handle: ScrollHandle,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Channel {
    name: String,
    platform: Platform,
    url: String,
}

#[derive(Clone, Debug)]
struct VideoInfo {
    id: String,
    title: String,
    url: String,
    upload_date: Option<String>,
    downloaded: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
enum Platform {
    YouTube,
    Twitch,
}

impl Platform {
    fn from_url(url: &str) -> Option<Self> {
        if url.contains("youtube.com") || url.contains("youtu.be") {
            Some(Platform::YouTube)
        } else if url.contains("twitch.tv") {
            Some(Platform::Twitch)
        } else {
            None
        }
    }

    fn extract_channel_name(url: &str) -> Option<String> {
        // Pour YouTube: youtube.com/@channel ou youtube.com/c/channel
        if url.contains("youtube.com") {
            if let Some(idx) = url.find("/@") {
                let rest = &url[idx + 2..];
                return Some(rest.split('/').next()?.to_string());
            } else if let Some(idx) = url.find("/c/") {
                let rest = &url[idx + 3..];
                return Some(rest.split('/').next()?.to_string());
            } else if let Some(idx) = url.find("/channel/") {
                let rest = &url[idx + 9..];
                return Some(rest.split('/').next()?.to_string());
            }
        }

        // Pour Twitch: twitch.tv/channel
        if url.contains("twitch.tv/") {
            if let Some(idx) = url.find("twitch.tv/") {
                let rest = &url[idx + 10..];
                let channel = rest.split('/').next()?;
                if !channel.is_empty() && channel != "videos" {
                    return Some(channel.to_string());
                }
            }
        }

        None
    }
}

fn format_date(date_str: &str) -> String {
    // Format YYYYMMDD -> DD/MM/YYYY
    if date_str.len() == 8 {
        let year = &date_str[0..4];
        let month = &date_str[4..6];
        let day = &date_str[6..8];
        format!("{}/{}/{}", day, month, year)
    } else {
        date_str.to_string()
    }
}

const CHANNELS_CACHE_FILE: &str = "/tmp/ndownload_channels.json";

fn load_channels() -> Vec<Channel> {
    match std::fs::read_to_string(CHANNELS_CACHE_FILE) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

fn save_channels(channels: &[Channel]) {
    if let Ok(content) = serde_json::to_string_pretty(channels) {
        let _ = std::fs::write(CHANNELS_CACHE_FILE, content);
    }
}

impl NDownloadApp {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let url_input = cx.new(|cx| {
            TextInputView::new(cx)
                .placeholder("Collez un lien YouTube ou Twitch...")
                .on_enter(move |_text| {
                    // L'action sera gérée directement par handle_add_channel
                })
        });

        Self {
            url_input,
            channels: load_channels(),
            selected_channel: None,
            videos: Vec::new(),
            scanner: Arc::new(VideoScanner::new()),
            download_queue: Arc::new(DownloadQueue::new(cx)),
            loading: false,
            scroll_handle: ScrollHandle::new(),
        }
    }

    fn add_channel_from_url(&mut self, url: String) {
        if let Some(platform) = Platform::from_url(&url) {
            if let Some(name) = Platform::extract_channel_name(&url) {
                // Éviter les doublons
                if !self.channels.iter().any(|c| c.url == url) {
                    self.channels.push(Channel {
                        name,
                        platform,
                        url,
                    });
                    save_channels(&self.channels);
                }
            }
        }
    }

    fn handle_add_channel(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let url = self.url_input.read(cx).value();
        if !url.trim().is_empty() {
            self.add_channel_from_url(url);
            // Clear the input
            self.url_input.update(cx, |input, _cx| {
                input.clear();
            });
            cx.notify();
        }
    }

    fn select_channel(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.selected_channel = Some(index);
        self.loading = true;
        self.videos.clear();
        cx.notify();

        let channel_url = self.channels[index].url.clone();
        let channel_name = self.channels[index].name.clone();
        let scanner = self.scanner.clone();

        // Scanner les vidéos en async
        cx.spawn_in(window, async move |this, cx| {
            match scanner.scan_channel_videos(&channel_url).await {
                Ok(metadata_videos) => {
                    let videos: Vec<VideoInfo> = metadata_videos
                        .into_iter()
                        .map(|meta| {
                            let downloaded_path = scanner.is_video_downloaded(&channel_name, meta.duration);
                            VideoInfo {
                                id: meta.id,
                                title: meta.title,
                                url: meta.url,
                                upload_date: meta.upload_date,
                                downloaded: downloaded_path.is_some(),
                            }
                        })
                        .collect();

                    let _ = this.update(cx, |this, cx| {
                        this.videos = videos;
                        this.loading = false;
                        cx.notify();
                    });
                }
                Err(e) => {
                    tracing::error!("Erreur scan vidéos: {}", e);
                    let _ = this.update(cx, |this, cx| {
                        this.loading = false;
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    fn go_back(&mut self, _cx: &mut Context<Self>) {
        self.selected_channel = None;
        self.videos.clear();
    }
}

impl Render for NDownloadApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Si une chaîne est sélectionnée, afficher la liste des vidéos
        if let Some(channel_index) = self.selected_channel {
            return self.render_video_list(channel_index, cx).into_any_element();
        }

        // Sinon, afficher la liste des chaînes
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(0x1e1e1e))
            .gap_4()
            .p_4()
            .child(
                // Header
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_color(rgb(0xffffff))
                            .text_size(px(24.0))
                            .font_weight(FontWeight::BOLD)
                            .child("NDownloader")
                    )
                    .child(
                        div()
                            .text_color(rgb(0xaaaaaa))
                            .text_size(px(14.0))
                            .child("Automatic video downloader for Twitch and YouTube")
                    )
            )
            .child(
                // URL input section
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .p_4()
                    .bg(rgb(0x2d2d2d))
                    .rounded_md()
                    .child(
                        div()
                            .text_color(rgb(0xffffff))
                            .text_size(px(16.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Ajouter une chaîne")
                    )
                    .child(
                        div()
                            .text_color(rgb(0xaaaaaa))
                            .text_size(px(13.0))
                            .child("Collez un lien YouTube ou Twitch (l'app détectera automatiquement la plateforme)")
                    )
                    .child(
                        // URL input and button
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                div()
                                    .flex_1()
                                    .h_10()
                                    .px_3()
                                    .bg(rgb(0x3d3d3d))
                                    .border_1()
                                    .border_color(rgb(0x4d4d4d))
                                    .rounded_md()
                                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                                        if event.keystroke.key == "enter" {
                                            this.handle_add_channel(window, cx);
                                        }
                                    }))
                                    .child(self.url_input.clone())
                            )
                            .child(
                                // Add button
                                div()
                                    .h_10()
                                    .px_6()
                                    .bg(rgb(0x0d96f2))
                                    .rounded_md()
                                    .cursor_pointer()
                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _event, window, cx| {
                                        this.handle_add_channel(window, cx);
                                    }))
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .h_full()
                                            .text_color(rgb(0xffffff))
                                            .text_size(px(14.0))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child("Ajouter")
                                    )
                            )
                    )
            )
            .child(
                // Channels list section
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .gap_2()
                    .p_4()
                    .bg(rgb(0x2d2d2d))
                    .rounded_md()
                    .child(
                        div()
                            .text_color(rgb(0xffffff))
                            .text_size(px(16.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .mb_2()
                            .child(format!("Chaînes surveillées ({})", self.channels.len()))
                    )
                    .child(
                        if self.channels.is_empty() {
                            div()
                                .flex()
                                .items_center()
                                .justify_center()
                                .h_full()
                                .text_color(rgb(0x888888))
                                .text_size(px(14.0))
                                .child("Aucune chaîne ajoutée")
                                .into_any_element()
                        } else {
                            div()
                                .id("channels-list")
                                .flex()
                                .flex_col()
                                .gap_2()
                                .overflow_y_scroll()
                                .track_scroll(&self.scroll_handle)
                                .children(self.channels.iter().enumerate().map(|(index, channel)| {
                                    let platform_color = match channel.platform {
                                        Platform::YouTube => rgb(0xff0000),
                                        Platform::Twitch => rgb(0x9146ff),
                                    };
                                    let platform_name = match channel.platform {
                                        Platform::YouTube => "YouTube",
                                        Platform::Twitch => "Twitch",
                                    };

                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_3()
                                        .p_3()
                                        .bg(rgb(0x3d3d3d))
                                        .rounded_md()
                                        .cursor_pointer()
                                        .hover(|style| style.bg(rgb(0x4d4d4d)))
                                        .on_mouse_down(MouseButton::Left, cx.listener(move |this, _event, window, cx| {
                                            this.select_channel(index, window, cx);
                                            cx.notify();
                                        }))
                                        .child(
                                            div()
                                                .px_2()
                                                .py_1()
                                                .bg(platform_color)
                                                .rounded_sm()
                                                .child(
                                                    div()
                                                        .text_color(rgb(0xffffff))
                                                        .text_size(px(12.0))
                                                        .font_weight(FontWeight::BOLD)
                                                        .child(platform_name)
                                                )
                                        )
                                        .child(
                                            div()
                                                .text_color(rgb(0xffffff))
                                                .text_size(px(14.0))
                                                .child(channel.name.clone())
                                        )
                                }))
                                .into_any_element()
                        }
                    )
            )
            .into_any_element()
    }
}

impl NDownloadApp {
    fn render_video_list(&mut self, channel_index: usize, cx: &mut Context<Self>) -> Div {
        let channel = &self.channels[channel_index];
        let platform_color = match channel.platform {
            Platform::YouTube => rgb(0xff0000),
            Platform::Twitch => rgb(0x9146ff),
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(0x1e1e1e))
            .gap_4()
            .p_4()
            .child(
                // Header avec bouton retour
                div()
                    .flex()
                    .items_center()
                    .gap_4()
                    .child(
                        // Bouton retour
                        div()
                            .px_4()
                            .py_2()
                            .bg(rgb(0x2d2d2d))
                            .rounded_md()
                            .cursor_pointer()
                            .hover(|style| style.bg(rgb(0x3d3d3d)))
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _event, _window, cx| {
                                this.go_back(cx);
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .text_color(rgb(0xffffff))
                                    .text_size(px(14.0))
                                    .child("← Retour")
                            )
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .px_2()
                                    .py_1()
                                    .bg(platform_color)
                                    .rounded_sm()
                                    .child(
                                        div()
                                            .text_color(rgb(0xffffff))
                                            .text_size(px(12.0))
                                            .font_weight(FontWeight::BOLD)
                                            .child(match channel.platform {
                                                Platform::YouTube => "YouTube",
                                                Platform::Twitch => "Twitch",
                                            })
                                    )
                            )
                            .child(
                                div()
                                    .text_color(rgb(0xffffff))
                                    .text_size(px(20.0))
                                    .font_weight(FontWeight::BOLD)
                                    .child(channel.name.clone())
                            )
                    )
            )
            .child(
                // Liste des vidéos
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .gap_2()
                    .p_4()
                    .bg(rgb(0x2d2d2d))
                    .rounded_md()
                    .child(
                        div()
                            .text_color(rgb(0xffffff))
                            .text_size(px(16.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .mb_2()
                            .child(format!("Vidéos disponibles ({})", self.videos.len()))
                    )
                    .child(
                        if self.loading {
                            div()
                                .flex()
                                .items_center()
                                .justify_center()
                                .h_full()
                                .text_color(rgb(0x0d96f2))
                                .text_size(px(14.0))
                                .child("Chargement des vidéos...")
                                .into_any_element()
                        } else if self.videos.is_empty() {
                            div()
                                .flex()
                                .items_center()
                                .justify_center()
                                .h_full()
                                .text_color(rgb(0x888888))
                                .text_size(px(14.0))
                                .child("Aucune vidéo trouvée")
                                .into_any_element()
                        } else {
                            div()
                                .id("videos-list")
                                .flex()
                                .flex_col()
                                .gap_2()
                                .overflow_y_scroll()
                                .track_scroll(&self.scroll_handle)
                                .children(self.videos.iter().map(|video| {
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_3()
                                        .p_3()
                                        .bg(rgb(0x3d3d3d))
                                        .rounded_md()
                                        .cursor_pointer()
                                        .hover(|style| style.bg(rgb(0x4d4d4d)))
                                        .child(
                                            // Indicateur de statut
                                            div()
                                                .w_3()
                                                .h_3()
                                                .rounded_full()
                                                .bg(if video.downloaded {
                                                    rgb(0x10b981) // Vert si téléchargé
                                                } else {
                                                    rgb(0xf59e0b) // Orange si non téléchargé
                                                })
                                        )
                                        .child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .gap_1()
                                                .flex_1()
                                                .child(
                                                    div()
                                                        .text_color(rgb(0xffffff))
                                                        .text_size(px(14.0))
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .child(video.title.clone())
                                                )
                                                .child(
                                                    div()
                                                        .text_color(if video.downloaded {
                                                            rgb(0x10b981)
                                                        } else {
                                                            rgb(0xf59e0b)
                                                        })
                                                        .text_size(px(12.0))
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .child(if video.downloaded {
                                                            "Téléchargé"
                                                        } else {
                                                            "Non téléchargé"
                                                        })
                                                )
                                        )
                                }))
                                .into_any_element()
                        }
                    )
            )
    }
}
