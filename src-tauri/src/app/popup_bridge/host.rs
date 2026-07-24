//! WinUI 弹窗宿主：子进程 + localhost TCP JSON 行协议。
//!
//! 进程内 hostfxr 加载 WinUI/WASDK 在非 main EXE 场景不稳定，任务 8 采用 subprocess。
//! 配置 `popupUi: winui` 与 `PopupUi` trait 不变；详见 `native/README.md`。

#![cfg(windows)]

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use super::dispatch::{self, BridgeResponse};
use super::protocol::BridgeEnvelope;
use super::push;
use crate::app::state::AppState;

/// IPC 行消息（与 C# `IpcMessage` 对齐，camelCase）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IpcMessage {
    op: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    ok: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    op_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    x: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    y: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mode: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    on: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    w: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    h: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<String>,
}

enum ReaderEvent {
    Hello,
    Result {
        op_name: Option<String>,
        ok: bool,
        error: Option<String>,
    },
    Disconnected,
}

struct HostInner {
    child: Option<Child>,
    writer: Option<TcpStream>,
    result_rx: Option<Receiver<ReaderEvent>>,
    _reader_join: Option<thread::JoinHandle<()>>,
}

static HOST: OnceLock<Mutex<HostInner>> = OnceLock::new();
/// 串行化「写命令 + 等 result」，与 HOST 锁分离，避免 push 死锁。
static OP_GATE: OnceLock<Mutex<()>> = OnceLock::new();
static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

fn host_mutex() -> &'static Mutex<HostInner> {
    HOST.get_or_init(|| {
        Mutex::new(HostInner {
            child: None,
            writer: None,
            result_rx: None,
            _reader_join: None,
        })
    })
}

fn op_gate() -> &'static Mutex<()> {
    OP_GATE.get_or_init(|| Mutex::new(()))
}

pub fn set_app_handle(app: AppHandle) {
    let _ = APP_HANDLE.set(app);
}

pub fn app_handle() -> Option<&'static AppHandle> {
    APP_HANDLE.get()
}

/// 定位 `Shizi.Popup.exe`。
pub fn resolve_popup_exe() -> Option<PathBuf> {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let p = dir.join("popup-native").join("Shizi.Popup.exe");
            if p.is_file() {
                return Some(p);
            }
            let p2 = dir.join("Shizi.Popup.exe");
            if p2.is_file() {
                return Some(p2);
            }
        }
    }

    for c in dev_exe_candidates() {
        if c.is_file() {
            return Some(c);
        }
    }
    None
}

fn dev_exe_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let native = manifest
        .join("..")
        .join("native")
        .join("windows")
        .join("popup");
    for cfg in ["Release", "Debug"] {
        out.push(
            native
                .join("bin")
                .join("x64")
                .join(cfg)
                .join("net8.0-windows10.0.19041.0")
                .join("win-x64")
                .join("Shizi.Popup.exe"),
        );
        out.push(
            native
                .join("bin")
                .join(cfg)
                .join("net8.0-windows10.0.19041.0")
                .join("win-x64")
                .join("Shizi.Popup.exe"),
        );
    }
    out
}

pub fn is_running() -> bool {
    let Ok(guard) = host_mutex().lock() else {
        return false;
    };
    guard.writer.is_some()
}

/// 启动子进程并完成 hello 握手（幂等）。
pub fn ensure_process(app: &AppHandle) -> Result<(), String> {
    set_app_handle(app.clone());

    {
        let guard = host_mutex()
            .lock()
            .map_err(|_| "popup host 锁损坏".to_string())?;
        if guard.writer.is_some() {
            return Ok(());
        }
    }

    let exe = resolve_popup_exe().ok_or_else(|| {
        "未找到 Shizi.Popup.exe（请先 dotnet build native/windows/popup -c Release）".to_string()
    })?;

    spawn_and_connect(&exe)?;
    install_push_sink();
    Ok(())
}

