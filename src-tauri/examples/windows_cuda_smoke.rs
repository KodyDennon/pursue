#[cfg(all(target_os = "windows", feature = "cuda"))]
fn main() -> anyhow::Result<()> {
    use anyhow::Context;
    use candle_core::{DType, Device, Tensor};
    use ort::{session::Session, value::Value};

    let model = std::env::args_os()
        .nth(1)
        .map(std::path::PathBuf::from)
        .context("usage: windows_cuda_smoke <onnx-model> <tokenizer-json>")?;
    let tokenizer_path = std::env::args_os()
        .nth(2)
        .map(std::path::PathBuf::from)
        .context("usage: windows_cuda_smoke <onnx-model> <tokenizer-json>")?;

    // Exercise a real Candle kernel and synchronize it. Device creation alone does not
    // detect a binary compiled for an incompatible GPU architecture.
    let device = Device::new_cuda(0).context("Candle could not initialize CUDA device 0")?;
    let sum = Tensor::ones((4, 4), DType::F32, &device)?
        .sum_all()?
        .to_scalar::<f32>()?;
    anyhow::ensure!(
        sum == 16.0,
        "Candle CUDA smoke result was {sum}, expected 16"
    );

    // The BGE graph contains a few lightweight shape/control nodes that CUDA EP does
    // not implement. Keep ORT's normal per-node CPU assignment, then require profiling
    // evidence that CUDA executed real graph kernels; provider initialization failures
    // remain fatal via `error_on_failure`.
    let profile_prefix = std::env::temp_dir().join("pursue-ort-cuda-profile");
    let cuda = ort::ep::CUDA::default()
        .with_device_id(0)
        .build()
        .error_on_failure();
    let mut session = Session::builder()
        .map_err(|error| anyhow::anyhow!(error.to_string()))?
        .with_profiling(&profile_prefix)
        .map_err(|error| anyhow::anyhow!(error.to_string()))?
        .with_execution_providers([cuda])
        .map_err(|error| anyhow::anyhow!(error.to_string()))?
        .commit_from_file(&model)
        .map_err(|error| anyhow::anyhow!(error.to_string()))
        .with_context(|| format!("CUDA provider could not load {}", model.display()))?;
    let tokenizer = tokenizers::Tokenizer::from_file(&tokenizer_path).map_err(|error| {
        anyhow::anyhow!(
            "failed to load tokenizer {}: {error}",
            tokenizer_path.display()
        )
    })?;
    let encoding = tokenizer
        .encode("PURSUE CUDA runtime verification", true)
        .map_err(|error| anyhow::anyhow!("tokenization failed: {error}"))?;
    let seq_len = encoding.len();
    anyhow::ensure!(seq_len > 0, "tokenizer produced an empty sequence");
    let input_ids = Value::from_array((
        vec![1, seq_len],
        encoding
            .get_ids()
            .iter()
            .map(|&value| i64::from(value))
            .collect::<Vec<_>>(),
    ))?;
    let attention_mask = Value::from_array((
        vec![1, seq_len],
        encoding
            .get_attention_mask()
            .iter()
            .map(|&value| i64::from(value))
            .collect::<Vec<_>>(),
    ))?;
    let token_type_ids = Value::from_array((
        vec![1, seq_len],
        encoding
            .get_type_ids()
            .iter()
            .map(|&value| i64::from(value))
            .collect::<Vec<_>>(),
    ))?;
    let outputs = session
        .run(ort::inputs![
            "input_ids" => input_ids,
            "attention_mask" => attention_mask,
            "token_type_ids" => token_type_ids,
        ])
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    let output = outputs
        .get("last_hidden_state")
        .or_else(|| outputs.get("sentence_embedding"))
        .context("CUDA inference returned no recognized embedding output")?;
    let (_, values) = output
        .try_extract_tensor::<f32>()
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    anyhow::ensure!(
        !values.is_empty(),
        "CUDA inference returned an empty tensor"
    );
    anyhow::ensure!(
        values.iter().all(|value| value.is_finite()),
        "CUDA inference returned a non-finite tensor"
    );
    drop(outputs);

    let profile_path = session
        .end_profiling()
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    let profile = std::fs::read_to_string(&profile_path)
        .with_context(|| format!("failed to read ONNX profile {profile_path}"))?;
    let events: serde_json::Value = serde_json::from_str(&profile)
        .with_context(|| format!("failed to parse ONNX profile {profile_path}"))?;
    let cuda_kernel_events = events
        .as_array()
        .context("ONNX profile root is not an event array")?
        .iter()
        .filter(|event| {
            event
                .get("args")
                .and_then(|args| args.get("provider"))
                .and_then(serde_json::Value::as_str)
                == Some("CUDAExecutionProvider")
        })
        .count();
    let _ = std::fs::remove_file(&profile_path);
    anyhow::ensure!(
        cuda_kernel_events > 0,
        "ONNX inference completed without any profiled CUDA kernel events"
    );

    println!("Candle CUDA kernel: PASS");
    println!("ONNX Runtime CUDA inference: PASS ({cuda_kernel_events} CUDA kernel events)");
    Ok(())
}

#[cfg(not(all(target_os = "windows", feature = "cuda")))]
fn main() {
    eprintln!("windows_cuda_smoke requires Windows and --features cuda");
    std::process::exit(2);
}
