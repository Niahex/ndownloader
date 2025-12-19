use crate::downloader_queue::DownloadQueue;
use crate::notifications::Notification;
use crate::scanner::VideoScanner;
use gpui::prelude::FluentBuilder;
use gpui::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub mod actions;
mod components;
mod text_input;

pub use actions::*;
use components::{ChannelItem, VideoItem};
use text_input::TextInputView;

// Palette Nord
const NORD0: u32 = 0x2e3440; // Polar Night - darkest
const NORD1: u32 = 0x3b4252; // Polar Night
const NORD2: u32 = 0x434c5e; // Polar Night
const NORD3: u32 = 0x4c566a; // Polar Night - lightest
const NORD4: u32 = 0xd8dee9; // Snow Storm - darkest
const NORD6: u32 = 0xeceff4; // Snow Storm - lightest
const NORD8: u32 = 0x88c0d0; // Frost - bright cyan
const NORD9: u32 = 0x81a1c1; // Frost - blue
const NORD10: u32 = 0x5e81ac; // Frost - dark blue
const NORD11: u32 = 0xbf616a; // Aurora - red
const NORD13: u32 = 0xebcb8b; // Aurora - yellow
const NORD14: u32 = 0xa3be8c; // Aurora - green
const NORD15: u32 = 0xb48ead; // Aurora - purple

pub struct NDownloaderApp {
    url_input: Entity<TextInputView>,
    channels: Vec<Channel>,
    selected_channel: Option<usize>,
    videos: Vec<VideoInfo>,
    scanner: Arc<VideoScanner>,
    download_queue: Arc<DownloadQueue>,
    loading: bool,
    download_input: Option<Entity<TextInputView>>,
    download_video: Option<DownloadingVideo>,
    downloading_videos: std::collections::HashSet<String>, // URLs des vidéos en cours de téléchargement
}

#[derive(Clone)]
struct DownloadingVideo {
    url: String,
    channel_name: String,
    progress: f32, // 0.0 to 1.0
    speed: Option<String>,
    eta: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Channel {
    name: String,
    platform: Platform,
    url: String,
}

#[derive(Clone, Debug)]
struct VideoInfo {
    title: String,
    url: String,
    status: VideoStatus,
}

#[derive(Clone, Debug, PartialEq)]
enum VideoStatus {
    NotDownloaded,
    Downloading,
    Downloaded,
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

const CHANNELS_CACHE_FILE: &str = "/tmp/ndownloader_channels.json";

fn load_channels() -> Vec<Channel> {
    match std::fs::read_to_string(CHANNELS_CACHE_FILE) {
        Ok(content) => match serde_json::from_str(&content) {
            Ok(channels) => channels,
            Err(error) => {
                tracing::warn!("Failed to parse channels cache: {}", error);
                Vec::new()
            }
        },
        Err(error) => {
            tracing::debug!("No channels cache file found: {}", error);
            Vec::new()
        }
    }
}

fn save_channels(channels: &[Channel]) {
    match serde_json::to_string_pretty(channels) {
        Ok(content) => {
            if let Err(error) = std::fs::write(CHANNELS_CACHE_FILE, content) {
                tracing::error!("Failed to save channels cache: {}", error);
            }
        }
        Err(error) => {
            tracing::error!("Failed to serialize channels: {}", error);
        }
    }
}

impl NDownloaderApp {
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
            download_input: None,
            download_video: None,
            downloading_videos: std::collections::HashSet::new(),
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

        cx.spawn_in(window, async move |this, cx| {
            let videos_result = scanner.scan_channel_videos(&channel_url).await;

            this.update(cx, |this, cx| {
                match videos_result {
                    Ok(metadata_videos) => {
                        this.videos = metadata_videos
                            .into_iter()
                            .map(|meta| {
                                let is_downloaded = scanner
                                    .is_video_downloaded(&channel_name, meta.duration)
                                    .is_some();
                                let is_downloading = this.downloading_videos.contains(&meta.url);

                                let status = if is_downloaded {
                                    VideoStatus::Downloaded
                                } else if is_downloading {
                                    VideoStatus::Downloading
                                } else {
                                    VideoStatus::NotDownloaded
                                };

                                VideoInfo {
                                    title: meta.title,
                                    url: meta.url,
                                    status,
                                }
                            })
                            .collect();
                    }
                    Err(error) => {
                        tracing::error!("Failed to scan channel videos: {}", error);
                    }
                }

                this.loading = false;
                cx.notify();
            })
        })
        .detach();
    }