fn spawn_and_connect(exe: &Path) -> Result<(), String> {
    let listener =
        TcpListener::bind("127.0.0.1:0").map_err(|e| format!("绑定 localhost TCP 失败: {e}"))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("读取绑定端口失败: {e}"))?
        .port();

    log::info!("popup host: 启动 {} --port {}", exe.display(), port);

    let mut child = Command::new(exe)
        .arg("--port")
        .arg(port.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("启动 Shizi.Popup 失败: {e}"))?;

    listener
        .set_nonblocking(true)
        .map_err(|e| format!("listener nonblocking: {e}"))?;
    let deadline = Instant::now() + Duration::from_secs(15);
    let stream = loop {
        match listener.accept() {
            Ok((s, _)) => break s,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() > deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("等待 Shizi.Popup 连接超时".into());
                }
                // 子进程是否已退出
                if let Ok(Some(status)) = child.try_wait() {
                    return Err(format!("Shizi.Popup 提前退出: {status}"));
                }
                thread::sleep(Duration::from_millis(50));
            }
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("accept 失败: {e}"));
            }
        }
    };
    stream
        .set_nonblocking(false)
        .map_err(|e| format!("stream blocking: {e}"))?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(60)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(10)));

    let reader_stream = stream
        .try_clone()
        .map_err(|e| format!("clone stream: {e}"))?;
    let writer = stream;

    let (tx, rx) = mpsc::channel::<ReaderEvent>();
    let join = thread::Builder::new()
        .name("shizi-popup-ipc".into())
        .spawn(move || reader_loop(reader_stream, tx))
        .map_err(|e| format!("启动 IPC 读线程失败: {e}"))?;

    let hello_deadline = Instant::now() + Duration::from_secs(15);
    loop {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(ReaderEvent::Hello) => break,
            Ok(ReaderEvent::Disconnected) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Shizi.Popup 在握手前断开".into());
            }
            Ok(ReaderEvent::Result { .. }) => {}
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if Instant::now() > hello_deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("等待 Shizi.Popup hello 超时".into());
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("IPC 读线程退出".into());
            }
        }
    }

    let mut guard = host_mutex()
        .lock()
        .map_err(|_| "popup host 锁损坏".to_string())?;
    guard.child = Some(child);
    guard.writer = Some(writer);
    guard.result_rx = Some(rx);
    guard._reader_join = Some(join);
    log::info!("popup host: 子进程已连接并完成 hello");
    Ok(())
}

fn reader_loop(stream: TcpStream, tx: Sender<ReaderEvent>) {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => {
                let _ = tx.send(ReaderEvent::Disconnected);
                break;
            }
            Ok(_) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                match serde_json::from_str::<IpcMessage>(trimmed) {
                    Ok(msg) => match msg.op.as_str() {
                        "hello" => {
                            let _ = tx.send(ReaderEvent::Hello);
                        }
                        "result" => {
                            let _ = tx.send(ReaderEvent::Result {
                                op_name: msg.op_name,
                                ok: msg.ok.unwrap_or(false),
                                error: msg.error,
                            });
                        }
                        "request" => {
                            if let Some(data) = msg.data {
                                handle_ui_request_async(data);
                            }
                        }
                        other => {
                            log::debug!("popup host: 未知子进程消息 op={other}");
                        }
                    },
                    Err(e) => {
                        log::warn!("popup host: 解析子进程行失败: {e}; line={trimmed}");
                    }
                }
            }
            Err(e) => {
                log::warn!("popup host: 读 IPC 失败: {e}");
                let _ = tx.send(ReaderEvent::Disconnected);
                break;
            }
        }
    }
}

