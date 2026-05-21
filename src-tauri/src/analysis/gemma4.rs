use candle_core::{Device, IndexOp, Result, Tensor, D};
use candle_nn::{rms_norm, Linear, Module, RmsNorm, VarBuilder};
use log::debug;

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ConfigWrapper {
    pub text_config: Option<Config>,
    #[serde(flatten)]
    pub config: Option<Config>,
}

impl ConfigWrapper {
    pub fn extract(self) -> std::result::Result<Config, String> {
        self.text_config
            .or(self.config)
            .ok_or_else(|| "Could not parse model config".to_string())
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct Config {
    pub vocab_size: usize,
    pub hidden_size: usize,
    pub intermediate_size: usize,
    pub num_hidden_layers: usize,
    pub num_attention_heads: usize,
    pub num_key_value_heads: usize,
    pub head_dim: usize,
    pub rms_norm_eps: f64,
    #[serde(default)]
    pub hidden_size_per_layer_input: usize,
    pub max_position_embeddings: usize,
    #[serde(default = "default_rope_theta")]
    pub rope_theta: f32,
}

fn default_rope_theta() -> f32 {
    10000.0
}

use super::nn::{DecoderLayer, KVCache};

pub struct Model {
    pub embed_tokens: candle_nn::Embedding,
    pub embed_tokens_per_layer: Option<candle_nn::Embedding>,
    pub layers: Vec<DecoderLayer>,
    pub norm: RmsNorm,
    pub lm_head: Linear,
    pub device: Device,
    pub hidden_size: f64,
}

impl Model {
    pub fn new(cfg: &Config, vb: VarBuilder) -> Result<Self> {
        debug!(
            "[Gemma4] Initializing model: vocab_size={}, layers={}, hidden_size={}",
            cfg.vocab_size, cfg.num_hidden_layers, cfg.hidden_size
        );
        // Find the architecture root by checking for common embedding or layer tensor paths
        let mut model_prefix = None;
        let prefixes = [
            "model.language_model",
            "language_model",
            "model",
            "transformer",
            "gemma",
            "gemma2",
            "gemma4",
            "gemma-4",
        ];

        for p in prefixes {
            if vb.contains_tensor(&format!("{}.embed_tokens.weight", p))
                || vb.contains_tensor(&format!("{}.layers.0.self_attn.q_proj.weight", p))
            {
                model_prefix = Some(p);
                break;
            }
        }

        let vb_m = match model_prefix {
            Some(p) => vb.pp(p),
            None => vb.clone(),
        };

        let embed_tokens =
            candle_nn::embedding(cfg.vocab_size, cfg.hidden_size, vb_m.pp("embed_tokens"))?;

        let embed_tokens_per_layer = if vb_m.contains_tensor("embed_tokens_per_layer.weight") {
            Some(candle_nn::embedding(
                cfg.vocab_size,
                cfg.num_hidden_layers * cfg.hidden_size_per_layer_input,
                vb_m.pp("embed_tokens_per_layer"),
            )?)
        } else {
            None
        };

        let mut layers = Vec::with_capacity(cfg.num_hidden_layers);
        let vb_layers = vb_m.pp("layers");
        for i in 0..cfg.num_hidden_layers {
            if i % 10 == 0 || i == cfg.num_hidden_layers - 1 {
                debug!(
                    "[Gemma4] Initializing layer {}/{}",
                    i, cfg.num_hidden_layers
                );
            }
            layers.push(DecoderLayer::new(cfg, vb_layers.pp(i))?);
        }
        let norm = rms_norm(cfg.hidden_size, cfg.rms_norm_eps, vb_m.pp("norm"))?;

        // lm_head detection: try root first, then inside model prefix
        let vb_head = if vb.contains_tensor("lm_head.weight") {
            vb.pp("lm_head")
        } else if vb_m.contains_tensor("lm_head.weight") {
            vb_m.pp("lm_head")
        } else if vb_m.contains_tensor("embed_tokens.weight") {
            // Some models share embedding weights for the head
            vb_m.pp("embed_tokens")
        } else {
            vb.pp("lm_head") // fallback
        };

        let lm_head = candle_nn::linear_no_bias(cfg.hidden_size, cfg.vocab_size, vb_head)?;

        debug!("[Gemma4] Model initialization successful.");

        Ok(Self {
            embed_tokens,
            embed_tokens_per_layer,
            layers,
            norm,
            lm_head,
            device: vb.device().clone(),
            hidden_size: cfg.hidden_size as f64,
        })
    }

    pub fn forward(&self, tokens: &Tensor, index: usize, cache: &mut [KVCache]) -> Result<Tensor> {
        let mut x = self.embed_tokens.forward(tokens)?;
        // Gemma scales embeddings
        x = (x * self.hidden_size.sqrt())?;

        let ple_full = if let Some(emb) = &self.embed_tokens_per_layer {
            Some(emb.forward(tokens)?)
        } else {
            None
        };

        let (_, seq_len) = tokens.dims2()?;
        let mask = if seq_len > 1 {
            let mask_vec: Vec<_> = (0..seq_len)
                .flat_map(|i| {
                    (0..seq_len).map(move |j| if i < j { f32::NEG_INFINITY } else { 0f32 })
                })
                .collect();
            Some(Tensor::from_vec(
                mask_vec,
                (seq_len, seq_len),
                &self.device,
            )?)
        } else {
            None
        };

        for (i, layer) in self.layers.iter().enumerate() {
            let ple_layer = if let Some(ple) = &ple_full {
                // Slice the concatenated embedding for the current layer
                let start = i * (ple.dim(D::Minus1)? / self.layers.len());
                let len = ple.dim(D::Minus1)? / self.layers.len();
                Some(ple.narrow(D::Minus1, start, len)?)
            } else {
                None
            };
            x = layer.forward(&x, ple_layer.as_ref(), index, mask.as_ref(), &mut cache[i])?;
        }
        let x = self.norm.forward(&x)?;
        let x = x.i((.., seq_len - 1, ..))?;
        self.lm_head.forward(&x)
    }

    pub fn new_kv_cache(&self) -> KVCache {
        KVCache::new()
    }
}
