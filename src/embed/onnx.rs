use std::path::Path;

use anyhow::{Context, Result};
use ndarray::Array2;
use ort::session::Session;
use ort::value::Tensor;
use tokenizers::Tokenizer;

use super::Device;

const MAX_SEQ_LEN: usize = 128;
const EMBED_DIM: usize = 384;

/// ONNX Runtime-based embedder with QNN (Hexagon NPU) / CPU support.
pub struct OnnxEmbedder {
    session: Session,
    tokenizer: Tokenizer,
}

impl OnnxEmbedder {
    /// Load an ONNX model and tokenizer from the given directory.
    pub fn load(model_dir: &Path, device: Device, threads: Option<usize>) -> Result<Self> {
        let tokenizer_path = model_dir.join("tokenizer.json");
        let mut tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| anyhow::anyhow!("Failed to load tokenizer: {e}"))?;

        // Truncate at MAX_SEQ_LEN to avoid tokenizing tokens we discard
        tokenizer
            .with_truncation(Some(tokenizers::TruncationParams {
                max_length: MAX_SEQ_LEN,
                ..Default::default()
            }))
            .map_err(|e| anyhow::anyhow!("Failed to set truncation: {e}"))?;

        // Pad to longest in batch on CPU (dynamic shape is fine).
        // QNN/HTP needs fixed shapes — use Fixed(MAX_SEQ_LEN) there.
        let padding_strategy = if device == Device::Npu {
            tokenizers::PaddingStrategy::Fixed(MAX_SEQ_LEN)
        } else {
            tokenizers::PaddingStrategy::BatchLongest
        };
        tokenizer.with_padding(Some(tokenizers::PaddingParams {
            strategy: padding_strategy,
            pad_id: 0,
            pad_type_id: 0,
            pad_token: "[PAD]".to_string(),
            ..Default::default()
        }));

        // Pick model file: QNN HTP handles FP32→FP16 natively, so prefer FP32
        // to avoid INT8 dequantize ops that cause slow HTP compilation.
        // For CPU, INT8 is faster.
        let model_path = if device == Device::Cpu {
            if model_dir.join("model_int8.onnx").exists() {
                eprintln!("vex: using INT8 quantized model");
                model_dir.join("model_int8.onnx")
            } else {
                model_dir.join("model.onnx")
            }
        } else {
            eprintln!("vex: using FP32 model (NPU will run in FP16)");
            model_dir.join("model.onnx")
        };

        let session = Self::create_session(&model_path, device, threads)?;