fn handle_ui_request_async(envelope_json: String) {
    let _ = thread::Builder::new()
        .name("shizi-bridge-dispatch".into())
        .spawn(move || {
            let Some(app) = app_handle() else {
                log::warn!("popup host: 无 AppHandle，丢弃 bridge request");
                return;
            };
            let envelope: BridgeEnvelope = match serde_json::from_str(&envelope_json) {
                Ok(e) => e,
                Err(e) => {
                    log::warn!("popup host: BridgeEnvelope 解析失败: {e}");
                    return;
                }
            };
            let Some(state) = app.try_state::<AppState>() else {
                log::warn!("popup host: AppState 未就绪");
                return;
            };
            let response: BridgeResponse =
                dispatch::handle_bridge_request(app, state.inner(), &envelope);
            match serde_json::to_string(&response) {
                Ok(s) => {
                    if let Err(e) = push_json_fire_and_forget(&s) {
                        log::warn!("popup host: 回写 response 失败: {e}");
                    }
                }
                Err(e) => log::warn!("BridgeResponse 序列化失败: {e}"),
            }
        });
}

fn install_push_sink() {
    let sink: Arc<dyn Fn(String) + Send + Sync> = Arc::new(|json: String| {
        if let Err(e) = push_json_fire_and_forget(&json) {
            log::warn!("popup host push_json 失败: {e}");
        }
    });
    push::global().set_sink(Some(sink));
}

/// push 不等待 result，避免与 call_op 嵌套死锁；仍走 OP_GATE 串行写。
fn push_json_fire_and_forget(json: &str) -> Result<(), String> {
    let msg = IpcMessage {
        op: "push_json".into(),
        ok: None,
        error: None,
        op_name: None,
        x: None,
        y: None,
        mode: None,
        on: None,
        w: None,
        h: None,
        data: Some(json.to_string()),
    };
    let _gate = op_gate()
        .lock()
        .map_err(|_| "popup op gate 损坏".to_string())?;
    write_msg(&msg)?;
    // 排空对应 result（短等），失败不致命
    let _ = wait_result("push_json", Duration::from_secs(3));
    Ok(())
}

fn write_msg(msg: &IpcMessage) -> Result<(), String> {
    let mut guard = host_mutex()
        .lock()
        .map_err(|_| "popup host 锁损坏".to_string())?;
    let writer = guard
        .writer
        .as_mut()
        .ok_or_else(|| "popup host 未连接".to_string())?;
    let mut line = serde_json::to_string(msg).map_err(|e| e.to_string())?;
    line.push('\n');
    writer
        .write_all(line.as_bytes())
        .map_err(|e| format!("写 IPC 失败: {e}"))?;
    writer
        .flush()
        .map_err(|e| format!("flush IPC 失败: {e}"))?;
    Ok(())
}

fn wait_result(op: &str, timeout: Duration) -> Result<(), String> {
    let deadline = Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(format!("等待 op={op} 结果超时"));
        }

        let event = {
            let mut guard = host_mutex()
                .lock()
                .map_err(|_| "popup host 锁损坏".to_string())?;
            let rx = guard
                .result_rx
                .as_mut()
                .ok_or_else(|| "popup host 无 result 通道".to_string())?;
            // 短超时轮询，避免长时间持锁
            rx.recv_timeout(Duration::from_millis(50).min(remaining))
        };

        match event {
            Ok(ReaderEvent::Result {
                op_name,
                ok,
                error,
            }) => {
                if op_name
                    .as_deref()
                    .is_none_or(|n| n == op || n.is_empty())
                {
                    if ok {
                        return Ok(());
                    }
                    return Err(error.unwrap_or_else(|| format!("op={op} 失败")));
                }
            }
            Ok(ReaderEvent::Hello) => {}
            Ok(ReaderEvent::Disconnected) => {
                let mut guard = host_mutex()
                    .lock()
                    .map_err(|_| "popup host 锁损坏".to_string())?;
                teardown_locked(&mut guard);
                return Err("Shizi.Popup 已断开".into());
            }
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                let mut guard = host_mutex()
                    .lock()
                    .map_err(|_| "popup host 锁损坏".to_string())?;
                teardown_locked(&mut guard);
                return Err("IPC 读线程退出".into());
            }
        }
    }
}

fn call_op(msg: IpcMessage) -> Result<(), String> {
    let op = msg.op.clone();
    let _gate = op_gate()
        .lock()
        .map_err(|_| "popup op gate 损坏".to_string())?;
    write_msg(&msg)?;
    wait_result(&op, Duration::from_secs(10))
}

