//! M0：windows-reactor 专用 STA 宿主（S1 共存模型）。
//!
//! - 进程级 `bootstrap()` 一次
//! - 专用 STA 线程跑 `App` 消息循环
//! - **哨兵窗**（主 `App` 窗，立即 hide）防止 last-window-exit 杀进程
//! - 弹窗为 `ReactorWindow`；`Hide` 用 `ShowWindow(SW_HIDE)`，**不** `Close`
//! - `publish_label` / show / hide 经 `mpsc` 非阻塞投递，UI 线程 `DispatcherTimer` 泵送

#![cfg(all(windows, feature = "popup-winui"))]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use windows::core::PCWSTR;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, IsWindow, IsWindowVisible, ShowWindow, SW_HIDE, SW_SHOW,
};
// 注意：windows_reactor 将 `Result` 重导出为 `Result<T> = Result<T, Error>`，
// 本模块的 API 错误类型用 `String`，故显式使用 `std::result::Result`。
use windows_reactor::{
    App, AsyncSetState, Backdrop, DispatcherTimer, Element, ReactorWindow, RenderCx, button,
    text_block, vstack,
};

/// 哨兵窗标题（主 App 窗；始终存活，默认隐藏）。
pub const SENTINEL_TITLE: &str = "Shizi Reactor Sentinel";
/// Spike 弹窗标题（Inspect / FindWindow 用）。
pub const POPUP_TITLE: &str = "Shizi Reactor Spike";

/// 进程级 bootstrap 结果缓存（成功或失败都只尝试一次）。
static PROCESS_BOOTSTRAP: OnceLock<std::result::Result<(), String>> = OnceLock::new();

/// 进程内仅允许一个 STA host（Application::Start 全局一次）。
static HOST_STARTED: AtomicBool = AtomicBool::new(false);

/// UI 线程命令。
#[derive(Debug)]
pub enum HostCmd {
    Show,
    Hide,
    SetLabel(String),
    /// 仅 hide 弹窗并保留哨兵；**不会**退出进程。
    Shutdown,
}

/// 跨线程共享的 UI 槽位（label setter 由弹窗组件挂载）。
struct SharedUi {
    label_setter: Mutex<Option<AsyncSetState<String>>>,
}

/// 调用方持有的宿主句柄（可 Send；命令非阻塞 send）。
pub struct ReactorHostHandle {
    tx: Sender<HostCmd>,
}

impl ReactorHostHandle {
    /// 启动 STA 线程 + WinUI Runtime bootstrap + 哨兵/弹窗。
    ///
    /// 失败（Runtime 缺失、bootstrap 失败、启动超时等）返回 `Err`，
    /// 供上层 `create_host_with_winui_fallback` 降级 WebView。
    pub fn start() -> std::result::Result<Self, String> {
        if HOST_STARTED.swap(true, Ordering::SeqCst) {
            return Err("ReactorHost 已在本进程启动（S1：仅允许一个 STA Application）".into());
        }

        let (tx, rx) = mpsc::channel::<HostCmd>();
        let (ready_tx, ready_rx) = mpsc::channel::<std::result::Result<(), String>>();

        let shared = Arc::new(SharedUi {
            label_setter: Mutex::new(None),
        });
        let rx = Arc::new(Mutex::new(rx));

        let spawn_result = thread::Builder::new()
            .name("shizi-reactor-ui".into())
            .spawn({
                let shared = Arc::clone(&shared);
                let rx = Arc::clone(&rx);
                move || sta_thread_main(shared, rx, ready_tx)
            });

        if let Err(e) = spawn_result {
            HOST_STARTED.store(false, Ordering::SeqCst);
            return Err(format!("无法创建 reactor STA 线程: {e}"));
        }

        match ready_rx.recv_timeout(Duration::from_secs(30)) {
            Ok(Ok(())) => Ok(Self { tx }),
            Ok(Err(e)) => {
                HOST_STARTED.store(false, Ordering::SeqCst);
                Err(e)
            }
            Err(_) => {
                HOST_STARTED.store(false, Ordering::SeqCst);
                Err("reactor UI 线程启动超时（30s）".into())
            }
        }
    }

    /// 非阻塞更新弹窗标签文本。
    pub fn publish_label(&self, s: impl Into<String>) {
        let _ = self.tx.send(HostCmd::SetLabel(s.into()));
    }

    /// 非阻塞显示弹窗（幂等）。
    pub fn show(&self) {
        let _ = self.tx.send(HostCmd::Show);
    }

