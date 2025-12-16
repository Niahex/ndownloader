use gpui::*;

pub struct NDownloadApp {
    search_query: SharedString,
    channels: Vec<Channel>,
}

#[derive(Clone, Debug)]
struct Channel {
    name: String,
    platform: Platform,
}

#[derive(Clone, Debug, PartialEq)]
enum Platform {
    YouTube,
    Twitch,
}

impl NDownloadApp {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            search_query: "".into(),
            channels: Vec::new(),
        }
    }

    fn add_channel(&mut self, name: String, platform: Platform) {
        self.channels.push(Channel { name, platform });
    }
}

impl Render for NDownloadApp {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
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
                // Search bar section
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
                        // Search input
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
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .h_full()
                                            .text_color(rgb(0xcccccc))
                                            .text_size(px(14.0))
                                            .child("Nom de la chaîne...")
                                    )
                            )
                            .child(
                                // YouTube button
                                div()
                                    .h_10()
                                    .px_4()
                                    .bg(rgb(0xff0000))
                                    .rounded_md()
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .h_full()
                                            .text_color(rgb(0xffffff))
                                            .text_size(px(14.0))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child("YouTube")
                                    )
                            )
                            .child(
                                // Twitch button
                                div()
                                    .h_10()
                                    .px_4()
                                    .bg(rgb(0x9146ff))
                                    .rounded_md()
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .h_full()
                                            .text_color(rgb(0xffffff))
                                            .text_size(px(14.0))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child("Twitch")
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
                                .flex()
                                .flex_col()
                                .gap_2()
                                .children(self.channels.iter().map(|channel| {
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
    }
}
