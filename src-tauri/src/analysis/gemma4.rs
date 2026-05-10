use candle_core::{DType, Device, IndexOp, Result, Tensor, D};
use candle_nn::{Linear, Module, VarBuilder, rms_norm, RmsNorm};

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
    pub hidden_size_per_layer_input: usize,
    pub max_position_embeddings: usize,
    pub rope_theta: f32,
}

struct MLP {
    gate_proj: Linear,
    up_proj: Linear,
    down_proj: Linear,
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
        let x = (self.gate_proj.forward(x)?.gelu()? * self.up_proj.forward(x)?)?;
        self.down_proj.forward(&x)
    }
}

struct RotaryEmbedding {
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

struct Attention {
    q_proj: Linear,
    k_proj: Linear,
    v_proj: Linear,
    o_proj: Linear,
    num_heads: usize,
    num_kv_heads: usize,
    head_dim: usize,
    rotary: RotaryEmbedding,
}

impl Attention {
    fn new(cfg: &Config, vb: VarBuilder) -> Result<Self> {
        let q_proj = candle_nn::linear_no_bias(cfg.hidden_size, cfg.num_attention_heads * cfg.head_dim, vb.pp("q_proj"))?;
        let k_proj = candle_nn::linear_no_bias(cfg.hidden_size, cfg.num_key_value_heads * cfg.head_dim, vb.pp("k_proj"))?;
        let v_proj = candle_nn::linear_no_bias(cfg.hidden_size, cfg.num_key_value_heads * cfg.head_dim, vb.pp("v_proj"))?;
        let o_proj = candle_nn::linear_no_bias(cfg.num_attention_heads * cfg.head_dim, cfg.hidden_size, vb.pp("o_proj"))?;
        let rotary = RotaryEmbedding::new(cfg, vb.device())?;
        Ok(Self {
            q_proj,
            k_proj,
            v_proj,
            o_proj,
            num_heads: cfg.num_attention_heads,
            num_kv_heads: cfg.num_key_value_heads,
            head_dim: cfg.head_dim,
            rotary,
        })
    }

    fn forward(&self, x: &Tensor, index: usize, mask: Option<&Tensor>) -> Result<Tensor> {
        let (b_sz, seq_len, _) = x.dims3()?;
        let q = self.q_proj.forward(x)?;
        let k = self.k_proj.forward(x)?;
        let v = self.v_proj.forward(x)?;

        let q = q.reshape((b_sz, seq_len, self.num_heads, self.head_dim))?.transpose(1, 2)?;
        let k = k.reshape((b_sz, seq_len, self.num_kv_heads, self.head_dim))?.transpose(1, 2)?;
        let v = v.reshape((b_sz, seq_len, self.num_kv_heads, self.head_dim))?.transpose(1, 2)?;

        let q = self.rotary.apply(&q, index)?;
        let k = self.rotary.apply(&k, index)?;

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

struct DecoderLayer {
    self_attn: Attention,
    mlp: MLP,
    input_layernorm: RmsNorm,
    post_attention_layernorm: RmsNorm,
    ple_embedding: Option<candle_nn::Embedding>,
    ple_proj: Option<Linear>,
    ple_gate: Option<Linear>,
}

impl DecoderLayer {
    fn new(cfg: &Config, vb: VarBuilder) -> Result<Self> {
        let self_attn = Attention::new(cfg, vb.pp("self_attn"))?;
        let mlp = MLP::new(cfg, vb.pp("mlp"))?;
        let input_layernorm = rms_norm(cfg.hidden_size, cfg.rms_norm_eps, vb.pp("input_layernorm"))?;
        let post_attention_layernorm = rms_norm(cfg.hidden_size, cfg.rms_norm_eps, vb.pp("post_attention_layernorm"))?;
        
        let ple_embedding = if cfg.hidden_size_per_layer_input > 0 {
            Some(candle_nn::embedding(cfg.vocab_size, cfg.hidden_size_per_layer_input, vb.pp("ple_embedding"))?)
        } else {
            None
        };
        
        let ple_proj = if cfg.hidden_size_per_layer_input > 0 {
            Some(candle_nn::linear_no_bias(cfg.hidden_size_per_layer_input, cfg.hidden_size, vb.pp("ple_proj"))?)
        } else {
            None
        };

        let ple_gate = if cfg.hidden_size_per_layer_input > 0 {
            Some(candle_nn::linear(cfg.hidden_size, cfg.hidden_size, vb.pp("ple_gate"))?)
        } else {
            None
        };

        Ok(Self {
            self_attn, mlp, input_layernorm, post_attention_layernorm,
            ple_embedding, ple_proj, ple_gate,
        })
    }

    fn forward(&self, x: &Tensor, tokens: &Tensor, index: usize, mask: Option<&Tensor>) -> Result<Tensor> {
        let mut x = x.clone();
        if let (Some(emb), Some(proj), Some(gate)) = (&self.ple_embedding, &self.ple_proj, &self.ple_gate) {
            let ple_x = emb.forward(tokens)?;
            let ple_proj = proj.forward(&ple_x)?;
            let g = candle_nn::ops::sigmoid(&gate.forward(&x)?)?;
            x = x.add(&(g * ple_proj)?)?;
        }

        let residual = x.clone();
        let x = self.input_layernorm.forward(&x)?;
        let x = self.self_attn.forward(&x, index, mask)?;
        let x = x.add(&residual)?;

        let residual = x.clone();
        let x = self.post_attention_layernorm.forward(&x)?;
        let x = self.mlp.forward(&x)?;
        let x = x.add(&residual)?;
        Ok(x)
    }
}

pub struct Model {
    embed_tokens: candle_nn::Embedding,
    layers: Vec<DecoderLayer>,
    norm: RmsNorm,
    lm_head: Linear,
    device: Device,
}

impl Model {
    pub fn new(cfg: &Config, vb: VarBuilder) -> Result<Self> {
        let embed_tokens = candle_nn::embedding(cfg.vocab_size, cfg.hidden_size, vb.pp("model.embed_tokens"))?;
        let mut layers = Vec::with_capacity(cfg.num_hidden_layers);
        let vb_layers = vb.pp("model.layers");
        for i in 0..cfg.num_hidden_layers {
            layers.push(DecoderLayer::new(cfg, vb_layers.pp(i))?);
        }
        let norm = rms_norm(cfg.hidden_size, cfg.rms_norm_eps, vb.pp("model.norm"))?;
        let lm_head = candle_nn::linear_no_bias(cfg.hidden_size, cfg.vocab_size, vb.pp("lm_head"))?;
        Ok(Self { embed_tokens, layers, norm, lm_head, device: vb.device().clone() })
    }

    pub fn forward(&self, tokens: &Tensor, index: usize) -> Result<Tensor> {
        let mut x = self.embed_tokens.forward(tokens)?;
        let (_, seq_len) = tokens.dims2()?;
        let mask = if seq_len > 1 {
            let mask_vec: Vec<_> = (0..seq_len)
                .flat_map(|i| (0..seq_len).map(move |j| if i < j { f32::NEG_INFINITY } else { 0f32 }))
                .collect();
            Some(Tensor::from_vec(mask_vec, (seq_len, seq_len), &self.device)?)
        } else {
            None
        };

        for layer in &self.layers {
            x = layer.forward(&x, tokens, index, mask.as_ref())?;
        }
        let x = self.norm.forward(&x)?;
        let x = x.i((.., seq_len - 1, ..))?;
        self.lm_head.forward(&x)
    }
}
