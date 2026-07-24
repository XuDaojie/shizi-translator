//! 路径 R：windows-reactor 专用 STA 宿主（生产化 ensure/show/hide/publish）。
//!
//! - 进程级 `bootstrap()` 一次
//! - 专用 STA 线程跑 `App` 消息循环
//! - **哨兵窗**（主 `App` 窗，立即 hide）防止 last-window-exit 杀进程
//! - 弹窗为 `ReactorWindow`；`Hide` / `Destroy` 用 `ShowWindow(SW_HIDE)`，**不** `Close`
//! - `publish` / show / hide 经 `mpsc` 非阻塞投递，UI 线程 `DispatcherTimer` 泵送
//! - 弹窗 HWND 缓存在 `SharedUi`（优先于 FindWindow 标题查找）
//! - `HOST_STARTED`：线程 spawn 成功后**永不复位**，避免超时后双 STA

#![cfg(all(windows, feature = "popup-winui"))]

use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use windows::core::PCWSTR;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::HiDpi::{GetDpiForSystem, GetDpiForWindow};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, IsWindow, IsWindowVisible, SetWindowPos, ShowWindow, HWND_NOTOPMOST, HWND_TOP,
    HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SW_HIDE, SW_SHOW,
    USER_DEFAULT_SCREEN_DPI,
};
// 注意：windows_reactor 将 `Result` 重导出为 `Result<T> = Result<T, Error>`，
// 本模块的 API 错误类型用 `String`，故显式使用 `std::result::Result`。
use windows_reactor::{
    App, AsyncSetState, Backdrop, DispatcherTimer, Element, ReactorWindow, RenderCx, text_block,
};

use crate::app::popup_backend::types::{PopupPositionMode, PopupViewModel};
use crate::app::popup_window::{compute_popup_position, LogicalPos, LogicalRect, LogicalSize};
use crate::platform::cursor_logical_context;

use super::state::{self, store_global};
use super::view;

/// 哨兵窗标题（主 App 窗；始终存活，默认隐藏）。
pub const SENTINEL_TITLE: &str = "Shizi Reactor Sentinel";
/// 弹窗标题（Inspect / FindWindow 回退用；产品名）。
pub const POPUP_TITLE: &str = "柿子翻译";

/// 与 GDI / Open Design 原型对齐的逻辑尺寸（定位与 `inner_size`）。
const POPUP_LOGICAL_WIDTH: f64 = 468.0;
const POPUP_LOGICAL_HEIGHT: f64 = 520.0;

/// 进程级 bootstrap 结果缓存（成功或失败都只尝试一次）。
static PROCESS_BOOTSTRAP: OnceLock<std::result::Result<(), String>> = OnceLock::new();

/// 进程内仅允许一个 STA host（Application::Start 全局一次）。
///
/// **spawn 成功后永不复位**：即使 ready 超时 / bootstrap 失败，也禁止再次 `start()`，
/// 避免「超时清标志 → 二次 start → 双 STA」。
static HOST_STARTED: AtomicBool = AtomicBool::new(false);

/// UI 线程命令。
#[derive(Debug)]
pub enum HostCmd {
    /// 若弹窗 HWND 无效则重建（不重建 Runtime / 不重跑 bootstrap）。
    Ensure,
    /// 显示弹窗；`NearCursor` 时按光标工作区定位（与 GDI 同输入）。
    Show(PopupPositionMode),
    /// 隐藏弹窗（幂等；不销毁 Runtime）。
    Hide,
    /// 产品语义：hide 并清除 HWND 缓存，**保留哨兵与 STA**（不 `Close`，避免 last-window-exit）。
    Destroy,
    /// 投递 ViewModel（UI 线程 apply；调用方应先 `store` 以保证无窗时快照可读）。
    Publish(PopupViewModel),
    /// 兼容 M0：更新 `source_text` 并 re-render。
    SetLabel(String),
    /// 设置弹窗 HWND 置顶（UI 线程执行）。
    SetTopmost(bool),
    /// 仅 hide 弹窗并保留哨兵；**不会**退出进程。
    Shutdown,
}