    fn delete_channel(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.channels.len() {
            self.channels.remove(index);

            // Sauvegarder les changements
            save_channels(&self.channels);

            // Si on était sur cette chaîne, revenir à la liste
            if self.selected_channel == Some(index) {
                self.selected_channel = None;
                self.videos.clear();
            } else if let Some(selected) = self.selected_channel {
                // Ajuster l'index si nécessaire
                if selected > index {
                    self.selected_channel = Some(selected - 1);
                }
            }

            cx.notify();
        }
    }

    fn go_back(&mut self, _: &GoBack, _window: &mut Window, _cx: &mut Context<Self>) {
        self.selected_channel = None;
        self.videos.clear();
    }

    fn handle_quit(&mut self, _: &Quit, _window: &mut Window, cx: &mut Context<Self>) {
        cx.quit();
    }

    fn handle_cancel_download(
        &mut self,
        _: &CancelDownload,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.cancel_download(cx);
    }

    fn start_download(&mut self, video_url: String, channel_name: String, cx: &mut Context<Self>) {
        let input =
            cx.new(|cx| TextInputView::new(cx).placeholder("Nom du fichier (sans extension)..."));
        self.download_input = Some(input);
        self.download_video = Some(DownloadingVideo {
            url: video_url,
            channel_name,
            progress: 0.0,
            speed: None,
            eta: None,
        });
        cx.notify();
    }

    fn cancel_download(&mut self, cx: &mut Context<Self>) {
        self.download_input = None;
        self.download_video = None;
        cx.notify();
    }

    fn confirm_download(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(input) = &self.download_input else {
            return;
        };
        let Some(video) = &self.download_video else {
            return;
        };

        let filename = input.read(cx).value();
        if filename.trim().is_empty() {
            tracing::warn!("Empty filename provided");
            return;
        }

        let channel_name = video.channel_name.clone();
        let video_url = video.url.clone();
        let download_queue = self.download_queue.clone();
        let scanner = self.scanner.clone();

        // Trouver le meilleur disque de stockage
        let storage_path = match scanner.find_best_storage_path() {
            Ok(path) => path,
            Err(error) => {
                tracing::error!("Failed to find storage path: {}", error);
                return;
            }
        };

        let output_path = format!("{}/{}/{}.mp4", storage_path, channel_name, filename.trim());

        // Marquer comme en cours de téléchargement
        self.downloading_videos.insert(video_url.clone());

        // Mettre à jour le statut des vidéos
        for video in &mut self.videos {
            if video.url == video_url {
                video.status = VideoStatus::Downloading;
                break;
            }
        }

        // Lancer le téléchargement
        let output_path_buf = std::path::PathBuf::from(&output_path);
        let filename_clone = filename.trim().to_string();

        // Notification de début
        Notification::info(
            "Téléchargement démarré",
            &format!("Téléchargement de {filename_clone} en cours..."),
        );

        cx.spawn(async move |this, cx| {
            if let Err(error) = download_queue
                .add_download(
                    filename.clone(),
                    video_url.clone(),
                    filename.clone(),
                    output_path_buf.clone(),
                )
                .await
            {
                tracing::error!("Failed to add download: {}", error);
                Notification::error(
                    "Erreur de téléchargement",
                    &format!("Impossible de démarrer le téléchargement: {error}"),
                );

                this.update(cx, |this, cx| {
                    this.downloading_videos.remove(&video_url);
                    for video in &mut this.videos {
                        if video.url == video_url {
                            video.status = VideoStatus::NotDownloaded;
                            break;
                        }
                    }
                    cx.notify();
                })
                .ok();
                return;
            }

            // Polling: attendre que le fichier existe vraiment
            let mut progress = 0.0;
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(2))
                    .await;

                // Simuler la progression (incrémenter jusqu'à 90%)
                if progress < 0.9 {
                    progress += 0.1;
                    this.update(cx, |this, cx| {
                        if let Some(ref mut video) = this.download_video {
                            if video.url == video_url {
                                video.progress = progress;
                                cx.notify();
                            }
                        }
                    })
                    .ok();
                }

                if output_path_buf.exists() {
                    // Fichier existe, téléchargement terminé !
                    this.update(cx, |this, cx| {
                        if let Some(ref mut video) = this.download_video {
                            if video.url == video_url {
                                video.progress = 1.0;
                                cx.notify();
                            }
                        }
                    })
                    .ok();

                    Notification::success(
                        "Téléchargement terminé",
                        &format!("{filename_clone} a été téléchargé avec succès"),
                    );

                    this.update(cx, |this, cx| {
                        this.downloading_videos.remove(&video_url);
                        for video in &mut this.videos {
                            if video.url == video_url {
                                video.status = VideoStatus::Downloaded;
                                break;
                            }
                        }
                        cx.notify();
                    })
                    .ok();
                    break;
                }

                // Timeout après 2 heures (en cas de problème)
                // TODO: améliorer avec une vraie vérification de l'état de la queue
            }
        })
        .detach();

