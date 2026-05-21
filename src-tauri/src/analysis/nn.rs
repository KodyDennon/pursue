use super::gemma4::Config;
use candle_core::{DType, Device, Result, Tensor, D};
use candle_nn::{rms_norm, Linear, Module, RmsNorm, VarBuilder};
use log::debug;

#[derive(Clone)]
pub struct KVCache {
    pub k: Option<Tensor>,
    pub v: Option<Tensor>,
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

impl Default for KVCache {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Mlp {
    pub gate_proj: Linear,
    pub up_proj: Linear,
    pub down_proj: Linear,
}

impl Mlp {
    pub fn new(cfg: &Config, vb: VarBuilder) -> Result<Self> {
        debug!(
            "[Gemma4] Initializing MLP with hidden_size: {}, intermediate_size: {}",
            cfg.hidden_size, cfg.intermediate_size
        );
        let gate_proj =
            candle_nn::linear_no_bias(cfg.hidden_size, cfg.intermediate_size, vb.pp("gate_proj"))?;
        let up_proj =
            candle_nn::linear_no_bias(cfg.hidden_size, cfg.intermediate_size, vb.pp("up_proj"))?;
        let down_proj =
            candle_nn::linear_no_bias(cfg.intermediate_size, cfg.hidden_size, vb.pp("down_proj"))?;
        Ok(Self {
            gate_proj,
            up_proj,
            down_proj,
        })
    }
}

impl Module for Mlp {
    fn forward(&self, x: &Tensor) -> Result<Tensor> {
        let x = (self.gate_proj.forward(x)?.gelu()? * self.up_proj.forward(x)?)?;
        self.down_proj.forward(&x)
    }
}

pub struct RotaryEmbedding {
    sin: Tensor,
    cos: Tensor,
}

impl RotaryEmbedding {
    pub fn new(cfg: &Config, device: &Device) -> Result<Self> {
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

    pub fn apply(&self, x: &Tensor, index: usize) -> Result<Tensor> {
        let (_b_sz, _n_heads, seq_len, head_dim) = x.dims4()?;
        let cos = self
            .cos
            .narrow(0, index, seq_len)?
            .reshape((1, 1, seq_len, head_dim / 2))?;
        let sin = self
            .sin
            .narrow(0, index, seq_len)?
            .reshape((1, 1, seq_len, head_dim / 2))?;

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
        let q_out_dim = vb
            .pp("q_proj")
            .get(
                (cfg.hidden_size, cfg.num_attention_heads * cfg.head_dim),
                "weight",
            )?
            .dim(0)?;
        let q_out_dim = if q_out_dim != cfg.num_attention_heads * cfg.head_dim {
            vb.pp("q_proj")
                .get(
                    (cfg.hidden_size, cfg.num_attention_heads * cfg.head_dim * 2),
                    "weight",
                )?
                .dim(0)?
        } else {
            q_out_dim
        };

        let k_out_dim = vb
            .pp("k_proj")
            .get(
                (cfg.hidden_size, cfg.num_key_value_heads * cfg.head_dim),
                "weight",
            )?
            .dim(0)?;
        let v_out_dim = vb
            .pp("v_proj")
            .get(
                (cfg.hidden_size, cfg.num_key_value_heads * cfg.head_dim),
                "weight",
            )?
            .dim(0)?;

        let q_proj = candle_nn::linear_no_bias(cfg.hidden_size, q_out_dim, vb.pp("q_proj"))?;
        let k_proj = candle_nn::linear_no_bias(cfg.hidden_size, k_out_dim, vb.pp("k_proj"))?;
        let v_proj = candle_nn::linear_no_bias(cfg.hidden_size, v_out_dim, vb.pp("v_proj"))?;
        let o_proj = candle_nn::linear_no_bias(q_out_dim, cfg.hidden_size, vb.pp("o_proj"))?;

        let mut actual_head_dim = q_out_dim / cfg.num_attention_heads;

        let q_norm = if vb.contains_tensor("q_norm.weight") {
            let norm_dim = vb.pp("q_norm").get(cfg.head_dim, "weight")?.dim(0)?;
            actual_head_dim = norm_dim;
            Some(rms_norm(norm_dim, cfg.rms_norm_eps, vb.pp("q_norm"))?)
        } else {
            None
        };

        let k_norm = if vb.contains_tensor("k_norm.weight") {
            let norm_dim = vb.pp("k_norm").get(cfg.head_dim, "weight")?.dim(0)?;
            Some(rms_norm(norm_dim, cfg.rms_norm_eps, vb.pp("k_norm"))?)
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
            num_heads: q_out_dim / actual_head_dim,
            num_kv_heads: k_out_dim / actual_head_dim,
            head_dim: actual_head_dim,
            rotary,
        })
    }

