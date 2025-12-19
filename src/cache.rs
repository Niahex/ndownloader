use anyhow::Result;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct Cache<T: Clone + Serialize + for<'de> Deserialize<'de>> {
    data: Arc<RwLock<HashMap<String, CacheEntry<T>>>>,
    cache_file: PathBuf,
    default_ttl: Duration,
}

struct CacheEntry<T> {
    value: T,
    timestamp: Instant,
}

impl<T: Clone + Serialize + for<'de> Deserialize<'de>> Cache<T> {
    pub fn new(cache_file: PathBuf, ttl: Duration) -> Self {
        let data = Self::load_from_disk(&cache_file).unwrap_or_default();
        Self {
            data: Arc::new(RwLock::new(data)),
            cache_file,
            default_ttl: ttl,
        }
    }

    pub fn get(&self, key: &str) -> Option<T> {
        let cache = self.data.read();
        cache.get(key).and_then(|entry| {
            if entry.timestamp.elapsed() < self.default_ttl {
                Some(entry.value.clone())
            } else {
                None
            }
        })
    }

    pub fn set(&self, key: String, value: T) {
        let mut cache = self.data.write();
        cache.insert(
            key,
            CacheEntry {
                value,
                timestamp: Instant::now(),
            },
        );
        drop(cache);

        if let Err(error) = self.save_to_disk() {
            tracing::warn!("Failed to save cache to disk: {}", error);
        }
    }

    fn load_from_disk(path: &PathBuf) -> Result<HashMap<String, CacheEntry<T>>> {
        let content = std::fs::read_to_string(path)?;
        let data: HashMap<String, T> = serde_json::from_str(&content)?;
        Ok(data
            .into_iter()
            .map(|(k, v)| {
                (
                    k,
                    CacheEntry {
                        value: v,
                        timestamp: Instant::now(),
                    },
                )
            })
            .collect())
    }

    fn save_to_disk(&self) -> Result<()> {
        let cache = self.data.read();
        let data: HashMap<String, T> = cache
            .iter()
            .map(|(k, v)| (k.clone(), v.value.clone()))
            .collect();
        let content = serde_json::to_string_pretty(&data)?;
        std::fs::write(&self.cache_file, content)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_cache_set_get() {
        let cache: Cache<String> =
            Cache::new(PathBuf::from("test_cache.json"), Duration::from_secs(3600));
        cache.set("key1".to_string(), "value1".to_string());

        assert_eq!(cache.get("key1"), Some("value1".to_string()));
        assert_eq!(cache.get("nonexistent"), None);
    }

    #[test]
    fn test_cache_clear() {
        let cache: Cache<String> = Cache::new(
            PathBuf::from("test_cache_clear.json"),
            Duration::from_secs(3600),
        );
        cache.set("key1".to_string(), "value1".to_string());
        cache.set("key2".to_string(), "value2".to_string());

        cache.clear();

        assert_eq!(cache.get("key1"), None);
        assert_eq!(cache.get("key2"), None);
    }
}
