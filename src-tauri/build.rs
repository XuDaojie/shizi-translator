fn main() {
    tauri_build::build();

    // Windows：可选编译 WinUI 弹窗；失败仅 warn，不阻断 Rust 构建。
    #[cfg(windows)]
    {
        build_winui_popup_best_effort();
    }
}

#[cfg(windows)]
fn build_winui_popup_best_effort() {
    use std::path::PathBuf;
    use std::process::Command;

    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default());
    let csproj = manifest_dir
        .join("..")
        .join("native")
        .join("windows")
        .join("popup")
        .join("Shizi.Popup.csproj");

    if !csproj.is_file() {
        println!("cargo:warning=WinUI popup csproj 不存在，跳过 dotnet build");
        return;
    }

    // 仅当显式开启时自动 build，避免每次 cargo 都拉长编译
    // 默认：若已有 Release 产物则跳过；无产物时尝试一次
    let release_exe = csproj
        .parent()
        .unwrap()
        .join("bin")
        .join("x64")
        .join("Release")
        .join("net8.0-windows10.0.19041.0")
        .join("win-x64")
        .join("Shizi.Popup.exe");

    let force = std::env::var("SHIZI_BUILD_WINUI").ok().as_deref() == Some("1");
    if release_exe.is_file() && !force {
        println!(
            "cargo:rerun-if-changed={}",
            csproj.to_string_lossy()
        );
        return;
    }

    println!("cargo:rerun-if-changed={}", csproj.to_string_lossy());
    println!("cargo:warning=正在可选构建 Shizi.Popup（SHIZI_BUILD_WINUI=1 可强制）…");

    let profile = if std::env::var("PROFILE").ok().as_deref() == Some("release") {
        "Release"
    } else {
        "Release" // 宿主优先找 Release 产物
    };

    match Command::new("dotnet")
        .args([
            "build",
            &csproj.to_string_lossy(),
            "-c",
            profile,
            "-p:Platform=x64",
            "--nologo",
            "-v",
            "q",
        ])
        .status()
    {
        Ok(status) if status.success() => {
            println!("cargo:warning=Shizi.Popup dotnet build 成功");
        }
        Ok(status) => {
            println!(
                "cargo:warning=Shizi.Popup dotnet build 失败 (exit={status})，WinUI 弹窗将不可用"
            );
        }
        Err(e) => {
            println!("cargo:warning=无法运行 dotnet build: {e}");
        }
    }
}
