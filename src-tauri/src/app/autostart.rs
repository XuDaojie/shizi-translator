//! 开机自启：Windows 写 HKCU Run；其它平台暂 no-op。
//! 启用时注册带 `--autostart` 的启动命令，登录后仅托盘驻留、不弹主窗。

use std::path::Path;

/// 自启进程参数：冷启动时前端跳过强制 show。
pub const AUTOSTART_ARG: &str = "--autostart";

const RUN_VALUE_NAME: &str = "Shizi";

/// 当前进程是否由开机自启拉起。
pub fn is_autostart_process() -> bool {
    std::env::args().any(|arg| arg == AUTOSTART_ARG)
}

/// 生成写入 Run 键的命令行（路径加引号，附带 `--autostart`）。
pub fn build_run_command(exe: &Path) -> String {
    format!("\"{}\" {AUTOSTART_ARG}", exe.display())
}

/// 按配置同步系统开机自启项。失败返回可读错误。
pub fn apply(enabled: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        apply_windows(enabled)
    }
    #[cfg(not(windows))]
    {
        let _ = enabled;
        Ok(())
    }
}

#[cfg(windows)]
fn apply_windows(enabled: bool) -> Result<(), String> {
    if enabled {
        let exe = std::env::current_exe().map_err(|e| format!("无法获取程序路径: {e}"))?;
        let command = build_run_command(&exe);
        set_run_value(&command)
    } else {
        delete_run_value()
    }
}

#[cfg(windows)]
fn open_run_key_for_write() -> Result<windows::Win32::System::Registry::HKEY, String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::ERROR_SUCCESS;
    use windows::Win32::System::Registry::{
        RegOpenKeyExW, HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE,
    };

    let subkey: Vec<u16> = std::ffi::OsStr::new(r"Software\Microsoft\Windows\CurrentVersion\Run")
        .encode_wide()
        .chain(Some(0))
        .collect();

    unsafe {
        let mut hkey = HKEY::default();
        let status = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            0,
            KEY_SET_VALUE,
            &mut hkey,
        );
        if status != ERROR_SUCCESS {
            return Err(format!("无法打开开机启动注册表项: {status:?}"));
        }
        Ok(hkey)
    }
}

#[cfg(windows)]
fn set_run_value(command: &str) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::ERROR_SUCCESS;
    use windows::Win32::System::Registry::{RegCloseKey, RegSetValueExW, REG_SZ};

    let value_name: Vec<u16> = std::ffi::OsStr::new(RUN_VALUE_NAME)
        .encode_wide()
        .chain(Some(0))
        .collect();
    let data: Vec<u16> = std::ffi::OsStr::new(command)
        .encode_wide()
        .chain(Some(0))
        .collect();
    let data_bytes =
        unsafe { std::slice::from_raw_parts(data.as_ptr() as *const u8, data.len() * 2) };

    let hkey = open_run_key_for_write()?;
    unsafe {
        let set = RegSetValueExW(hkey, PCWSTR(value_name.as_ptr()), 0, REG_SZ, Some(data_bytes));
        let _ = RegCloseKey(hkey);
        if set != ERROR_SUCCESS {
            return Err(format!("无法写入开机启动项: {set:?}"));
        }
    }
    Ok(())
}

#[cfg(windows)]
fn delete_run_value() -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
    use windows::Win32::System::Registry::{RegCloseKey, RegDeleteValueW};

    let value_name: Vec<u16> = std::ffi::OsStr::new(RUN_VALUE_NAME)
        .encode_wide()
        .chain(Some(0))
        .collect();

    let hkey = match open_run_key_for_write() {
        Ok(k) => k,
        Err(_) => return Ok(()), // 键不存在则视为已关闭
    };
    unsafe {
        let del = RegDeleteValueW(hkey, PCWSTR(value_name.as_ptr()));
        let _ = RegCloseKey(hkey);
        if del != ERROR_SUCCESS && del != ERROR_FILE_NOT_FOUND {
            return Err(format!("无法删除开机启动项: {del:?}"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn build_run_command_quotes_path_and_appends_arg() {
        let exe = PathBuf::from(r"C:\Program Files\Shizi\shizi.exe");
        assert_eq!(
            build_run_command(&exe),
            r#""C:\Program Files\Shizi\shizi.exe" --autostart"#
        );
    }

    #[test]
    fn autostart_arg_constant() {
        assert_eq!(AUTOSTART_ARG, "--autostart");
    }
}