        // Fermer l'overlay
        self.download_input = None;
        self.download_video = None;
        cx.notify();
    }
}

impl Render for NDownloaderApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let main_content = if let Some(channel_index) = self.selected_channel {
            self.render_video_list(channel_index, cx).into_any_element()
        } else {
            self.render_channel_list(cx)
        };

        // Si l'overlay de téléchargement est actif, l'afficher
        if self.download_input.is_some() {
            return self.render_download_overlay(main_content, cx);
        }

        main_content
    }
}

impl NDownloaderApp {
    fn render_channel_list(&mut self, cx: &mut Context<Self>) -> AnyElement {
        // Sinon, afficher la liste des chaînes
        div()
            .on_action(cx.listener(Self::go_back))
            .on_action(cx.listener(Self::handle_quit))
            .on_action(cx.listener(Self::handle_cancel_download))
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(NORD0))
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
                            .text_color(rgb(NORD6))
                            .text_size(px(24.0))
                            .font_weight(FontWeight::BOLD)
                            .child("NDownloader")
                    )
                    .child(
                        div()
                            .text_color(rgb(NORD4))
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
                    .bg(rgb(NORD1))
                    .rounded_md()
                    .child(
                        div()
                            .text_color(rgb(NORD6))
                            .text_size(px(16.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Ajouter une chaîne")
                    )
                    .child(
                        div()
                            .text_color(rgb(NORD4))
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
                                    .bg(rgb(NORD2))
                                    .border_1()
                                    .border_color(rgb(NORD3))
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
                                    .bg(rgb(NORD8))
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
                                            .text_color(rgb(NORD6))
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
                    .bg(rgb(NORD1))
                    .rounded_md()
                    .overflow_hidden()
                    .child(
                        div()
                            .text_color(rgb(NORD6))
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
                                .text_color(rgb(NORD3))
                                .text_size(px(14.0))
                                .child("Aucune chaîne ajoutée")
                                .into_any_element()
                        } else {
                            div()
                                .id("channels-list")
                                .flex()
                                .flex_col()
                                .gap_2()
                                .size_full()
                                .overflow_y_scroll()
                                .children(self.channels.iter().enumerate().map(|(index, channel)| {
                                    div()
                                        .flex()
                                        .p_3()
                                        .bg(rgb(NORD2))
                                        .rounded_md()
                                        .cursor_pointer()
                                        .hover(|style| style.bg(rgb(NORD3)))
                                        .on_mouse_down(MouseButton::Left, cx.listener(move |this, _event, window, cx| {
                                            this.select_channel(index, window, cx);
                                        }))
                                        .child(
                                            div()
                                                .flex_1()
                                                .child(ChannelItem::new(channel.clone()))
                                        )
                                        .child(
                                            div()
                                                .px_2()
                                                .py_1()
                                                .bg(rgb(NORD11))
                                                .rounded_sm()
                                                .cursor_pointer()
                                                .hover(|style| style.bg(rgb(0x8f4149)))
                                                .on_mouse_down(MouseButton::Left, cx.listener(move |this, _event, _window, cx| {
                                                    this.delete_channel(index, cx);
                                                    cx.stop_propagation();
                                                }))
                                                .child(
                                                    div()
                                                        .text_color(rgb(NORD6))
                                                        .text_size(px(12.0))
                                                        .font_weight(FontWeight::BOLD)
                                                        .child("✕")
                                                )
                                        )
                                }))
                                .into_any_element()
                        }
                    )
            )
            .into_any_element()
    }
}

