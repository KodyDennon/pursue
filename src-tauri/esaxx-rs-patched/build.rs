#[cfg(feature = "cpp")]
fn main() {
    let mut build = cc::Build::new();
    build.cpp(true)
        .file("src/esaxx.cpp")
        .include("src");

    if cfg!(target_os = "macos") {
        build.flag("-std=c++11")
             .flag("-stdlib=libc++");
    } else {
        build.flag_if_supported("-std=c++11");
    }

    build.compile("esaxx");
}

#[cfg(not(feature = "cpp"))]
fn main() {}
