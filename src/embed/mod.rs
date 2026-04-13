mod onnx;

pub use onnx::OnnxEmbedder;

/// Which device to prefer for inference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Device {
    /// Auto-detect: try DirectML (NPU/GPU), fall back to CPU
    Auto,
    /// Force NPU via DirectML
    Npu,
    /// Force GPU via DirectML
    Gpu,
    /// Force CPU only
    Cpu,
}

impl Device {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "npu" => Device::Npu,
            "gpu" => Device::Gpu,
            "cpu" => Device::Cpu,
            _ => Device::Auto,
        }
    }
}