    /// 非阻塞隐藏弹窗（幂等；不销毁 Runtime / 不退出进程）。
    pub fn hide(&self) {
        let _ = self.tx.send(HostCmd::Hide);
    }

    /// 非阻塞：hide 弹窗，保留哨兵与 STA 循环。
    pub fn shutdown(&self) {
        let _ = self.tx.send(HostCmd::Shutdown);
    }
}

/// 进程级 WinAppSDK bootstrap（framework-dependent）。可从任意线程调用探测。
pub fn ensure_process_bootstrap() -> std::result::Result<(), String> {
    PROCESS_BOOTSTRAP
        .get_or_init(|| {
            windows_reactor::bootstrap()
                .map_err(|e| format!("windows_reactor::bootstrap 失败（需安装 Windows App Runtime）: {e}"))
        })
        .clone()
}

/// 是否已成功完成进程级 bootstrap（探测用；未调用过则为 false）。
pub fn is_process_bootstrapped() -> bool {
    matches!(PROCESS_BOOTSTRAP.get(), Some(Ok(())))
}

fn sta_thread_main(
    shared: Arc<SharedUi>,
    rx: Arc<Mutex<Receiver<HostCmd>>>,
    ready_tx: Sender<std::result::Result<(), String>>,
) {
    let ready = Arc::new(Mutex::new(Some(ready_tx)));

    if let Err(e) = ensure_process_bootstrap() {
        if let Some(tx) = ready.lock().ok().and_then(|mut g| g.take()) {
            let _ = tx.send(Err(e));
        }
        return;
    }

    let ready_for_app = Arc::clone(&ready);
    let shared_for_app = Arc::clone(&shared);
    let rx_for_app = Arc::clone(&rx);

    // App::render 阻塞于 UI 消息循环；最后一扇已注册窗关闭会 process::exit——
    // 因此必须保留哨兵，弹窗只 hide 不 close。
    let app_result = App::new()
        .title(SENTINEL_TITLE)
        .inner_size(1.0, 1.0)
        .render(move |cx| {
            sentinel_root(
                cx,
                Arc::clone(&shared_for_app),
                Arc::clone(&rx_for_app),
                Arc::clone(&ready_for_app),
            )
        });

    // 仅在 Application::Start 启动失败时走到这里（正常托盘场景永不返回）。
    if let Err(e) = app_result {
        if let Some(tx) = ready.lock().ok().and_then(|mut g| g.take()) {
            let _ = tx.send(Err(format!("App::render / Application::Start 失败: {e}")));
        }
    }
}

fn sentinel_root(
    cx: &mut RenderCx,
    shared: Arc<SharedUi>,
    rx: Arc<Mutex<Receiver<HostCmd>>>,
    ready: Arc<Mutex<Option<Sender<std::result::Result<(), String>>>>>,
) -> Element {
    cx.use_effect((), {
        let shared = Arc::clone(&shared);
        let rx = Arc::clone(&rx);
        let ready = Arc::clone(&ready);
        move || {
            // 立刻隐藏哨兵，避免 1×1 闪窗（仍占用 registry，防止 last-window-exit）。
            hide_window_by_title(SENTINEL_TITLE);

            if let Err(e) = open_popup(Arc::clone(&shared)) {
                signal_ready(&ready, Err(e));
                return;
            }

            // 命令泵：UI 线程定时 try_recv，保证 WinUI 调用始终在 STA。
            let shared_pump = Arc::clone(&shared);
            let rx_pump = Arc::clone(&rx);
            match DispatcherTimer::new(Duration::from_millis(33), move || {
                pump_commands(&rx_pump, &shared_pump);
            }) {
                Ok(timer) => {
                    // 进程级 STA 存活期间一直泵送。
                    std::mem::forget(timer);
                }
                Err(e) => {
                    signal_ready(&ready, Err(format!("DispatcherTimer 创建失败: {e}")));
                    return;
                }
            }

            // 弹窗创建后略延迟再 hide（Activate 经 Dispatcher 异步），避免 FindWindow 竞态。
            // 之后由 HostCmd::Show 唤起。
            match DispatcherTimer::new_one_shot(Duration::from_millis(80), || {
                hide_window_by_title(POPUP_TITLE);
            }) {
                Ok(t) => std::mem::forget(t),
                Err(e) => {
                    // 定时器失败则尽力同步 hide，不阻断 ready
                    log::warn!("initial hide timer 失败，回退同步 hide: {e}");
                    hide_window_by_title(POPUP_TITLE);
                }
            }

            signal_ready(&ready, Ok(()));
        }
    });

    // 哨兵几乎不可见内容
    text_block("").into()
}

