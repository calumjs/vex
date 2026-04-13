use std::path::Path;

use anyhow::{Context, Result};
use ndarray::Array2;
use ort::session::Session;
use ort::value::Tensor;
use tokenizers::Tokenizer;

use super::Device;

/// ONNX Runtime-based embedder with DirectML (NPU/GPU) support.
pub struct OnnxEmbedder {
    session: Session,
    tokenizer: Tokenizer,
    dim: usize,
}

impl OnnxEmbedder {
    /// Load an ONNX model and tokenizer from the given directory.
    ///
    /// Tries DirectML execution provider first (for Hexagon NPU / Adreno GPU),
    /// falls back to CPU if DirectML is unavailable.
    pub fn load(model_dir: &Path, device: Device) -> Result<Self> {
        let tokenizer_path = model_dir.join("tokenizer.json");
        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| anyhow::anyhow!("Failed to load tokenizer: {e}"))?;

        // Pick model file: prefer INT8 quantized (NPU-optimized), fall back to FP32
        let model_path = if model_dir.join("model_int8.onnx").exists() {
            eprintln!("vex: using INT8 quantized model (NPU-optimized)");
            model_dir.join("model_int8.onnx")
        } else {
            model_dir.join("model.onnx")
        };

        let session = Self::create_session(&model_path, device)?;

        // MiniLM-L6-v2 has 384 dimensions
        let dim = 384;

        Ok(Self {
            session,
            tokenizer,
            dim,
        })
    }

    fn create_session(model_path: &Path, device: Device) -> Result<Session> {
        match device {
            Device::Auto | Device::Npu | Device::Gpu => {
                // Try DirectML EP (exposes Hexagon NPU on Snapdragon X Elite)
                let mut dml = ort::ep::DirectML::default();
                if device == Device::Npu {
                    dml = dml.with_device_filter(ort::ep::directml::DeviceFilter::Npu);
                }

                let try_dml = (|| -> std::result::Result<Session, String> {
                    let builder =
                        Session::builder().map_err(|e| format!("builder: {e}"))?;
                    let mut builder = builder
                        .with_execution_providers([dml.build()])
                        .map_err(|e| format!("ep: {e}"))?;
                    builder
                        .commit_from_file(model_path)
                        .map_err(|e| format!("commit: {e}"))
                })();

                match try_dml {
                    Ok(session) => {
                        eprintln!("vex: using DirectML execution provider (NPU/GPU)");
                        return Ok(session);
                    }
                    Err(e) => {
                        if device == Device::Auto {
                            eprintln!("vex: DirectML not available ({e}), falling back to CPU");
                        } else {
                            return Err(anyhow::anyhow!(
                                "DirectML requested but not available: {e}"
                            ));
                        }
                    }
                }
            }
            Device::Cpu => {}
        }

        // CPU fallback
        let mut builder =
            Session::builder().map_err(|e| anyhow::anyhow!("Failed to create session builder: {e}"))?;
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
            return Ok(Array2::zeros((0, self.dim)));
        }

        // Tokenize all texts
        let encodings = self
            .tokenizer
            .encode_batch(texts.to_vec(), true)
            .map_err(|e| anyhow::anyhow!("Tokenization failed: {e}"))?;

        let batch_size = encodings.len();

        // Find max length for padding (capped at 128 for MiniLM)
        let max_len = encodings
            .iter()
            .map(|e| e.get_ids().len())
            .max()
            .unwrap_or(0)
            .min(128);

        // Build padded input tensors
        let mut input_ids_data = vec![0i64; batch_size * max_len];
        let mut attention_mask_data = vec![0i64; batch_size * max_len];
        let mut token_type_ids_data = vec![0i64; batch_size * max_len];

        for (i, encoding) in encodings.iter().enumerate() {
            let ids = encoding.get_ids();
            let mask = encoding.get_attention_mask();
            let type_ids = encoding.get_type_ids();
            let len = ids.len().min(max_len);

            for j in 0..len {
                input_ids_data[i * max_len + j] = ids[j] as i64;
                attention_mask_data[i * max_len + j] = mask[j] as i64;
                token_type_ids_data[i * max_len + j] = type_ids[j] as i64;
            }
        }

        // Create ort Tensor values (shape tuple + boxed slice)
        let shape = [batch_size, max_len];
        let input_ids_tensor = Tensor::from_array((shape, input_ids_data.into_boxed_slice()))
            .map_err(|e| anyhow::anyhow!("Failed to create input_ids tensor: {e}"))?;
        let attention_mask_tensor =
            Tensor::from_array((shape, attention_mask_data.clone().into_boxed_slice()))
                .map_err(|e| anyhow::anyhow!("Failed to create attention_mask tensor: {e}"))?;
        let token_type_ids_tensor =
            Tensor::from_array((shape, token_type_ids_data.into_boxed_slice()))
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
        let mut embeddings = Array2::zeros((batch_size, self.dim));

        for i in 0..batch_size {
            let mut sum = vec![0f32; self.dim];
            let mut mask_sum = 0f32;

            for j in 0..max_len {
                let m = attention_mask_data[i * max_len + j] as f32;
                if m > 0.0 {
                    mask_sum += m;
                    for d in 0..self.dim {
                        sum[d] += hidden[[i, j, d]] * m;
                    }
                }
            }

            if mask_sum > 0.0 {
                let mut norm_sq = 0f32;
                for d in 0..self.dim {
                    sum[d] /= mask_sum;
                    norm_sq += sum[d] * sum[d];
                }
                let norm = norm_sq.sqrt();
                if norm > 0.0 {
                    for d in 0..self.dim {
                        embeddings[[i, d]] = sum[d] / norm;
                    }
                }
            }
        }

        Ok(embeddings)
    }

    /// Embedding dimensionality.
    pub fn dim(&self) -> usize {
        self.dim
    }
}
