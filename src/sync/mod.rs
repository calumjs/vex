pub mod github;

use std::path::PathBuf;

use anyhow::Result;
use ureq::tls::{TlsConfig, TlsProvider};

/// Root directory for synced sources: {data_local_dir}/vex/sources/
pub fn sources_dir() -> Result<PathBuf> {
    let base = dirs::data_local_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;
    Ok(base.join("vex").join("sources"))
}

/// Create a shared HTTP agent with TLS configured.
pub fn http_agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .tls_config(
            TlsConfig::builder()
                .provider(TlsProvider::NativeTls)
                .build(),
        )
        .build()
        .new_agent()
}