/// 跨线程共享的 UI 槽位（HWND + VM setter 由弹窗组件挂载）。
struct SharedUi {
    /// 弹窗 HWND 缓存（`HWND.0 as isize`；0 = 无）。
    popup_hwnd: AtomicIsize,
    /// UI 挂载后的 VM setter；未挂载时 publish 仅写 `SharedPopupState`（pending）。
    vm_setter: Mutex<Option<AsyncSetState<PopupViewModel>>>,
}

impl SharedUi {
    fn new() -> Self {
        Self {
            popup_hwnd: AtomicIsize::new(0),
            vm_setter: Mutex::new(None),
        }
    }

    fn set_popup_hwnd(&self, hwnd: HWND) {
        let raw = hwnd.0 as isize;
        if raw != 0 {
            self.popup_hwnd.store(raw, Ordering::SeqCst);
        }
    }

    fn clear_popup_hwnd(&self) {
        self.popup_hwnd.store(0, Ordering::SeqCst);
    }

    fn popup_hwnd(&self) -> Option<HWND> {
        let raw = self.popup_hwnd.load(Ordering::SeqCst);
        if raw == 0 {
            return None;
        }
        let hwnd = HWND(raw as *mut core::ffi::c_void);
        let alive = unsafe { IsWindow(hwnd).as_bool() };
        if alive {
            Some(hwnd)
        } else {
            self.clear_popup_hwnd();
            None
        }
    }

    fn is_popup_visible(&self) -> bool {
        self.popup_hwnd()
            .map(|hwnd| unsafe { IsWindowVisible(hwnd).as_bool() })
            .unwrap_or(false)
    }
}

/// 调用方持有的宿主句柄（可 Send；命令非阻塞 send）。
pub struct ReactorHostHandle {
    tx: Sender<HostCmd>,
    shared: Arc<SharedUi>,
    /// `start` 成功 ready 后为 true；STA 存活期间保持 true。
    alive: Arc<AtomicBool>,
}

impl ReactorHostHandle {
    /// 启动 STA 线程 + WinUI Runtime bootstrap + 哨兵/弹窗。
    ///
    /// 失败（Runtime 缺失、bootstrap 失败、启动超时等）返回 `Err`，
    /// 供上层 `create_host_with_winui_fallback` 降级 WebView。
    ///
    /// **注意**：线程 `spawn` 成功后即使失败/超时，`HOST_STARTED` 仍保持 true，
    /// 本进程内不可再次 `start()`（防双 STA）。
    pub fn start() -> std::result::Result<Self, String> {
        if HOST_STARTED.swap(true, Ordering::SeqCst) {
            return Err("ReactorHost 已在本进程启动（S1：仅允许一个 STA Application）".into());
        }

        let (tx, rx) = mpsc::channel::<HostCmd>();
        let (ready_tx, ready_rx) = mpsc::channel::<std::result::Result<(), String>>();

        let shared = Arc::new(SharedUi::new());
        let alive = Arc::new(AtomicBool::new(false));
        let rx = Arc::new(Mutex::new(rx));

        let spawn_result = thread::Builder::new()
            .name("shizi-reactor-ui".into())
            .spawn({
                let shared = Arc::clone(&shared);
                let rx = Arc::clone(&rx);
                move || sta_thread_main(shared, rx, ready_tx)
            });

        if let Err(e) = spawn_result {
            // 仅 spawn 失败可复位：线程根本未起，无双 STA 风险。
            HOST_STARTED.store(false, Ordering::SeqCst);
            return Err(format!("无法创建 reactor STA 线程: {e}"));
        }

        // spawn 已成功：此后 HOST_STARTED 永不复位（含超时 / ready Err）。
        match ready_rx.recv_timeout(Duration::from_secs(30)) {
            Ok(Ok(())) => {
                alive.store(true, Ordering::SeqCst);
                Ok(Self { tx, shared, alive })
            }
            Ok(Err(e)) => Err(format!(
                "{e}（HOST 已标记污染，本进程禁止再次 start）"
            )),
            Err(_) => Err(
                "reactor UI 线程启动超时（30s；HOST 已标记污染，禁止再 start 以防双 STA）"
                    .into(),
            ),
        }
    }

