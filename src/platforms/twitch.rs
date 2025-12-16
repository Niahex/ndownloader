use anyhow::Result;
use crate::database::Video;
use super::Platform;

pub struct Twitch {
    client: reqwest::Client,
}

impl Twitch {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

impl Platform for Twitch {
    async fn get_latest_videos(&self, channel: &str) -> Result<Vec<Video>> {
        // Pour Twitch, on utilise yt-dlp pour obtenir la liste des VODs
        // Alternative: utiliser l'API Twitch officielle (nécessite une clé API)

        tracing::info!("Récupération des VODs Twitch pour: {}", channel);

        let _channel_url = format!("https://www.twitch.tv/{}/videos", channel);

        // TODO: Utiliser yt-dlp pour lister les vidéos disponibles
        // yt-dlp --flat-playlist --dump-json URL

        let videos = Vec::new();

        tracing::warn!("Récupération des VODs Twitch non implémentée complètement");

        Ok(videos)
    }
}