fn popup_root(cx: &mut RenderCx, shared: Arc<SharedUi>) -> Element {
    let (label, set_label) = cx.use_async_state(String::from("spike"));

    // 挂载跨线程 setter（供 HostCmd::SetLabel）
    cx.use_effect((), {
        let set_label = set_label.clone();
        let shared = Arc::clone(&shared);
        move || {
            if let Ok(mut slot) = shared.label_setter.lock() {
                *slot = Some(set_label);
            }
        }
    });

    vstack((
        text_block("M0 windows-reactor spike")
            .font_size(14.0)
            .semibold(),
        text_block(label.clone()).font_size(22.0).bold(),
        button("Close").on_click(|| {
            // hide，不 Close → 不触发 last-window-exit
            hide_window_by_title(POPUP_TITLE);
        }),
    ))
    .spacing(12.0)
    .into()
}

fn open_popup(shared: Arc<SharedUi>) -> std::result::Result<(), String> {
    ReactorWindow::new()
        .title(POPUP_TITLE)
        .inner_size(468.0, 320.0)
        .backdrop(Backdrop::Mica)
        .render(move |cx| popup_root(cx, Arc::clone(&shared)))
        .map(|_| ())
        .map_err(|e| format!("ReactorWindow 打开失败: {e}"))
}

fn pump_commands(rx: &Arc<Mutex<Receiver<HostCmd>>>, shared: &Arc<SharedUi>) {
    let Ok(guard) = rx.lock() else {
        return;
    };
    while let Ok(cmd) = guard.try_recv() {
        match cmd {
            HostCmd::Show => {
                show_or_reopen_popup(shared);
            }
            HostCmd::Hide | HostCmd::Shutdown => {
                hide_window_by_title(POPUP_TITLE);
            }
            HostCmd::SetLabel(s) => {
                if let Ok(slot) = shared.label_setter.lock() {
                    if let Some(setter) = slot.as_ref() {
                        setter.call(s);
                    }
                }
            }
        }
    }
}

fn show_or_reopen_popup(shared: &Arc<SharedUi>) {
    if let Some(hwnd) = find_hwnd(POPUP_TITLE) {
        show_hwnd(hwnd);
        return;
    }
    // 标题栏 X 关闭后 HWND 消失：在 UI 线程重建弹窗（不重建 Runtime / 不重跑 bootstrap）
    if let Ok(mut slot) = shared.label_setter.lock() {
        *slot = None;
    }
    if let Err(e) = open_popup(Arc::clone(shared)) {
        log::warn!("reactor popup 重建失败: {e}");
        return;
    }
    // open 后默认可见；与首次 start 时「先 hide 等 Show」不同，此处本就是 Show 路径
}

fn signal_ready(
    ready: &Mutex<Option<Sender<std::result::Result<(), String>>>>,
    result: std::result::Result<(), String>,
) {
    if let Ok(mut g) = ready.lock() {
        if let Some(tx) = g.take() {
            let _ = tx.send(result);
        }
    }
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn find_hwnd(title: &str) -> Option<HWND> {
    let wide = to_wide(title);
    // windows 0.58：FindWindowW 返回 Result<HWND, Error>
    let hwnd = unsafe { FindWindowW(PCWSTR::null(), PCWSTR(wide.as_ptr())) }.ok()?;
    if hwnd.is_invalid() {
        return None;
    }
    let alive = unsafe { IsWindow(hwnd).as_bool() };
    if alive {
        Some(hwnd)
    } else {
        None
    }
}

fn hide_window_by_title(title: &str) {
    if let Some(hwnd) = find_hwnd(title) {
        unsafe {
            let _ = ShowWindow(hwnd, SW_HIDE);
        }
    }
}

fn show_hwnd(hwnd: HWND) {
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOW);
    }
}

