fn main() {
    // Windows gives the main thread a 1 MiB stack (macOS: 8 MiB). Release-mode inlining
    // produces >350 KiB Tauri command futures that are constructed on the main thread
    // during IPC dispatch, which overflowed the default stack (WER APPCRASH 0xc00000fd,
    // "thread 'main' has overflowed its stack"). Reserve 8 MiB to match macOS headroom.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        println!("cargo:rustc-link-arg-bins=/STACK:8388608");
    }
    tauri_build::build()
}