    /// STA 是否已成功 ready（handle 级）。
    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }

    /// 弹窗当前是否可见（读缓存 HWND / IsWindowVisible）。
    pub fn is_visible(&self) -> bool {
        self.shared.is_popup_visible()
    }

    /// 非阻塞：确保弹窗 HWND 存在（失效则重建）。
    pub fn ensure(&self) {
        let _ = self.tx.send(HostCmd::Ensure);
    }

    /// 非阻塞显示弹窗。`NearCursor` 用 `compute_popup_position` + 平台 cursor。
    pub fn show(&self, mode: PopupPositionMode) -> std::result::Result<(), String> {
        self.tx
            .send(HostCmd::Show(mode))
            .map_err(|_| "reactor host 通道已关闭".into())
    }

    /// 非阻塞隐藏弹窗（幂等；不销毁 Runtime / 不退出进程）。
    pub fn hide(&self) {
        let _ = self.tx.send(HostCmd::Hide);
    }

    /// 非阻塞：产品 destroy = hide + 清 HWND 缓存，保留哨兵。
    pub fn destroy(&self) {
        let _ = self.tx.send(HostCmd::Destroy);
    }

    /// 非阻塞 publish：先写全局快照，再 post UI 线程 apply。
    ///
    /// 调用线程**不**等待 UI；窗未创建 / setter 未挂载时仅 pending 在
    /// [`SharedPopupState`]（last-write-wins），挂载后 flush。
    pub fn publish(&self, vm: &PopupViewModel) {
        store_global(vm);
        let _ = self.tx.send(HostCmd::Publish(vm.clone()));
    }

    /// 兼容 M0：映射为更新 `source_text` 的 publish。
    pub fn publish_label(&self, s: impl Into<String>) {
        let mut vm = state::global_snapshot();
        vm.source_text = s.into();
        self.publish(&vm);
    }

    /// 非阻塞：hide 弹窗，保留哨兵与 STA 循环。
    pub fn shutdown(&self) {
        let _ = self.tx.send(HostCmd::Shutdown);
    }

    /// 非阻塞：在 UI 线程设置弹窗置顶。
    pub fn set_topmost(&self, topmost: bool) {
        let _ = self.tx.send(HostCmd::SetTopmost(topmost));
    }
}

/// 任意线程：按标题查找弹窗 HWND 并设置置顶（best-effort；UI 线程路径优先用 [`ReactorHostHandle::set_topmost`]）。
pub fn apply_popup_topmost(topmost: bool) {
    let Some(hwnd) = find_hwnd(POPUP_TITLE) else {
        log::debug!("apply_popup_topmost：未找到弹窗 HWND");
        return;
    };
    set_hwnd_topmost(hwnd, topmost);
}

fn set_hwnd_topmost(hwnd: HWND, topmost: bool) {
    let insert = if topmost { HWND_TOPMOST } else { HWND_NOTOPMOST };
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            insert,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }
}

/// 进程级 WinAppSDK bootstrap（framework-dependent）。可从任意线程调用探测。
pub fn ensure_process_bootstrap() -> std::result::Result<(), String> {
    PROCESS_BOOTSTRAP
        .get_or_init(|| {
            windows_reactor::bootstrap().map_err(|e| {
                format!("windows_reactor::bootstrap 失败（需安装 Windows App Runtime）: {e}")
            })
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

            // 弹窗创建后略延迟再 hide（Activate 经 Dispatcher 异步），并缓存 HWND。
            // 之后由 HostCmd::Show 唤起。
            let shared_hide = Arc::clone(&shared);
            match DispatcherTimer::new_one_shot(Duration::from_millis(80), move || {
                capture_popup_hwnd(&shared_hide);
                hide_popup_hwnd(&shared_hide);
            }) {
                Ok(t) => std::mem::forget(t),
                Err(e) => {
                    log::warn!("initial hide timer 失败，回退同步 hide: {e}");
                    capture_popup_hwnd(&shared);
                    hide_popup_hwnd(&shared);
                }
            }

            signal_ready(&ready, Ok(()));
        }
    });

    // 哨兵几乎不可见内容
    text_block("").into()
}

fn popup_root(cx: &mut RenderCx, shared: Arc<SharedUi>) -> Element {
    // 初始值取全局 pending 快照（publish 可能早于窗挂载）。
    let initial = state::global_snapshot();
    let (vm, set_vm) = cx.use_async_state(initial);

    // 挂载 setter + flush pending + 缓存 HWND
    cx.use_effect((), {
        let set_vm = set_vm.clone();
        let shared = Arc::clone(&shared);
        move || {
            capture_popup_hwnd(&shared);
            if let Ok(mut slot) = shared.vm_setter.lock() {
                *slot = Some(set_vm.clone());
            }
            // last-write-wins pending → flush 到 UI
            let pending = state::global_snapshot();
            set_vm.call(pending);
        }
    });

    // 最小 UI 统一走 view，避免 host 内嵌第二套控件树。
    view::render_popup(&vm)
}