fn teardown_locked(guard: &mut HostInner) {
    push::global().set_sink(None);
    if let Some(mut child) = guard.child.take() {
        let _ = child.kill();
        let _ = child.wait();
    }
    guard.writer = None;
    guard.result_rx = None;
}

// —— 对外控制 API（对齐 C ABI 语义）——

pub fn ensure(app: &AppHandle) -> Result<(), String> {
    ensure_process(app)?;
    call_op(IpcMessage {
        op: "ensure".into(),
        ok: None,
        error: None,
        op_name: None,
        x: None,
        y: None,
        mode: None,
        on: None,
        w: None,
        h: None,
        data: None,
    })
}

pub fn show(x: f64, y: f64, mode: i32) -> Result<(), String> {
    call_op(IpcMessage {
        op: "show".into(),
        ok: None,
        error: None,
        op_name: None,
        x: Some(x),
        y: Some(y),
        mode: Some(mode),
        on: None,
        w: None,
        h: None,
        data: None,
    })
}

pub fn hide() -> Result<(), String> {
    if !is_running() {
        return Ok(());
    }
    call_op(IpcMessage {
        op: "hide".into(),
        ok: None,
        error: None,
        op_name: None,
        x: None,
        y: None,
        mode: None,
        on: None,
        w: None,
        h: None,
        data: None,
    })
}

pub fn set_always_on_top(on: bool) -> Result<(), String> {
    call_op(IpcMessage {
        op: "set_always_on_top".into(),
        ok: None,
        error: None,
        op_name: None,
        x: None,
        y: None,
        mode: None,
        on: Some(on),
        w: None,
        h: None,
        data: None,
    })
}

pub fn set_size(w: f64, h: f64) -> Result<(), String> {
    call_op(IpcMessage {
        op: "set_size".into(),
        ok: None,
        error: None,
        op_name: None,
        x: None,
        y: None,
        mode: None,
        on: None,
        w: Some(w),
        h: Some(h),
        data: None,
    })
}

pub fn shutdown() {
    let Ok(_gate) = op_gate().lock() else {
        return;
    };
    let mut guard = match host_mutex().lock() {
        Ok(g) => g,
        Err(_) => return,
    };
    if let Some(w) = guard.writer.as_mut() {
        let msg = IpcMessage {
            op: "shutdown".into(),
            ok: None,
            error: None,
            op_name: None,
            x: None,
            y: None,
            mode: None,
            on: None,
            w: None,
            h: None,
            data: None,
        };
        if let Ok(mut line) = serde_json::to_string(&msg) {
            line.push('\n');
            let _ = w.write_all(line.as_bytes());
            let _ = w.flush();
        }
        thread::sleep(Duration::from_millis(100));
    }
    teardown_locked(&mut guard);
}

pub fn is_available() -> bool {
    resolve_popup_exe().is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_popup_exe_candidates_non_empty() {
        let c = dev_exe_candidates();
        assert!(!c.is_empty());
        assert!(c
            .iter()
            .all(|p| p.to_string_lossy().contains("Shizi.Popup")));
    }

    #[test]
    fn ipc_message_roundtrip_show() {
        let msg = IpcMessage {
            op: "show".into(),
            ok: None,
            error: None,
            op_name: None,
            x: Some(10.0),
            y: Some(20.0),
            mode: Some(0),
            on: None,
            w: None,
            h: None,
            data: None,
        };
        let s = serde_json::to_string(&msg).unwrap();
        assert!(s.contains("\"op\":\"show\""));
        assert!(s.contains("\"mode\":0"));
        let back: IpcMessage = serde_json::from_str(&s).unwrap();
        assert_eq!(back.mode, Some(0));
    }

    #[test]
    fn is_available_true_when_release_built() {
        // 本任务门禁：若已 dotnet build Release，应能解析到 exe
        if dev_exe_candidates().iter().any(|p| p.is_file()) {
            assert!(is_available());
        }
    }
}
