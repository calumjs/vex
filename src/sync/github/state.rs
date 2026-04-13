use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Tracks sync watermark for incremental updates.
#[derive(Serialize, Deserialize, Debug, Default)]
pub struct SyncState {
    /// ISO 8601 timestamp of the last sync
    pub last_sync: String,
    /// The repo that was synced
    pub repo: String,
    /// What was included
    pub include: Vec<String>,
    /// Total items synced
    pub item_count: usize,
}

/// Manifest tracking all synced files.
#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Manifest {
    pub version: u32,
    pub source_type: String,
    pub owner: String,
    pub repo: String,
    pub included_kinds: Vec<String>,
    pub synced_at: String,
    pub files: Vec<ManifestEntry>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ManifestEntry {
    pub number: u64,
    pub kind: String,
    pub path: String,
    pub updated_at: String,
}

/// Load sync state from `.meta/sync-state.json`.
pub fn load_state(repo_dir: &Path) -> Option<SyncState> {
    let path = repo_dir.join(".meta").join("sync-state.json");
    let data = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&data).ok()
}

/// Save sync state to `.meta/sync-state.json`.
pub fn save_state(repo_dir: &Path, state: &SyncState) -> Result<()> {
    let meta_dir = repo_dir.join(".meta");
    std::fs::create_dir_all(&meta_dir)?;
    let path = meta_dir.join("sync-state.json");
    let json = serde_json::to_string_pretty(state)?;
    std::fs::write(&path, json).context("Failed to write sync state")?;
    Ok(())
}

/// Save manifest to `manifest.json`.
pub fn save_manifest(repo_dir: &Path, manifest: &Manifest) -> Result<()> {
    let path = repo_dir.join("manifest.json");
    let json = serde_json::to_string_pretty(manifest)?;
    std::fs::write(&path, json).context("Failed to write manifest")?;
    Ok(())
}
