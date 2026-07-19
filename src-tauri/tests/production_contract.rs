use std::fs;
use std::path::PathBuf;

fn tauri_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn repository_root() -> PathBuf {
    tauri_root().parent().unwrap().to_path_buf()
}

#[test]
fn updater_runtime_and_frontend_are_registered() {
    let lib_rs = fs::read_to_string(tauri_root().join("src/lib.rs")).unwrap();
    let package_json = fs::read_to_string(repository_root().join("package.json")).unwrap();
    let capability = fs::read_to_string(tauri_root().join("capabilities/default.json")).unwrap();

    assert!(lib_rs.contains("tauri_plugin_updater::Builder"));
    assert!(package_json.contains("@tauri-apps/plugin-updater"));
    assert!(capability.contains("updater:default"));
}

#[test]
fn hugging_face_credentials_never_use_plaintext_app_settings() {
    let system =
        fs::read_to_string(repository_root().join("src-tauri/src/commands/system.rs")).unwrap();
    let model_manager =
        fs::read_to_string(repository_root().join("src-tauri/src/analysis/model_manager.rs"))
            .unwrap();
    let onboarding =
        fs::read_to_string(repository_root().join("src/lib/components/FirstLaunch.svelte"))
            .unwrap();

    assert!(system.contains("set_hugging_face_manual_token"));
    assert!(system.contains("if key == \"huggingface_token\""));
    assert!(model_manager.contains("store_hf_manual_token"));
    assert!(model_manager.contains("DELETE FROM app_settings WHERE key = 'huggingface_token'"));
    assert!(onboarding.contains("begin_hugging_face_device_auth"));
    assert!(onboarding.contains("set_hugging_face_manual_token"));
    assert!(!onboarding.contains("key: 'huggingface_token'"));
}

#[test]
fn updater_config_requires_signed_artifacts_and_https() {
    let config: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(tauri_root().join("tauri.conf.json")).unwrap())
            .unwrap();

    assert_eq!(config["bundle"]["createUpdaterArtifacts"], true);
    let pubkey = config["plugins"]["updater"]["pubkey"]
        .as_str()
        .unwrap_or_default();
    assert!(pubkey.len() > 100, "embedded updater public key is missing");
    let endpoints = config["plugins"]["updater"]["endpoints"]
        .as_array()
        .expect("updater endpoints");
    assert!(!endpoints.is_empty());
    assert!(endpoints.iter().all(|endpoint| endpoint
        .as_str()
        .is_some_and(|value| value.starts_with("https://"))));
}

#[test]
fn update_targets_keep_gpu_provider_lanes_separate() {
    let system = fs::read_to_string(tauri_root().join("src/commands/system.rs")).unwrap();
    for target in [
        "windows-cuda-x86_64",
        "windows-directml-x86_64",
        "macos-metal-aarch64",
    ] {
        assert!(system.contains(target), "missing updater lane {target}");
    }
}

#[test]
fn installer_hooks_preserve_all_user_data() {
    let hooks = fs::read_to_string(tauri_root().join("installer.nsh")).unwrap();
    assert!(hooks.contains("NSIS_HOOK_PREINSTALL"));
    assert!(hooks.contains("NSIS_HOOK_PREUNINSTALL"));
    assert!(!hooks.contains("RMDir"));
    assert!(!hooks.contains("Delete"));
}

#[test]
fn release_workflow_publishes_every_signed_update_lane() {
    let workflow =
        fs::read_to_string(repository_root().join(".github/workflows/release.yml")).unwrap();
    assert!(workflow.contains("Verify updater signing configuration"));
    assert!(workflow.contains("bundle_args: --bundles msi"));
    assert!(workflow.contains("*.msi.zip.sig"));
    assert!(workflow.contains("*.nsis.zip.sig"));
    assert!(workflow.contains("*.app.tar.gz.sig"));
    assert!(workflow.contains("generate-updater-manifest.mjs"));
    assert!(workflow.contains("needs: [installers, publish-updater]"));
}
