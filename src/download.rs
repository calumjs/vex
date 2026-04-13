use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

const HF_BASE: &str = "https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main";

/// Files needed for the default model (all-MiniLM-L6-v2).
const MODEL_FILES: &[(&str, &str)] = &[
    ("tokenizer.json", "/tokenizer.json"),
    ("tokenizer_config.json", "/tokenizer_config.json"),
    ("vocab.txt", "/vocab.txt"),
    ("model.onnx", "/onnx/model.onnx"),
];

/// Architecture-specific quantized model for faster inference.
#[cfg(target_arch = "aarch64")]
const QUANTIZED_MODEL: (&str, &str) = ("model_int8.onnx", "/onnx/model_qint8_arm64.onnx");

#[cfg(not(target_arch = "aarch64"))]
const QUANTIZED_MODEL: (&str, &str) = ("model_int8.onnx", "/onnx/quint8_avx2.onnx");

/// Returns the platform data directory for vex models.
/// Windows: %LOCALAPPDATA%\vex\models\
/// Linux/Mac: ~/.local/share/vex/models/
pub fn models_dir() -> Result<PathBuf> {
    let base = dirs::data_local_dir()
        .context("Could not determine data directory")?;
    Ok(base.join("vex").join("models"))
}

/// Download the default model from HuggingFace Hub.
/// Stores files in the platform data directory.
pub fn download_default_model() -> Result<PathBuf> {
    let model_name = "minilm-l6-v2";
    let dest = models_dir()?.join(model_name);
    fs::create_dir_all(&dest)?;

    eprintln!("vex: downloading model (all-MiniLM-L6-v2) — this only happens once");

    let all_files = MODEL_FILES.iter()
        .chain(std::iter::once(&QUANTIZED_MODEL));

    for (filename, hf_path) in all_files {
        let dest_file = dest.join(filename);
        if dest_file.exists() {
            eprintln!("  {filename} (cached)");
            continue;
        }

        let url = format!("{HF_BASE}{hf_path}");
        eprintln!("  {filename} ...");

        download_file(&url, &dest_file)
            .with_context(|| format!("Failed to download {filename} from {url}"))?;
    }

    eprintln!("vex: model ready at {}", dest.display());
    Ok(dest)
}

/// Download a single file from a URL to a local path.
fn download_file(url: &str, dest: &Path) -> Result<()> {
    use ureq::tls::{TlsConfig, TlsProvider};

    let agent = ureq::Agent::config_builder()
        .tls_config(TlsConfig::builder().provider(TlsProvider::NativeTls).build())
        .build()
        .new_agent();
    let response = agent.get(url).call()
        .context("HTTP request failed")?;

    let len = response.headers().get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok());

    // Write to a temp file first, then rename (atomic-ish)
    let tmp = dest.with_extension("tmp");
    let mut file = fs::File::create(&tmp)?;

    let mut reader = response.into_body().into_reader();
    let mut buf = vec![0u8; 256 * 1024]; // 256KB buffer
    let mut downloaded = 0u64;

    loop {
        let n = std::io::Read::read(&mut reader, &mut buf)?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])?;
        downloaded += n as u64;

        // Progress indicator for large files
        if let Some(total) = len {
            if total > 1_000_000 {
                let pct = (downloaded as f64 / total as f64 * 100.0) as u32;
                // Print every ~10%
                if downloaded % (total / 10).max(1) < buf.len() as u64 {
                    eprint!("\r    {:.1} MB / {:.1} MB ({pct}%)",
                        downloaded as f64 / 1e6,
                        total as f64 / 1e6);
                }
            }
        }
    }

    if len.is_some_and(|t| t > 1_000_000) {
        eprintln!(); // newline after progress
    }

    file.flush()?;
    drop(file);
    fs::rename(&tmp, dest)?;

    Ok(())
}