fn open_popup(shared: Arc<SharedUi>) -> std::result::Result<(), String> {
    // 重建前清旧 setter，避免指向已销毁组件
    if let Ok(mut slot) = shared.vm_setter.lock() {
        *slot = None;
    }
    ReactorWindow::new()
        .title(POPUP_TITLE)
        .inner_size(POPUP_LOGICAL_WIDTH, POPUP_LOGICAL_HEIGHT)
        .backdrop(Backdrop::Mica)
        .render({
            let shared = Arc::clone(&shared);
            move |cx| popup_root(cx, Arc::clone(&shared))
        })
        .map_err(|e| format!("ReactorWindow 打开失败: {e}"))?;
    capture_popup_hwnd(&shared);
    Ok(())
}

fn pump_commands(rx: &Arc<Mutex<Receiver<HostCmd>>>, shared: &Arc<SharedUi>) {
    let Ok(guard) = rx.lock() else {
        return;
    };
    while let Ok(cmd) = guard.try_recv() {
        match cmd {
            HostCmd::Ensure => {
                ensure_popup(shared);
            }
            HostCmd::Show(mode) => {
                show_popup_with_mode(shared, mode);
            }
            HostCmd::Hide | HostCmd::Shutdown => {
                hide_popup_hwnd(shared);
            }
            HostCmd::Destroy => {
                // 产品语义：hide 保留哨兵；不 Close。
                hide_popup_hwnd(shared);
                shared.clear_popup_hwnd();
                if let Ok(mut slot) = shared.vm_setter.lock() {
                    *slot = None;
                }
            }
            HostCmd::Publish(vm) => {
                apply_publish(shared, &vm);
            }
            HostCmd::SetLabel(s) => {
                let mut vm = state::global_snapshot();
                vm.source_text = s;
                store_global(&vm);
                apply_publish(shared, &vm);
            }
            HostCmd::SetTopmost(topmost) => {
                if let Some(hwnd) = shared
                    .popup_hwnd()
                    .or_else(|| find_hwnd(POPUP_TITLE).inspect(|h| shared.set_popup_hwnd(*h)))
                {
                    set_hwnd_topmost(hwnd, topmost);
                } else {
                    log::debug!("SetTopmost：无弹窗 HWND");
                }
            }
        }
    }
}

fn apply_publish(shared: &SharedUi, vm: &PopupViewModel) {
    store_global(vm);
    if let Ok(slot) = shared.vm_setter.lock() {
        if let Some(setter) = slot.as_ref() {
            setter.call(vm.clone());
        }
        // setter 未挂载：pending 已在 SharedPopupState，挂载 use_effect 会 flush
    }
}

fn ensure_popup(shared: &Arc<SharedUi>) {
    if shared.popup_hwnd().is_some() {
        return;
    }
    // 标题栏 X 关闭后 HWND 消失：在 UI 线程重建弹窗
    if let Err(e) = open_popup(Arc::clone(shared)) {
        log::warn!("reactor popup ensure/重建失败: {e}");
    }
}

fn show_popup_with_mode(shared: &Arc<SharedUi>, mode: PopupPositionMode) {
    ensure_popup(shared);
    let Some(hwnd) = shared
        .popup_hwnd()
        .or_else(|| find_hwnd(POPUP_TITLE).inspect(|h| shared.set_popup_hwnd(*h)))
    else {
        log::warn!("reactor popup show：无可用 HWND");
        return;
    };

    match mode {
        PopupPositionMode::NearCursor => {
            let scale = window_scale(hwnd);
            if let Some((cx, cy, wx, wy, ww, wh)) = cursor_logical_context(scale) {
                let pos = compute_popup_position(
                    LogicalPos { x: cx, y: cy },
                    LogicalSize {
                        width: POPUP_LOGICAL_WIDTH,
                        height: POPUP_LOGICAL_HEIGHT,
                    },
                    LogicalRect {
                        x: wx,
                        y: wy,
                        width: ww,
                        height: wh,
                    },
                );
                let x = logical_to_physical(pos.x, scale);
                let y = logical_to_physical(pos.y, scale);
                let w = logical_to_physical(POPUP_LOGICAL_WIDTH, scale);
                let h = logical_to_physical(POPUP_LOGICAL_HEIGHT, scale);
                unsafe {
                    let _ = SetWindowPos(
                        hwnd,
                        HWND_TOP,
                        x,
                        y,
                        w,
                        h,
                        SWP_SHOWWINDOW | SWP_NOACTIVATE,
                    );
                }
            } else {
                show_hwnd(hwnd);
            }
        }
        PopupPositionMode::Restore => {
            show_hwnd(hwnd);
        }
    }
}

