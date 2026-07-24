fn main() {
    // Windows：tauri.conf bundle.resources 指向 resources/popup-native，路径必须存在
    #[cfg(windows)]
    {
        ensure_popup_native_resources_dir();
    }

    tauri_build::build();

    // Windows：可选编译 WinUI 弹窗；失败仅 warn，不阻断 Rust 构建。
    #[cfg(windows)]
    {
        build_winui_popup_best_effort();
    }
}

#[cfg(windows)]
use std::path::{Path, PathBuf};

/// 保证 `resources/popup-native` 存在，避免 tauri_build 因缺失资源路径失败。
#[cfg(windows)]
fn ensure_popup_native_resources_dir() {
    use std::fs;

    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default());
    let dest = manifest_dir.join("resources").join("popup-native");
    if dest.is_dir() {
        return;
    }
    if let Err(e) = fs::create_dir_all(&dest) {
        println!("cargo:warning=无法创建 {}: {e}", dest.display());
        return;
    }
    let marker = dest.join(".shizi-popup-native-placeholder");
    let _ = fs::write(
        marker,
        "Placeholder so tauri bundle.resources path exists. Run npm run popup-native:copy to fill.\n",
    );
}

#[cfg(windows)]
fn build_winui_popup_best_effort() {
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

    let popup_root = csproj.parent().unwrap().to_path_buf();
    let win_x64 = popup_root
        .join("bin")
        .join("x64")
        .join("Release")
        .join("net8.0-windows10.0.19041.0")
        .join("win-x64");
    let release_exe = win_x64.join("Shizi.Popup.exe");

    let force = std::env::var("SHIZI_BUILD_WINUI").ok().as_deref() == Some("1");
    let need_build = force || !release_exe.is_file();

    println!("cargo:rerun-if-changed={}", csproj.to_string_lossy());
    println!("cargo:rerun-if-env-changed=SHIZI_BUILD_WINUI");
    println!("cargo:rerun-if-env-changed=PROFILE");

    if need_build {
        println!("cargo:warning=正在可选构建 Shizi.Popup（SHIZI_BUILD_WINUI=1 可强制）…");

        match Command::new("dotnet")
            .args([
                "build",
                &csproj.to_string_lossy(),
                "-c",
                "Release",
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
                return;
            }
            Err(e) => {
                println!("cargo:warning=无法运行 dotnet build: {e}");
                return;
            }
        }
    }

    if !release_exe.is_file() {
        return;
    }

    // Release profile：拷到 resources（供 tauri bundle）
    let is_release = std::env::var("PROFILE").ok().as_deref() == Some("release");
    if is_release || force {
        let resources_dest = manifest_dir.join("resources").join("popup-native");
        if let Err(e) = copy_dir_best_effort(&win_x64, &resources_dest) {
            println!("cargo:warning=拷贝 popup-native → resources 失败: {e}");
        } else {
            println!(
                "cargo:warning=已同步 popup-native → {}",
                resources_dest.display()
            );
        }
    }

    // best-effort 同步到 target/<profile>/popup-native（current_exe 旁）
    if let Ok(target_dir) = std::env::var("CARGO_TARGET_DIR") {
        let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".into());
        let dest = PathBuf::from(target_dir)
            .join(&profile)
            .join("popup-native");
        if let Err(e) = copy_dir_best_effort(&win_x64, &dest) {
            println!("cargo:warning=拷贝 popup-native → target/{profile} 失败: {e}");
        }
    } else {
        let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".into());
        let dest = manifest_dir
            .join("target")
            .join(&profile)
            .join("popup-native");
        if dest.parent().is_some_and(|p| p.is_dir()) {
            if let Err(e) = copy_dir_best_effort(&win_x64, &dest) {
                println!("cargo:warning=拷贝 popup-native → target/{profile} 失败: {e}");
            }
        }
    }
}

#[cfg(windows)]
fn copy_dir_best_effort(src: &Path, dest: &Path) -> Result<(), String> {
    use std::fs;

    if !src.is_dir() {
        return Err(format!("源目录不存在: {}", src.display()));
    }

    // 已同步且 exe 时间戳不新于目标则跳过（粗略）
    let src_exe = src.join("Shizi.Popup.exe");
    let dest_exe = dest.join("Shizi.Popup.exe");
    if src_exe.is_file() && dest_exe.is_file() {
        if let (Ok(s), Ok(d)) = (fs::metadata(&src_exe), fs::metadata(&dest_exe)) {
            if let (Ok(st), Ok(dt)) = (s.modified(), d.modified()) {
                if st <= dt {
                    return Ok(());
                }
            }
        }
    }

    if dest.exists() {
        fs::remove_dir_all(dest).map_err(|e| format!("清理旧目录: {e}"))?;
    }
    copy_dir_all(src, dest).map_err(|e| format!("复制: {e}"))?;
    Ok(())
}

#[cfg(windows)]
fn copy_dir_all(src: &Path, dest: &Path) -> std::io::Result<()> {
    use std::fs;

    fs::create_dir_all(dest)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let to = dest.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &to)?;
        } else {
            fs::copy(entry.path(), to)?;
        }
    }
    Ok(())
}
