use candle_core::{DType, Device, IndexOp, Result, Tensor, D};
use candle_nn::{Linear, Module, VarBuilder, rms_norm, RmsNorm};

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ConfigWrapper {
    pub text_config: Option<Config>,
    #[serde(flatten)]
    pub config: Option<Config>,
}

impl ConfigWrapper {
    pub fn extract(self) -> std::result::Result<Config, String> {
        self.text_config.or(self.config).ok_or_else(|| "Could not parse model config".to_string())
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

fn default_rope_theta() -> f32 { 10000.0 }

#[derive(Clone)]
pub struct KVCache {
    k: Option<Tensor>,
    v: Option<Tensor>,
}

impl KVCache {
    pub fn new() -> Self {
        Self { k: None, v: None }
    }
    
    pub fn append(&mut self, k: &Tensor, v: &Tensor) -> Result<(Tensor, Tensor)> {
        let (k, v) = match (&self.k, &self.v) {
            (Some(prev_k), Some(prev_v)) => {
                let k = Tensor::cat(&[prev_k, k], 2)?;
                let v = Tensor::cat(&[prev_v, v], 2)?;
                (k, v)
            }
            _ => (k.clone(), v.clone()),
        };
        self.k = Some(k.clone());
        self.v = Some(v.clone());
        Ok((k, v))
    }
}

pub struct MLP {
    pub gate_proj: Linear,
    pub up_proj: Linear,
    pub down_proj: Linear,
}

impl MLP {
    fn new(cfg: &Config, vb: VarBuilder) -> Result<Self> {
        let gate_proj = candle_nn::linear_no_bias(cfg.hidden_size, cfg.intermediate_size, vb.pp("gate_proj"))?;
        let up_proj = candle_nn::linear_no_bias(cfg.hidden_size, cfg.intermediate_size, vb.pp("up_proj"))?;
        let down_proj = candle_nn::linear_no_bias(cfg.intermediate_size, cfg.hidden_size, vb.pp("down_proj"))?;
        Ok(Self { gate_proj, up_proj, down_proj })
    }
}

impl Module for MLP {
    fn forward(&self, x: &Tensor) -> Result<Tensor> {
        // Gemma uses GELU with tanh approximation
        let x = (self.gate_proj.forward(x)?.gelu_erf()? * self.up_proj.forward(x)?)?;
        self.down_proj.forward(&x)
    }
}

pub struct RotaryEmbedding {
    sin: Tensor,
    cos: Tensor,
}

impl RotaryEmbedding {
    fn new(cfg: &Config, device: &Device) -> Result<Self> {
        let dim = cfg.head_dim;
        let max_seq_len = cfg.max_position_embeddings;
        let inv_freq: Vec<_> = (0..dim)
            .step_by(2)
            .map(|i| 1f32 / cfg.rope_theta.powf(i as f32 / dim as f32))
            .collect();
        let inv_freq = Tensor::new(inv_freq, device)?;
        let t = Tensor::arange(0u32, max_seq_len as u32, device)?.to_dtype(DType::F32)?;
        let freqs = t.unsqueeze(1)?.matmul(&inv_freq.unsqueeze(0)?)?;
        let sin = freqs.sin()?;
        let cos = freqs.cos()?;
        Ok(Self { sin, cos })
    }

    fn apply(&self, x: &Tensor, index: usize) -> Result<Tensor> {
        let (_b_sz, _n_heads, seq_len, head_dim) = x.dims4()?;
        let cos = self.cos.narrow(0, index, seq_len)?.reshape((1, 1, seq_len, head_dim / 2))?;
        let sin = self.sin.narrow(0, index, seq_len)?.reshape((1, 1, seq_len, head_dim / 2))?;
        
        let x1 = x.narrow(D::Minus1, 0, head_dim / 2)?;
        let x2 = x.narrow(D::Minus1, head_dim / 2, head_dim / 2)?;
        
        let rotated_x1 = (x1.broadcast_mul(&cos)? - x2.broadcast_mul(&sin)?)?;
        let rotated_x2 = (x1.broadcast_mul(&sin)? + x2.broadcast_mul(&cos)?)?;
        Tensor::cat(&[rotated_x1, rotated_x2], D::Minus1)
    }
}

pub struct Attention {
    pub q_proj: Linear,
    pub k_proj: Linear,
    pub v_proj: Linear,
    pub o_proj: Linear,
    pub q_norm: Option<RmsNorm>,
    pub k_norm: Option<RmsNorm>,
    pub num_heads: usize,
    pub num_kv_heads: usize,
    pub head_dim: usize,
    pub rotary: RotaryEmbedding,
}

impl Attention {
    pub fn new(cfg: &Config, vb: VarBuilder) -> Result<Self> {
        let q_proj = candle_nn::linear_no_bias(cfg.hidden_size, cfg.num_attention_heads * cfg.head_dim, vb.pp("q_proj"))?;
        let k_proj = candle_nn::linear_no_bias(cfg.hidden_size, cfg.num_key_value_heads * cfg.head_dim, vb.pp("k_proj"))?;
        let v_proj = candle_nn::linear_no_bias(cfg.hidden_size, cfg.num_key_value_heads * cfg.head_dim, vb.pp("v_proj"))?;
        let o_proj = candle_nn::linear_no_bias(cfg.num_attention_heads * cfg.head_dim, cfg.hidden_size, vb.pp("o_proj"))?;
        
        let q_norm = if vb.contains_tensor("q_norm.weight") {
            Some(rms_norm(cfg.head_dim, cfg.rms_norm_eps, vb.pp("q_norm"))?)
        } else {
            None
        };
        let k_norm = if vb.contains_tensor("k_norm.weight") {
            Some(rms_norm(cfg.head_dim, cfg.rms_norm_eps, vb.pp("k_norm"))?)
        } else {
            None
        };

        let rotary = RotaryEmbedding::new(cfg, vb.device())?;
        Ok(Self {
            q_proj,
            k_proj,
            v_proj,
            o_proj,
            q_norm,
            k_norm,
            num_heads: cfg.num_attention_heads,
            num_kv_heads: cfg.num_key_value_heads,
            head_dim: cfg.head_dim,
            rotary,
        })
    }

    pub fn forward(&self, x: &Tensor, index: usize, mask: Option<&Tensor>, cache: &mut KVCache) -> Result<Tensor> {
        let (b_sz, seq_len, _) = x.dims3()?;
        let mut q = self.q_proj.forward(x)?;
        let mut k = self.k_proj.forward(x)?;
        let v = self.v_proj.forward(x)?;

        if let (Some(q_n), Some(k_n)) = (&self.q_norm, &self.k_norm) {
            // Apply normalization to head dimension
            q = q.reshape((b_sz, seq_len, self.num_heads, self.head_dim))?;
            q = q_n.forward(&q)?.reshape((b_sz, seq_len, ()))?;
            k = k.reshape((b_sz, seq_len, self.num_kv_heads, self.head_dim))?;
            k = k_n.forward(&k)?.reshape((b_sz, seq_len, ()))?;
        }

        let q = q.reshape((b_sz, seq_len, self.num_heads, self.head_dim))?.transpose(1, 2)?;
        let k = k.reshape((b_sz, seq_len, self.num_kv_heads, self.head_dim))?.transpose(1, 2)?;
        let v = v.reshape((b_sz, seq_len, self.num_kv_heads, self.head_dim))?.transpose(1, 2)?;

        let q = self.rotary.apply(&q, index)?;
        let k = self.rotary.apply(&k, index)?;

        let (k, v) = cache.append(&k, &v)?;

        // Repeat KV heads if needed (Grouped Query Attention)
        let k = if self.num_heads != self.num_kv_heads {
            let n_rep = self.num_heads / self.num_kv_heads;
            k.unsqueeze(2)?.expand((b_sz, self.num_kv_heads, n_rep, k.dim(2)?, self.head_dim))?.reshape((b_sz, self.num_heads, k.dim(2)?, self.head_dim))?
        } else { k };
        
        let v = if self.num_heads != self.num_kv_heads {
            let n_rep = self.num_heads / self.num_kv_heads;
            v.unsqueeze(2)?.expand((b_sz, self.num_kv_heads, n_rep, v.dim(2)?, self.head_dim))?.reshape((b_sz, self.num_heads, v.dim(2)?, self.head_dim))?
        } else { v };

        let scale = 1.0 / (self.head_dim as f64).sqrt();
        let att = (q.matmul(&k.transpose(2, 3)?)? * scale)?;
        let att = match mask {
            Some(mask) => att.broadcast_add(mask)?,
            None => att,
        };
        let att = candle_nn::ops::softmax(&att, D::Minus1)?;
        let x = att.matmul(&v)?;
        let x = x.transpose(1, 2)?.reshape((b_sz, seq_len, ()))?;
        self.o_proj.forward(&x)
    }
}

pub struct DecoderLayer {
    pub self_attn: Attention,
    pub mlp: MLP,
    pub input_layernorm: RmsNorm,
    pub post_attention_layernorm: RmsNorm,
    pub per_layer_input_gate: Option<Linear>,
    pub per_layer_projection: Option<Linear>,
    pub post_per_layer_input_norm: Option<RmsNorm>,
}

impl DecoderLayer {
    fn new(cfg: &Config, vb: VarBuilder) -> Result<Self> {
        let self_attn = Attention::new(cfg, vb.pp("self_attn"))?;
        let mlp = MLP::new(cfg, vb.pp("mlp"))?;
        let input_layernorm = rms_norm(cfg.hidden_size, cfg.rms_norm_eps, vb.pp("input_layernorm"))?;
        let post_attention_layernorm = rms_norm(cfg.hidden_size, cfg.rms_norm_eps, vb.pp("post_attention_layernorm"))?;
        
        let per_layer_input_gate = if cfg.hidden_size_per_layer_input > 0 {
            Some(candle_nn::linear_no_bias(cfg.hidden_size, cfg.hidden_size_per_layer_input, vb.pp("per_layer_input_gate"))?)
        } else {
            None
        };
        
        let per_layer_projection = if cfg.hidden_size_per_layer_input > 0 {
            Some(candle_nn::linear_no_bias(cfg.hidden_size_per_layer_input, cfg.hidden_size, vb.pp("per_layer_projection"))?)
        } else {
            None
        };

        let post_per_layer_input_norm = if cfg.hidden_size_per_layer_input > 0 {
            Some(rms_norm(cfg.hidden_size, cfg.rms_norm_eps, vb.pp("post_per_layer_input_norm"))?)
        } else {
            None
        };

        Ok(Self {
            self_attn, mlp, input_layernorm, post_attention_layernorm,
            per_layer_input_gate, per_layer_projection, post_per_layer_input_norm,
        })
    }

    pub fn forward(&self, x: &Tensor, ple_x: Option<&Tensor>, index: usize, mask: Option<&Tensor>, cache: &mut KVCache) -> Result<Tensor> {
        let mut x = x.clone();
        if let (Some(px), Some(proj), Some(gate)) = (ple_x, &self.per_layer_projection, &self.per_layer_input_gate) {
            let g = candle_nn::ops::sigmoid(&gate.forward(&x)?)?;
            let gated_px = px.broadcast_mul(&g)?;
            let mut ple_proj = proj.forward(&gated_px)?;
            
            if let Some(norm) = &self.post_per_layer_input_norm {
                ple_proj = norm.forward(&ple_proj)?;
            }
            x = x.add(&ple_proj)?;
        }

        let residual = x.clone();
        let mut x = self.input_layernorm.forward(&x)?;
        x = self.self_attn.forward(&x, index, mask, cache)?;
        let mut x = x.add(&residual)?;

        let mut x2 = self.post_attention_layernorm.forward(&x)?;
        x2 = self.mlp.forward(&x2)?;
        x = x.add(&x2)?;
        Ok(x)
    }
}

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
        // Find the architecture root by checking for common embedding or layer tensor paths
        let mut model_prefix = None;
        let prefixes = ["model.language_model", "language_model", "model", "transformer", "gemma", "gemma2", "gemma4", "gemma-4"];
        
        for p in prefixes {
            if vb.contains_tensor(&format!("{}.embed_tokens.weight", p)) || 
               vb.contains_tensor(&format!("{}.layers.0.self_attn.q_proj.weight", p)) {
                model_prefix = Some(p);
                break;
            }
        }

        let vb_m = match model_prefix {
            Some(p) => vb.pp(p),
            None => vb.clone(),
        };

        let embed_tokens = candle_nn::embedding(cfg.vocab_size, cfg.hidden_size, vb_m.pp("embed_tokens"))?;
        
        let embed_tokens_per_layer = if cfg.hidden_size_per_layer_input > 0 {
            Some(candle_nn::embedding(cfg.vocab_size, cfg.hidden_size_per_layer_input, vb_m.pp("embed_tokens_per_layer"))?)
        } else {
            None
        };

        let mut layers = Vec::with_capacity(cfg.num_hidden_layers);
        let vb_layers = vb_m.pp("layers");
        for i in 0..cfg.num_hidden_layers {
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
        
        Ok(Self { 
            embed_tokens, 
            embed_tokens_per_layer,
            layers, 
            norm, 
            lm_head, 
            device: vb.device().clone(), 
            hidden_size: cfg.hidden_size as f64 
        })
    }

    pub fn forward(&self, tokens: &Tensor, index: usize, cache: &mut [KVCache]) -> Result<Tensor> {
        let mut x = self.embed_tokens.forward(tokens)?;
        // Gemma scales embeddings
        x = (x * self.hidden_size.sqrt())?;
        
        let ple_x = if let Some(emb) = &self.embed_tokens_per_layer {
            Some(emb.forward(tokens)?)
        } else {
            None
        };

        let (_, seq_len) = tokens.dims2()?;
        let mask = if seq_len > 1 {
            let mask_vec: Vec<_> = (0..seq_len)
                .flat_map(|i| (0..seq_len).map(move |j| if i < j { f32::NEG_INFINITY } else { 0f32 }))
                .collect();
            Some(Tensor::from_vec(mask_vec, (seq_len, seq_len), &self.device)?)
        } else {
            None
        };

        for (i, layer) in self.layers.iter().enumerate() {
            x = layer.forward(&x, ple_x.as_ref(), index, mask.as_ref(), &mut cache[i])?;
        }
        let x = self.norm.forward(&x)?;
        let x = x.i((.., seq_len - 1, ..))?;
        self.lm_head.forward(&x)
    }
}