fn capture_popup_hwnd(shared: &SharedUi) {
    if let Some(hwnd) = find_hwnd(POPUP_TITLE) {
        shared.set_popup_hwnd(hwnd);
    }
}

fn hide_popup_hwnd(shared: &SharedUi) {
    if let Some(hwnd) = shared.popup_hwnd() {
        unsafe {
            let _ = ShowWindow(hwnd, SW_HIDE);
        }
        return;
    }
    hide_window_by_title(POPUP_TITLE);
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

fn logical_to_physical(logical: f64, scale: f64) -> i32 {
    (logical * scale).round() as i32
}

fn window_scale(hwnd: HWND) -> f64 {
    let dpi = unsafe {
        let d = GetDpiForWindow(hwnd);
        if d == 0 {
            GetDpiForSystem().max(USER_DEFAULT_SCREEN_DPI)
        } else {
            d
        }
    };
    dpi as f64 / USER_DEFAULT_SCREEN_DPI as f64
}

/// 测试/诊断：弹窗 HWND 当前是否可见。
pub fn is_popup_window_visible() -> bool {
    find_hwnd(POPUP_TITLE)
        .map(|hwnd| unsafe { IsWindowVisible(hwnd).as_bool() })
        .unwrap_or(false)
}

/// 本进程是否已尝试启动过 Reactor host（含污染态）。
pub fn is_host_started() -> bool {
    HOST_STARTED.load(Ordering::SeqCst)
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::state::SharedPopupState;
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
    fn popup_window_metrics_and_title() {
        assert_eq!(POPUP_TITLE, "柿子翻译");
        assert!((POPUP_LOGICAL_WIDTH - 468.0).abs() < f64::EPSILON);
        assert!((POPUP_LOGICAL_HEIGHT - 520.0).abs() < f64::EPSILON);
    }

    #[test]
    fn host_cmd_channel_send_is_nonblocking() {
        let (tx, rx) = mpsc::channel::<HostCmd>();
        // 无接收方阻塞时 send 仍立即返回（有界队列才会在满时失败；mpsc 无界）
        let vm = PopupViewModel {
            source_text: "a".into(),
            ..Default::default()
        };
        tx.send(HostCmd::Publish(vm)).unwrap();
        tx.send(HostCmd::Show(PopupPositionMode::NearCursor))
            .unwrap();
        tx.send(HostCmd::Hide).unwrap();
        tx.send(HostCmd::Ensure).unwrap();
        tx.send(HostCmd::Destroy).unwrap();
        assert!(matches!(rx.try_recv(), Ok(HostCmd::Publish(v)) if v.source_text == "a"));
        assert!(matches!(
            rx.try_recv(),
            Ok(HostCmd::Show(PopupPositionMode::NearCursor))
        ));
        assert!(matches!(rx.try_recv(), Ok(HostCmd::Hide)));
        assert!(matches!(rx.try_recv(), Ok(HostCmd::Ensure)));
        assert!(matches!(rx.try_recv(), Ok(HostCmd::Destroy)));
    }

    #[test]
    fn publish_label_api_does_not_block_on_full_queue_semantics() {
        // 文档约束：publish 使用非阻塞 mpsc send（无界 channel 下 send 即非阻塞）。
        let (tx, _rx) = mpsc::sync_channel::<HostCmd>(1);
        let vm = PopupViewModel {
            source_text: "x".into(),
            ..Default::default()
        };
        tx.try_send(HostCmd::Publish(vm)).unwrap();
        // 队列满时 try_send 立即返回，不阻塞调用线程
        let err = tx
            .try_send(HostCmd::Publish(PopupViewModel::default()))
            .unwrap_err();
        assert!(matches!(err, TrySendError::Full(HostCmd::Publish(_))));
    }

    #[test]
    fn publish_does_not_require_window() {
        // host 未 start 时 store 全局快照仍成功（与现 backend publish 窗未创建分支一致）
        let st = SharedPopupState::default();
        let vm = PopupViewModel {
            source_text: "hi".into(),
            ..Default::default()
        };
        st.store(&vm);
        assert_eq!(st.load().source_text, "hi");
    }

    #[test]
    fn shared_ui_hwnd_cache_roundtrip() {
        let ui = SharedUi::new();
        assert!(ui.popup_hwnd().is_none());
        // 无效 HWND 不应被缓存为「存活」
        ui.set_popup_hwnd(HWND(0 as *mut _));
        // 0 被 set 忽略
        assert!(ui.popup_hwnd().is_none());
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
        assert!(host.is_alive());

        // 等初始 hide one-shot 完成
        thread::sleep(Duration::from_millis(200));

        host.publish_label("m0-label-1");
        host.show(PopupPositionMode::Restore).expect("show");
        thread::sleep(Duration::from_millis(800));
        assert!(
            find_hwnd(POPUP_TITLE).is_some(),
            "Show 后应能找到弹窗 HWND（title={POPUP_TITLE})"
        );
        assert!(is_popup_window_visible() || host.is_visible(), "Show 后弹窗应可见");

        host.publish_label("m0-label-2");
        thread::sleep(Duration::from_millis(400));

        host.hide();
        thread::sleep(Duration::from_millis(400));
        // hide 后进程必须仍在（本测试能继续即证明未 process::exit）
        assert!(
            !is_popup_window_visible() && !host.is_visible(),
            "Hide 后弹窗应不可见"
        );

        host.show(PopupPositionMode::NearCursor).expect("show near");
        thread::sleep(Duration::from_millis(400));
        assert!(
            is_popup_window_visible() || host.is_visible(),
            "再次 Show 应可见（不重建 Runtime）"
        );
        host.hide();
        // 哨兵仍在；不调用任何会导致 last-window-exit 的 close
        eprintln!("m0_reactor_host_smoke: hide/show 完成，进程仍存活");
    }

    /// 回归：模拟 Tauri/tao 已设置进程 DPI 后再 start（历史上 0x80070005 硬失败）。
    ///
    /// ```text
    /// set SHIZI_M0_SPIKE=1
    /// cargo test -p shizi --lib m0_reactor_host_smoke_after_dpi_preset -- --nocapture --test-threads=1
    /// ```
    ///
    /// **勿**与 `m0_reactor_host_smoke` 同进程连跑（`HOST_STARTED` 进程级仅一次）。
    #[test]
    fn m0_reactor_host_smoke_after_dpi_preset() {
        if std::env::var("SHIZI_M0_SPIKE").is_err() {
            eprintln!("skip m0_reactor_host_smoke_after_dpi_preset（设置 SHIZI_M0_SPIKE=1 启用）");
            return;
        }
        if is_host_started() {
            eprintln!(
                "skip m0_reactor_host_smoke_after_dpi_preset（HOST 已启动；请单独跑本测试）"
            );
            return;
        }

        ensure_reactor_runtime_assets_for_test();

        // 与 tao 相同：进程级 PerMonitorV2（-4）。第二次 SetProcessDpi 会 ACCESS_DENIED。
        // SAFETY: user32 FFI；进程级 DPI 设置。
        let dpi_set = unsafe {
            windows::Win32::UI::HiDpi::SetProcessDpiAwarenessContext(
                windows::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
            )
        };
        // 已设置时可能失败；首次应成功。无论结果，只要进程已有 DPI 上下文即可。
        let _ = dpi_set;

        let host = match ReactorHostHandle::start() {
            Ok(h) => h,
            Err(e) => panic!(
                "进程 DPI 已预设时 start 仍应成功（vendor 补丁应吞 0x80070005）: {e}"
            ),
        };
        assert!(host.is_alive(), "DPI 预设后 host 应 alive");
        host.hide();
        eprintln!("m0_reactor_host_smoke_after_dpi_preset: start ok after DPI preset");
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
