use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn updater_runtime_is_not_registered() {
    let lib_rs = fs::read_to_string(repo_root().join("src/lib.rs")).unwrap();
    assert!(
        !lib_rs.contains("tauri_plugin_updater"),
        "updater plugin must stay disabled until signing and artifact trust are solved"
    );
}

#[test]
fn tauri_config_has_no_updater_endpoint() {
    let config = fs::read_to_string(repo_root().join("tauri.conf.json")).unwrap();
    assert!(
        !config.contains("\"updater\"") && !config.contains("latest.json"),
        "tauri config must not advertise a broken updater endpoint"
    );
}

#[test]
fn updater_frontend_dependency_is_removed() {
    let package_json = fs::read_to_string(repo_root().parent().unwrap().join("package.json")).unwrap();
    assert!(
        !package_json.contains("@tauri-apps/plugin-updater"),
        "frontend updater dependency should be absent while manual releases are the product contract"
    );
}