        Ok(Self {
            session,
            tokenizer,
        })
    }

    fn create_session(
        model_path: &Path,
        device: Device,
        threads: Option<usize>,
    ) -> Result<Session> {
        // QNN EP (Hexagon NPU) — only when explicitly requested via --device npu.
        // Uses context caching: first run compiles the model for HTP (~30s),
        // subsequent runs load the cached context binary (instant).
        if device == Device::Npu {
            // Context cache path next to the model file
            let ctx_path = model_path.with_extension("qnn_ctx.onnx");
            let has_cached_ctx = ctx_path.exists();

            let qnn = ort::ep::QNN::default()
                .with_backend_path("QnnHtp.dll")
                .with_performance_mode(ort::ep::qnn::PerformanceMode::Burst)
                .with_htp_fp16_precision(true);

            let try_qnn = (|| -> std::result::Result<Session, String> {
                let mut builder = Session::builder().map_err(|e| format!("builder: {e}"))?;

                // Enable context caching — saves compiled HTP binary on first run
                if !has_cached_ctx {
                    builder = builder
                        .with_config_entry("ep.context_enable", "1")
                        .map_err(|e| format!("ctx_enable: {e}"))?;
                    builder = builder
                        .with_config_entry("ep.context_file_path", ctx_path.to_string_lossy())
                        .map_err(|e| format!("ctx_path: {e}"))?;
                    builder = builder
                        .with_config_entry("ep.context_embed_mode", "0")
                        .map_err(|e| format!("ctx_mode: {e}"))?;
                }

                let mut builder = builder
                    .with_execution_providers([qnn.build()])
                    .map_err(|e| format!("ep: {e}"))?;

                // Load from cached context if available, otherwise from ONNX model
                let load_path = if has_cached_ctx { &ctx_path } else { model_path };
                builder
                    .commit_from_file(load_path)
                    .map_err(|e| format!("commit: {e}"))
            })();

            match try_qnn {
                Ok(session) => {
                    if has_cached_ctx {
                        eprintln!("vex: using QNN execution provider (Hexagon NPU, cached context)");
                    } else {
                        eprintln!("vex: using QNN execution provider (Hexagon NPU, first-run compilation)");
                    }
                    return Ok(session);
                }
                Err(e) => {
                    return Err(anyhow::anyhow!(
                        "QNN NPU requested but not available: {e}"
                    ));
                }
            }
        }

        // CPU fallback — use all available cores unless overridden
        let num_threads = threads.unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4)
        });
        eprintln!("vex: using CPU execution provider ({num_threads} threads)");
        let builder = Session::builder()
            .map_err(|e| anyhow::anyhow!("Failed to create session builder: {e}"))?;
        let mut builder = builder
            .with_intra_threads(num_threads)
            .map_err(|e| anyhow::anyhow!("Failed to set intra-op threads: {e}"))?;
        builder
            .commit_from_file(model_path)
            .map_err(|e| anyhow::anyhow!("Failed to load ONNX model: {e}"))
    }

    /// Embed a single text string, returning a normalized vector.
    pub fn embed_one(&mut self, text: &str) -> Result<Vec<f32>> {
        let batch = self.embed_batch(&[text])?;
        Ok(batch.row(0).to_vec())
    }

    /// Embed a batch of texts, returning a (batch_size, dim) matrix of normalized vectors.
    pub fn embed_batch(&mut self, texts: &[&str]) -> Result<Array2<f32>> {
        if texts.is_empty() {
            return Ok(Array2::zeros((0, EMBED_DIM)));
        }

        // Tokenize with truncation + padding handled by the tokenizer
        let encodings = self
            .tokenizer
            .encode_batch(texts.to_vec(), true)
            .map_err(|e| anyhow::anyhow!("Tokenization failed: {e}"))?;

        let batch_size = encodings.len();
        let seq_len = encodings[0].get_ids().len();

        // Build input tensors directly from padded encodings
        let total = batch_size * seq_len;
        let mut input_ids = Vec::with_capacity(total);
        let mut attention_mask = Vec::with_capacity(total);
        let mut token_type_ids = Vec::with_capacity(total);

        for enc in &encodings {
            for &id in enc.get_ids() {
                input_ids.push(id as i64);
            }
            for &m in enc.get_attention_mask() {
                attention_mask.push(m as i64);
            }
            for &t in enc.get_type_ids() {
                token_type_ids.push(t as i64);
            }
        }

        let shape = [batch_size, seq_len];
        let input_ids_tensor = Tensor::from_array((shape, input_ids.into_boxed_slice()))
            .map_err(|e| anyhow::anyhow!("Failed to create input_ids tensor: {e}"))?;
        let attention_mask_tensor =
            Tensor::from_array((shape, attention_mask.clone().into_boxed_slice()))
                .map_err(|e| anyhow::anyhow!("Failed to create attention_mask tensor: {e}"))?;
        let token_type_ids_tensor =
            Tensor::from_array((shape, token_type_ids.into_boxed_slice()))
                .map_err(|e| anyhow::anyhow!("Failed to create token_type_ids tensor: {e}"))?;

        // Run inference
        let outputs = self
            .session
            .run(ort::inputs! {
                "input_ids" => input_ids_tensor,
                "attention_mask" => attention_mask_tensor,
                "token_type_ids" => token_type_ids_tensor,
            })
            .map_err(|e| anyhow::anyhow!("Inference failed: {e}"))?;

        // Extract last_hidden_state: (batch, seq_len, hidden_dim)
        let hidden = outputs[0]
            .try_extract_array::<f32>()
            .map_err(|e| anyhow::anyhow!("Failed to extract output tensor: {e}"))?;
        let hidden = hidden
            .into_dimensionality::<ndarray::Ix3>()
            .context("Expected 3D output tensor")?;

        // Mean pooling with attention mask, then L2 normalize
        let mut embeddings = Array2::zeros((batch_size, EMBED_DIM));

        for i in 0..batch_size {
            let mut sum = vec![0f32; EMBED_DIM];
            let mut mask_sum = 0f32;

            for j in 0..seq_len {
                let m = attention_mask[i * seq_len + j] as f32;
                if m > 0.0 {
                    mask_sum += m;
                    for d in 0..EMBED_DIM {
                        sum[d] += hidden[[i, j, d]] * m;
                    }
                }
            }

            if mask_sum > 0.0 {
                let mut norm_sq = 0f32;
                for d in 0..EMBED_DIM {
                    sum[d] /= mask_sum;
                    norm_sq += sum[d] * sum[d];
                }
                let norm = norm_sq.sqrt();
                if norm > 0.0 {
                    for d in 0..EMBED_DIM {
                        embeddings[[i, d]] = sum[d] / norm;
                    }
                }
            }
        }

        Ok(embeddings)
    }

    /// Embedding dimensionality.
    pub fn dim(&self) -> usize {
        EMBED_DIM
    }
}