/// 测试/诊断：弹窗 HWND 当前是否可见。
pub fn is_popup_window_visible() -> bool {
    find_hwnd(POPUP_TITLE)
        .map(|hwnd| unsafe { IsWindowVisible(hwnd).as_bool() })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::TrySendError;

    #[test]
    fn ensure_process_bootstrap_is_idempotent() {
        // 本机有 Runtime 时期望 Ok；无 Runtime 时两次都是同一 Err。
        let a = ensure_process_bootstrap();
        let b = ensure_process_bootstrap();
        assert_eq!(a.is_ok(), b.is_ok(), "bootstrap 结果应缓存: {a:?} vs {b:?}");
        if let (Err(e1), Err(e2)) = (&a, &b) {
            assert_eq!(e1, e2);
        }
    }

    #[test]
    fn host_cmd_channel_send_is_nonblocking() {
        let (tx, rx) = mpsc::channel::<HostCmd>();
        // 无接收方阻塞时 send 仍立即返回（有界队列才会在满时失败；mpsc 无界）
        tx.send(HostCmd::SetLabel("a".into())).unwrap();
        tx.send(HostCmd::Show).unwrap();
        tx.send(HostCmd::Hide).unwrap();
        assert!(matches!(rx.try_recv(), Ok(HostCmd::SetLabel(s)) if s == "a"));
        assert!(matches!(rx.try_recv(), Ok(HostCmd::Show)));
        assert!(matches!(rx.try_recv(), Ok(HostCmd::Hide)));
    }

    #[test]
    fn publish_label_api_does_not_block_on_full_queue_semantics() {
        // 文档约束：publish 使用非阻塞 mpsc send（无界 channel 下 send 即非阻塞）。
        let (tx, _rx) = mpsc::sync_channel::<HostCmd>(1);
        tx.try_send(HostCmd::SetLabel("x".into())).unwrap();
        // 队列满时 try_send 立即返回，不阻塞调用线程
        let err = tx.try_send(HostCmd::SetLabel("y".into())).unwrap_err();
        assert!(matches!(
            err,
            TrySendError::Full(HostCmd::SetLabel(_))
        ));
    }

    /// 完整 GUI 冒烟：需交互式会话 + Windows App Runtime。
    ///
    /// ```text
    /// set SHIZI_M0_SPIKE=1
    /// cargo test -p shizi --lib m0_reactor_host_smoke -- --nocapture --test-threads=1
    /// ```
    ///
    /// 说明：`cargo test` 可执行文件在 `target/debug/deps/`，而
    /// `windows_reactor_setup` 把 Bootstrap DLL / `resources.pri` 放到 `target/debug/`。
    /// 本测试会尽力把这两项复制到 exe 旁。
    #[test]
    fn m0_reactor_host_smoke() {
        if std::env::var("SHIZI_M0_SPIKE").is_err() {
            eprintln!("skip m0_reactor_host_smoke（设置 SHIZI_M0_SPIKE=1 启用）");
            return;
        }

        ensure_reactor_runtime_assets_for_test();

        let host = match ReactorHostHandle::start() {
            Ok(h) => h,
            Err(e) => panic!("start 失败（应可降级 WebView）: {e}"),
        };

        // 等初始 hide one-shot 完成
        thread::sleep(Duration::from_millis(200));

        host.publish_label("m0-label-1");
        host.show();
        thread::sleep(Duration::from_millis(800));
        assert!(
            find_hwnd(POPUP_TITLE).is_some(),
            "Show 后应能找到弹窗 HWND（title={POPUP_TITLE})"
        );
        assert!(
            is_popup_window_visible(),
            "Show 后弹窗应可见"
        );

        host.publish_label("m0-label-2");
        thread::sleep(Duration::from_millis(400));

        host.hide();
        thread::sleep(Duration::from_millis(400));
        // hide 后进程必须仍在（本测试能继续即证明未 process::exit）
        assert!(
            !is_popup_window_visible(),
            "Hide 后弹窗应不可见"
        );

        host.show();
        thread::sleep(Duration::from_millis(400));
        assert!(is_popup_window_visible(), "再次 Show 应可见（不重建 Runtime）");
        host.hide();
        // 哨兵仍在；不调用任何会导致 last-window-exit 的 close
        eprintln!("m0_reactor_host_smoke: hide/show 完成，进程仍存活");
    }

    /// 将 framework-dependent 资产复制到当前测试 exe 目录（若缺失）。
    fn ensure_reactor_runtime_assets_for_test() {
        let exe = match std::env::current_exe() {
            Ok(p) => p,
            Err(_) => return,
        };
        let Some(exe_dir) = exe.parent() else {
            return;
        };
        // target/debug/deps -> target/debug
        let candidates = [
            exe_dir.to_path_buf(),
            exe_dir.join(".."),
            exe_dir.join("../.."),
        ];
        let names = [
            "microsoft.windowsappruntime.bootstrap.dll",
            "Microsoft.WindowsAppRuntime.Bootstrap.dll",
            "resources.pri",
        ];
        for name in names {
            let dest = exe_dir.join(name);
            if dest.is_file() {
                continue;
            }
            for base in &candidates {
                let src = base.join(name);
                if src.is_file() {
                    let _ = std::fs::copy(&src, &dest);
                    break;
                }
            }
        }
    }
}
