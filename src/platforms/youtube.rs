use anyhow::Result;
use crate::database::Video;
use super::Platform;

pub struct YouTube {
    client: reqwest::Client,
}

impl YouTube {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    /// Parse le channel ID ou URL pour obtenir l'URL RSS
    fn get_rss_url(channel: &str) -> String {
        // Si c'est déjà un channel ID
        if channel.starts_with("UC") && channel.len() == 24 {
            return format!("https://www.youtube.com/feeds/videos.xml?channel_id={}", channel);
        }

        // Si c'est un nom d'utilisateur
        format!("https://www.youtube.com/feeds/videos.xml?user={}", channel)
    }
}

impl Platform for YouTube {
    async fn get_latest_videos(&self, channel: &str) -> Result<Vec<Video>> {
        let rss_url = Self::get_rss_url(channel);

        tracing::info!("Récupération du flux RSS YouTube: {}", rss_url);

        let response = self.client.get(&rss_url).send().await?;
        let _content = response.text().await?;

        // Parse le XML RSS
        // Pour simplifier, on utilise une approche basique
        // Dans une version plus robuste, utilisez une bibliothèque XML
        let videos = Vec::new();

        // Extraction simple des vidéos du flux RSS
        // Format: <yt:videoId>ID</yt:videoId>
        // <title>Titre</title>

        // TODO: Implémenter un parsing XML plus robuste
        // Pour l'instant, retourne une liste vide

        tracing::warn!("Parsing RSS YouTube non implémenté complètement");

        Ok(videos)
    }
}
