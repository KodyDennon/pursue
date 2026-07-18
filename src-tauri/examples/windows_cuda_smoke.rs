#[cfg(all(target_os = "windows", feature = "cuda"))]
fn main() -> anyhow::Result<()> {
    use anyhow::Context;
    use candle_core::{DType, Device, Tensor};
    use ort::{session::Session, value::Value};

    let model = std::env::args_os()
        .nth(1)
        .map(std::path::PathBuf::from)
        .context("usage: windows_cuda_smoke <onnx-model>")?;

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

    // Disable ONNX Runtime's implicit CPU provider. A successful session and inference
    // therefore proves that the packaged CUDA/cuDNN dependency chain is usable.
    let cuda = ort::ep::CUDA::default()
        .with_device_id(0)
        .build()
        .error_on_failure();
    let mut session = Session::builder()
        .map_err(|error| anyhow::anyhow!(error.to_string()))?
        .with_config_entry("session.disable_cpu_ep_fallback", "1")
        .map_err(|error| anyhow::anyhow!(error.to_string()))?
        .with_execution_providers([cuda])
        .map_err(|error| anyhow::anyhow!(error.to_string()))?
        .commit_from_file(&model)
        .map_err(|error| anyhow::anyhow!(error.to_string()))
        .with_context(|| format!("CUDA provider could not load {}", model.display()))?;
    let input_name = session
        .inputs()
        .first()
        .context("ONNX model has no inputs")?
        .name()
        .to_string();
    let input = Value::from_array((vec![1, 3, 64, 64], vec![0.0f32; 1 * 3 * 64 * 64]))?;
    let outputs = session
        .run(ort::inputs![input_name.as_str() => input])
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    anyhow::ensure!(outputs.len() > 0, "CUDA inference returned no outputs");

    println!("Candle CUDA kernel: PASS");
    println!("ONNX Runtime CUDA inference: PASS");
    Ok(())
}

#[cfg(not(all(target_os = "windows", feature = "cuda")))]
fn main() {
    eprintln!("windows_cuda_smoke requires Windows and --features cuda");
    std::process::exit(2);
}
