fn main() {
    // Windows gives the main thread a 1 MiB stack (macOS: 8 MiB). Release-mode inlining
    // produces >350 KiB Tauri command futures that are constructed on the main thread
    // during IPC dispatch, which overflowed the default stack (WER APPCRASH 0xc00000fd,
    // "thread 'main' has overflowed its stack"). Reserve 8 MiB to match macOS headroom.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        println!("cargo:rustc-link-arg-bins=/STACK:8388608");
        // Cargo's generated test harness does not inherit Tauri's application
        // manifest. Without an explicit v6 common-controls activation context,
        // Windows loads comctl32 5.82 and rejects the test process before main
        // because Tauri/rfd imports TaskDialogIndirect (available in v6).
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg=/MANIFESTDEPENDENCY:type='win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'"
        );
        // tauri-build links its own resource manifest into the GUI executable.
        // Disable link.exe's generated manifest for binaries only, after the
        // crate-wide arguments above, so that resource remains the single ID 1.
        println!("cargo:rustc-link-arg-bins=/MANIFEST:NO");
    }
    tauri_build::build()
}
