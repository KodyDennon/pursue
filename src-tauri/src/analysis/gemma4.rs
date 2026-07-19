use candle_core::{Device, Result, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::gemma4::{config::Gemma4Config, text::TextModel};

/// Parses the real Gemma 4 multimodal configuration. Gemma 4 E4B stores its text
/// architecture under `text_config`; accepting a Gemma 3 config here would make an
/// incompatible checkpoint appear healthy until inference starts.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(transparent)]
pub struct ConfigWrapper(serde_json::Value);

impl ConfigWrapper {
    pub fn extract(self) -> std::result::Result<Config, String> {
        let model_type = self
            .0
            .get("model_type")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        if model_type != "gemma4" {
            return Err(format!(
                "expected a Gemma 4 checkpoint (model_type=gemma4), found {model_type:?}"
            ));
        }
        serde_json::from_value(self.0)
            .map_err(|error| format!("could not parse Gemma 4 configuration: {error}"))
    }
}

pub type Config = Gemma4Config;

/// Text-only view of the official Gemma 4 checkpoint. The E4B safetensors also
/// contain image/audio towers, but evidence synthesis only needs the language model;
/// avoiding those towers saves accelerator memory without altering the cached files.
pub struct Model {
    inner: TextModel,
    pub device: Device,
}

impl Model {
    pub fn new(cfg: &Config, vb: VarBuilder) -> Result<Self> {
        let device = vb.device().clone();
        let inner = TextModel::new(&cfg.text_config, vb.pp("model").pp("language_model"))?;
        Ok(Self { inner, device })
    }

    pub fn forward(&mut self, tokens: &Tensor, index: usize) -> Result<Tensor> {
        self.inner.forward(tokens, index)
    }

    pub fn clear_kv_cache(&mut self) {
        self.inner.clear_kv_cache();
    }
}
