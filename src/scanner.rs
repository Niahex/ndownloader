use crate::cache::Cache;
use anyhow::Result;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VideoMetadata {
    pub id: String,
    pub title: String,
    pub url: String,
    #[serde(default)]
    pub duration: Option<f64>,
    #[serde(default)]
    pub upload_date: Option<String>,
    #[serde(default)]
    pub uploader: Option<String>,
}

pub struct VideoScanner {
    storage_paths: Vec<String>,
    cache: Cache<Vec<VideoMetadata>>,
    file_durations_cache: Arc<Mutex<HashMap<String, f64>>>,
}

impl VideoScanner {
    pub fn new() -> Self {
        Self {
            storage_paths: vec![
                "/run/mount/ve_stock_1".to_string(),
                "/run/mount/ve_stock_2".to_string(),
                "/run/mount/ve_ext_1".to_string(),
            ],
            cache: Cache::new(
                std::path::PathBuf::from("/tmp/ndownload_videos_cache.json"),
                Duration::from_secs(300),
            ),
            file_durations_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Scanne les vidéos disponibles d'une chaîne avec yt-dlp
    pub async fn scan_channel_videos(&self, channel_url: &str) -> Result<Vec<VideoMetadata>> {
        tracing::info!("Scan des vidéos de: {}", channel_url);

        // Pour Twitch, s'assurer qu'on utilise l'URL /videos pour les VODs
        let url = if channel_url.contains("twitch.tv") && !channel_url.contains("/videos") {
            format!("{}/videos", channel_url.trim_end_matches('/'))
        } else {
            channel_url.to_string()
        };

        tracing::info!("URL utilisée: {}", url);

        // Vérifier le cache
        if let Some(videos) = self.cache.get(&url) {
            tracing::info!("Utilisation du cache pour: {}", url);
            return Ok(videos);
        }

        let output = smol::process::Command::new("yt-dlp")
            .arg("--skip-download")
            .arg("--no-write-info-json")
            .arg("--no-write-playlist-metafiles")
            .arg("--dump-json")
            .arg("--playlist-end")
            .arg("30") // Limiter à 30 vidéos pour garder de la vitesse
            .arg(&url)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("yt-dlp a échoué: {error}");
        }

        let stdout = String::from_utf8(output.stdout)?;
        let mut videos = Vec::new();

        // Chaque ligne est un JSON
        for line in stdout.lines() {
            if line.trim().is_empty() {
                continue;
            }

            match serde_json::from_str::<VideoMetadata>(line) {
                Ok(video) => {
                    tracing::debug!("Vidéo: {} - durée: {:?}", video.title, video.duration);
                    videos.push(video);
                }
                Err(e) => {
                    tracing::warn!("Erreur parsing JSON: {} - ligne: {}", e, line);
                }
            }
        }

        tracing::info!("Trouvé {} vidéos", videos.len());

        // Mettre à jour le cache
        self.cache.set(url.clone(), videos.clone());

        Ok(videos)
    }

    /// Vérifie si une vidéo est déjà téléchargée en comparant la durée
    pub fn is_video_downloaded(&self, channel_name: &str, duration: Option<f64>) -> Option<String> {
        let Some(target_duration) = duration else {
            tracing::debug!("Pas de durée cible, impossible de vérifier");
            return None;
        };

        tracing::debug!(
            "Recherche vidéo avec durée {} pour {}",
            target_duration,
            channel_name
        );

        for storage_path in &self.storage_paths {
            let channel_path = format!("{storage_path}/{channel_name}");

            // Vérifier si le dossier existe
            if let Ok(entries) = std::fs::read_dir(&channel_path) {
                tracing::debug!("Scan du dossier: {}", channel_path);
                for entry in entries.flatten() {
                    let path = entry.path();
                    if !path.is_file() {
                        continue;
                    }

                    let path_str = path.to_string_lossy().to_string();

                    // Vérifier le cache d'abord
                    let local_duration = {
                        let cache = self.file_durations_cache.lock();
                        cache.get(&path_str).copied()
                    };

                    // Si pas en cache, lire avec ffprobe
                    let local_duration = if let Some(dur) = local_duration {
                        dur
                    } else if let Some(dur) = Self::get_video_duration(&path) {
                        // Mettre en cache
                        self.file_durations_cache
                            .lock()
                            .insert(path_str.clone(), dur);
                        dur
                    } else {
                        tracing::warn!("Impossible de lire la durée de: {}", path.display());
                        continue;
                    };

                    tracing::debug!("Fichier: {} - durée: {}", path.display(), local_duration);
                    // Tolérance de 5 secondes
                    if (local_duration - target_duration).abs() < 5.0 {
                        tracing::info!(
                            "Match trouvé: {} (durée: {})",
                            path.display(),
                            local_duration
                        );
                        return Some(path_str);
                    }
                }
            } else {
                tracing::debug!("Dossier n'existe pas: {}", channel_path);
            }
        }

        None
    }

    /// Obtient la durée d'une vidéo locale avec ffprobe
    fn get_video_duration(path: &std::path::Path) -> Option<f64> {
        let output = std::process::Command::new("ffprobe")
            .arg("-v")
            .arg("error")
            .arg("-show_entries")
            .arg("format=duration")
            .arg("-of")
            .arg("default=noprint_wrappers=1:nokey=1")
            .arg(path)
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let duration_str = String::from_utf8(output.stdout).ok()?;
        duration_str.trim().parse::<f64>().ok()
    }

    /// Trouve le meilleur disque de stockage (celui avec le plus d'espace)
    pub fn find_best_storage_path(&self) -> Result<String> {
        // Pour l'instant, retourner le premier disponible
        for path in &self.storage_paths {
            if std::path::Path::new(path).exists() {
                return Ok(path.clone());
            }
        }

        anyhow::bail!("Aucun disque de stockage disponible")
    }
}
