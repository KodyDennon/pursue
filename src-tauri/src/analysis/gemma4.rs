use candle_core::{Device, Result, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::gemma3;

/// Accepts both the text-only Gemma 3 configuration and the `text_config` nested in
/// multimodal repositories. Keeping parsing here prevents model-specific JSON structure
/// from leaking into the inference orchestrator.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(transparent)]
pub struct ConfigWrapper(serde_json::Value);

impl ConfigWrapper {
    pub fn extract(self) -> std::result::Result<Config, String> {
        let value = self.0.get("text_config").cloned().unwrap_or(self.0);
        serde_json::from_value(value)
            .map_err(|error| format!("Could not parse supported Gemma 3 config: {error}"))
    }
}

pub type Config = gemma3::Config;

/// Thin compatibility wrapper around Candle's maintained Gemma 3 implementation. The
/// previous hand-written "Gemma 4" decoder did not implement that checkpoint's per-layer
/// geometry or AltUp architecture and could never produce valid inference.
pub struct Model {
    inner: gemma3::Model,
    pub device: Device,
}

impl Model {
    pub fn new(cfg: &Config, vb: VarBuilder) -> Result<Self> {
        // Gemma 3 4B+ multimodal repositories nest the language model under
        // `language_model.model`; the 1B text repository starts directly at `model`.
        let vb = if vb.contains_tensor("language_model.model.embed_tokens.weight") {
            vb.pp("language_model")
        } else {
            vb
        };
        let device = vb.device().clone();
        let inner = gemma3::Model::new(false, cfg, vb)?;
        Ok(Self { inner, device })
    }

    pub fn forward(&mut self, tokens: &Tensor, index: usize) -> Result<Tensor> {
        self.inner.forward(tokens, index)
    }

    pub fn clear_kv_cache(&mut self) {
        self.inner.clear_kv_cache();
    }
}