    pub fn forward(
        &self,
        x: &Tensor,
        index: usize,
        mask: Option<&Tensor>,
        cache: &mut KVCache,
    ) -> Result<Tensor> {
        let (b_sz, seq_len, _) = x.dims3()?;
        let mut q = self.q_proj.forward(x)?;
        let mut k = self.k_proj.forward(x)?;
        let v = self.v_proj.forward(x)?;

        if let (Some(q_n), Some(k_n)) = (&self.q_norm, &self.k_norm) {
            q = q.reshape((b_sz, seq_len, self.num_heads, self.head_dim))?;
            q = q_n.forward(&q)?.reshape((b_sz, seq_len, ()))?;
            k = k.reshape((b_sz, seq_len, self.num_kv_heads, self.head_dim))?;
            k = k_n.forward(&k)?.reshape((b_sz, seq_len, ()))?;
        }

        let q = q
            .reshape((b_sz, seq_len, self.num_heads, self.head_dim))?
            .transpose(1, 2)?;
        let k = k
            .reshape((b_sz, seq_len, self.num_kv_heads, self.head_dim))?
            .transpose(1, 2)?;
        let v = v
            .reshape((b_sz, seq_len, self.num_kv_heads, self.head_dim))?
            .transpose(1, 2)?;

        let q = self.rotary.apply(&q, index)?;
        let k = self.rotary.apply(&k, index)?;

        let (k, v) = cache.append(&k, &v)?;

        let k = if self.num_heads != self.num_kv_heads {
            let n_rep = self.num_heads / self.num_kv_heads;
            k.unsqueeze(2)?
                .expand((b_sz, self.num_kv_heads, n_rep, k.dim(2)?, self.head_dim))?
                .reshape((b_sz, self.num_heads, k.dim(2)?, self.head_dim))?
        } else {
            k
        };

        let v = if self.num_heads != self.num_kv_heads {
            let n_rep = self.num_heads / self.num_kv_heads;
            v.unsqueeze(2)?
                .expand((b_sz, self.num_kv_heads, n_rep, v.dim(2)?, self.head_dim))?
                .reshape((b_sz, self.num_heads, v.dim(2)?, self.head_dim))?
        } else {
            v
        };

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
    pub mlp: Mlp,
    pub input_layernorm: RmsNorm,
    pub post_attention_layernorm: RmsNorm,
    pub per_layer_input_gate: Option<Linear>,
    pub per_layer_projection: Option<Linear>,
    pub post_per_layer_input_norm: Option<RmsNorm>,
}

impl DecoderLayer {
    pub fn new(cfg: &Config, vb: VarBuilder) -> Result<Self> {
        let self_attn = Attention::new(cfg, vb.pp("self_attn"))?;
        let mlp = Mlp::new(cfg, vb.pp("mlp"))?;
        let input_layernorm =
            rms_norm(cfg.hidden_size, cfg.rms_norm_eps, vb.pp("input_layernorm"))?;
        let post_attention_layernorm = rms_norm(
            cfg.hidden_size,
            cfg.rms_norm_eps,
            vb.pp("post_attention_layernorm"),
        )?;

        let per_layer_input_gate = if cfg.hidden_size_per_layer_input > 0 {
            Some(candle_nn::linear_no_bias(
                cfg.hidden_size,
                cfg.hidden_size_per_layer_input,
                vb.pp("per_layer_input_gate"),
            )?)
        } else {
            None
        };

        let per_layer_projection = if cfg.hidden_size_per_layer_input > 0 {
            Some(candle_nn::linear_no_bias(
                cfg.hidden_size_per_layer_input,
                cfg.hidden_size,
                vb.pp("per_layer_projection"),
            )?)
        } else {
            None
        };

        let post_per_layer_input_norm = if cfg.hidden_size_per_layer_input > 0 {
            Some(rms_norm(
                cfg.hidden_size,
                cfg.rms_norm_eps,
                vb.pp("post_per_layer_input_norm"),
            )?)
        } else {
            None
        };

        Ok(Self {
            self_attn,
            mlp,
            input_layernorm,
            post_attention_layernorm,
            per_layer_input_gate,
            per_layer_projection,
            post_per_layer_input_norm,
        })
    }

    pub fn forward(
        &self,
        x: &Tensor,
        ple_x: Option<&Tensor>,
        index: usize,
        mask: Option<&Tensor>,
        cache: &mut KVCache,
    ) -> Result<Tensor> {
        let mut x = x.clone();
        if let (Some(px), Some(proj), Some(gate)) = (
            ple_x,
            &self.per_layer_projection,
            &self.per_layer_input_gate,
        ) {
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
