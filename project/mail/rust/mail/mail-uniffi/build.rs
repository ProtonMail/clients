use std::path::{Path, PathBuf};

fn main() {
    gen_package_version_info();
    setup_x86_64_android_workaround();
}

/// Prefer NDK LLVM prebuilt that matches this machine (Apple Silicon may use `darwin-arm64` or a
/// universal `darwin-x86_64` tree only). Fall back across known host triples.
fn resolve_ndk_prebuilt_host(android_ndk_home: &Path) -> String {
    let prebuilt_root = android_ndk_home.join("toolchains/llvm/prebuilt");
    let candidates: &[&str] = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => &["darwin-arm64", "darwin-x86_64"],
        ("macos", _) => &["darwin-x86_64"],
        ("linux", "aarch64") => &["linux-aarch64", "linux-x86_64"],
        ("linux", _) => &["linux-x86_64"],
        ("windows", _) => &["windows-x86_64"],
        _ => panic!("Unsupported host OS for Android NDK builds. Use Linux, macOS, or Windows."),
    };
    for host in candidates {
        if prebuilt_root.join(host).join("lib/clang").is_dir() {
            return (*host).to_string();
        }
    }
    panic!(
        "No LLVM prebuilt under {}. Tried {:?}. Set ANDROID_NDK_HOME to a full NDK (r28+).",
        prebuilt_root.display(),
        candidates
    );
}

/// NDK `lib/clang/<ver>/lib/linux` uses a major-only folder (`19`, `20`, …) that changes with the
/// NDK release. Prefer `NDK_CLANG_VERSION` when set; otherwise pick the newest version that ships
/// `libclang_rt.builtins-x86_64-android.a`.
fn resolve_clang_linux_lib_dir(android_ndk_home: &Path, prebuilt_host: &str) -> PathBuf {
    let clang_root = android_ndk_home
        .join("toolchains/llvm/prebuilt")
        .join(prebuilt_host)
        .join("lib/clang");

    let builtins_name = "libclang_rt.builtins-x86_64-android.a";

    if let Ok(v) = std::env::var("NDK_CLANG_VERSION") {
        let p = clang_root.join(&v).join("lib/linux");
        if p.join(builtins_name).is_file() {
            return p;
        }
        panic!(
            "NDK_CLANG_VERSION={v} — missing {builtins_name} under {}",
            p.display()
        );
    }

    let mut versions: Vec<String> = std::fs::read_dir(&clang_root)
        .unwrap_or_else(|e| panic!("read {}: {e}", clang_root.display()))
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|name| name.chars().next().is_some_and(|c| c.is_ascii_digit()))
        .collect();

    versions.sort_by_key(|s| {
        s.split('.')
            .next()
            .and_then(|x| x.parse::<u32>().ok())
            .unwrap_or(0)
    });

    for v in versions.iter().rev() {
        let p = clang_root.join(v).join("lib/linux");
        if p.join(builtins_name).is_file() {
            return p;
        }
    }

    panic!(
        "Could not find lib/clang/*/lib/linux/{builtins_name} under {}. \
Set NDK_CLANG_VERSION to the directory name under lib/clang (e.g. 20 for recent NDKs).",
        clang_root.display()
    );
}

fn setup_x86_64_android_workaround() {
    // FIXME: hack to ensure that libs compile correctly for android x86_64bit emulator versions
    //        see https://github.com/rusqlite/rusqlite/issues/1380#issuecomment-1689765485
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").expect("CARGO_CFG_TARGET_OS not set");
    let target_arch =
        std::env::var("CARGO_CFG_TARGET_ARCH").expect("CARGO_CFG_TARGET_ARCH not set");
    if target_arch == "x86_64" && target_os == "android" {
        let android_ndk_home = std::env::var("ANDROID_NDK_HOME").expect("ANDROID_NDK_HOME not set");
        let ndk_path = PathBuf::from(android_ndk_home);
        let prebuilt_host = resolve_ndk_prebuilt_host(&ndk_path);
        let linux_lib_dir = resolve_clang_linux_lib_dir(&ndk_path, &prebuilt_host);

        println!(
            "cargo:rustc-link-search={}",
            linux_lib_dir.to_string_lossy()
        );
        println!("cargo:rustc-link-lib=static=clang_rt.builtins-x86_64-android");
    }
}

// Generate package version info that can be accessed at runtime.
fn gen_package_version_info() {
    let major = std::env::var("CARGO_PKG_VERSION_MAJOR").expect("CARGO_PKG_VERSION_MAJOR not set");
    let minor = std::env::var("CARGO_PKG_VERSION_MINOR").expect("CARGO_PKG_VERSION_MINOR not set");
    let patch = std::env::var("CARGO_PKG_VERSION_PATCH").expect("CARGO_PKG_VERSION_PATCH not set");

    let output_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR not set"))
        .join("package_version.rs");

    let data = format!(
        r#"
pub const VERSION_MAJOR:u32 = {major};
pub const VERSION_MINOR:u32 = {minor};
pub const VERSION_PATCH:u32 = {patch};
pub const VERSION_STRING:&str = "{major}.{minor}.{patch}";
    "#
    );

    std::fs::write(&output_dir, data).expect("Could not write to version file");
}