impl NDownloaderApp {
    fn render_video_list(&mut self, channel_index: usize, cx: &mut Context<Self>) -> Div {
        let channel = &self.channels[channel_index];
        let platform_color = match channel.platform {
            Platform::YouTube => rgb(NORD11),
            Platform::Twitch => rgb(NORD15),
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(NORD0))
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
                            .bg(rgb(NORD1))
                            .rounded_md()
                            .cursor_pointer()
                            .hover(|style| style.bg(rgb(NORD3)))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _event, window, cx| {
                                    this.go_back(&GoBack, window, cx);
                                    cx.notify();
                                }),
                            )
                            .child(
                                div()
                                    .text_color(rgb(NORD6))
                                    .text_size(px(14.0))
                                    .child("← Retour"),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div().px_2().py_1().bg(platform_color).rounded_sm().child(
                                    div()
                                        .text_color(rgb(NORD6))
                                        .text_size(px(12.0))
                                        .font_weight(FontWeight::BOLD)
                                        .child(match channel.platform {
                                            Platform::YouTube => "YouTube",
                                            Platform::Twitch => "Twitch",
                                        }),
                                ),
                            )
                            .child(
                                div()
                                    .text_color(rgb(NORD6))
                                    .text_size(px(20.0))
                                    .font_weight(FontWeight::BOLD)
                                    .child(channel.name.clone()),
                            ),
                    ),
            )
            .child(
                // Liste des vidéos
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .gap_2()
                    .p_4()
                    .bg(rgb(NORD1))
                    .rounded_md()
                    .overflow_hidden()
                    .child(
                        div()
                            .text_color(rgb(NORD6))
                            .text_size(px(16.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .mb_2()
                            .child(format!("Vidéos disponibles ({})", self.videos.len())),
                    )
                    .child(if self.loading {
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .h_full()
                            .text_color(rgb(NORD8))
                            .text_size(px(14.0))
                            .child("Chargement des vidéos...")
                            .into_any_element()
                    } else if self.videos.is_empty() {
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .h_full()
                            .text_color(rgb(NORD3))
                            .text_size(px(14.0))
                            .child("Aucune vidéo trouvée")
                            .into_any_element()
                    } else {
                        div()
                            .id("videos-list")
                            .flex()
                            .flex_col()
                            .gap_2()
                            .size_full()
                            .overflow_y_scroll()
                            .children(self.videos.iter().map(|video| {
                                let video_url = video.url.clone();
                                let channel_name = self.channels[channel_index].name.clone();
                                let status = video.status.clone();

                                // Récupérer la progression si en cours de téléchargement
                                let progress = if status == VideoStatus::Downloading {
                                    self.download_queue
                                        .get_tasks()
                                        .iter()
                                        .find(|t| t.video_url == video_url)
                                        .map(|t| t.progress)
                                } else {
                                    None
                                };

                                let mut video_item = VideoItem::new(video.clone());
                                if let Some(p) = progress {
                                    video_item = video_item.with_progress(p);
                                }

                                div()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .p_3()
                                    .bg(rgb(NORD2))
                                    .rounded_md()
                                    .when(status == VideoStatus::NotDownloaded, |this| {
                                        this.cursor_pointer()
                                            .hover(|style| style.bg(rgb(NORD3)))
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(move |this, _event, _window, cx| {
                                                    this.start_download(
                                                        video_url.clone(),
                                                        channel_name.clone(),
                                                        cx,
                                                    );
                                                }),
                                            )
                                    })
                                    .child(video_item)
                            }))
                            .into_any_element()
                    }),
            )
    }

    fn render_download_overlay(
        &mut self,
        main_content: AnyElement,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .size_full()
            .relative()
            .child(main_content)
            .child(
                // Overlay semi-transparent
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .bg(black().opacity(0.7))
                    .flex()
                    .items_center()
                    .justify_center()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _event, _window, cx| {
                            this.cancel_download(cx);
                        }),
                    )
                    .child(
                        // Dialog box
                        div()
                            .w(px(500.0))
                            .bg(rgb(NORD1))
                            .rounded_lg()
                            .p_6()
                            .flex()
                            .flex_col()
                            .gap_4()
                            .on_mouse_down(MouseButton::Left, |_event, _phase, cx| {
                                cx.stop_propagation();
                            })
                            .child(
                                // Titre
                                div()
                                    .text_color(rgb(NORD6))
                                    .text_size(px(18.0))
                                    .font_weight(FontWeight::BOLD)
                                    .child("Télécharger la vidéo"),
                            )
                            .child(
                                // Input
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .child(
                                        div().text_color(rgb(NORD4)).text_size(px(13.0)).child(
                                            "Entrez le nom du fichier (sans extension .mp4) :",
                                        ),
                                    )
                                    .child(
                                        div()
                                            .h_10()
                                            .px_3()
                                            .bg(rgb(NORD2))
                                            .border_1()
                                            .border_color(rgb(NORD3))
                                            .rounded_md()
                                            .on_key_down(cx.listener(
                                                |this, event: &KeyDownEvent, window, cx| {
                                                    if event.keystroke.key == "enter" {
                                                        this.confirm_download(window, cx);
                                                    } else if event.keystroke.key == "escape" {
                                                        this.cancel_download(cx);
                                                    }
                                                },
                                            ))
                                            .child(self.download_input.clone().unwrap()),
                                    ),
                            )
                            .when_some(self.download_video.as_ref(), |this, video| {
                                this.child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_2()
                                        .child(
                                            div()
                                                .flex()
                                                .justify_between()
                                                .child(
                                                    div()
                                                        .text_color(rgb(NORD4))
                                                        .text_size(px(13.0))
                                                        .child(format!(
                                                            "Progression: {:.0}%",
                                                            video.progress * 100.0
                                                        )),
                                                )
                                                .when_some(video.speed.as_ref(), |this, speed| {
                                                    this.child(
                                                        div()
                                                            .text_color(rgb(NORD4))
                                                            .text_size(px(13.0))
                                                            .child(format!("{}", speed)),
                                                    )
                                                })
                                                .when_some(video.eta.as_ref(), |this, eta| {
                                                    this.child(
                                                        div()
                                                            .text_color(rgb(NORD4))
                                                            .text_size(px(13.0))
                                                            .child(format!("ETA {}", eta)),
                                                    )
                                                }),
                                        )
                                        .child(components::ProgressBar::new(video.progress)),
                                )
                            })
                            .child(
                                // Boutons
                                div()
                                    .flex()
                                    .gap_3()
                                    .justify_end()
                                    .child(
                                        // Bouton Annuler
                                        div()
                                            .px_4()
                                            .py_2()
                                            .bg(rgb(NORD2))
                                            .rounded_md()
                                            .cursor_pointer()
                                            .hover(|style| style.bg(rgb(NORD3)))
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(|this, _event, _window, cx| {
                                                    this.cancel_download(cx);
                                                }),
                                            )
                                            .child(
                                                div()
                                                    .text_color(rgb(NORD6))
                                                    .text_size(px(14.0))
                                                    .child("Annuler"),
                                            ),
                                    )
                                    .child(
                                        // Bouton Télécharger
                                        div()
                                            .px_4()
                                            .py_2()
                                            .bg(rgb(NORD8))
                                            .rounded_md()
                                            .cursor_pointer()
                                            .hover(|style| style.bg(rgb(NORD10)))
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(|this, _event, window, cx| {
                                                    this.confirm_download(window, cx);
                                                }),
                                            )
                                            .child(
                                                div()
                                                    .text_color(rgb(NORD6))
                                                    .text_size(px(14.0))
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .child("Télécharger"),
                                            ),
                                    ),
                            ),
                    ),
            )
            .into_any_element()
    }
}
