//! Bridge 推送 hub：将事件 JSON 字符串交给 WinUI 宿主 sink。
//!
//! 无 sink 时 `push_json` 为 no-op。宿主接线后 `set_sink` 即可接收推送。
//! 调用方宜**始终** push（不依赖 session.active），避免 WinUI 配置下 stub 回退到
//! webview 后宿主永远收不到事件。

use std::sync::{Arc, Mutex, OnceLock};

use super::protocol::BridgePush;

type SinkFn = Arc<dyn Fn(String) + Send + Sync>;

/// 进程级 Bridge 推送中心。
pub struct BridgePushHub {
    sink: Mutex<Option<SinkFn>>,
}

impl BridgePushHub {
    fn new() -> Self {
        Self {
            sink: Mutex::new(None),
        }
    }

    /// 注册或清除宿主回调（`None` 清除）。
    pub fn set_sink(&self, f: Option<Arc<dyn Fn(String) + Send + Sync>>) {
        if let Ok(mut guard) = self.sink.lock() {
            *guard = f;
        }
    }

    /// 组装 [`BridgePush`] 序列化后交给 sink；无 sink 或锁失败时 no-op。
    pub fn push_json(&self, type_name: &str, payload: serde_json::Value) {
        let sink = {
            let Ok(guard) = self.sink.lock() else {
                return;
            };
            guard.clone()
        };
        let Some(sink) = sink else {
            return;
        };

        let push = BridgePush {
            bridge_version: 1,
            type_name: type_name.to_string(),
            payload: Some(payload),
        };
        match serde_json::to_string(&push) {
            Ok(s) => sink(s),
            Err(error) => log::warn!("BridgePush 序列化失败 type={type_name}: {error}"),
        }
    }
}

/// 进程级单例 hub。
pub fn global() -> &'static BridgePushHub {
    static HUB: OnceLock<BridgePushHub> = OnceLock::new();
    HUB.get_or_init(BridgePushHub::new)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex as StdMutex;

    #[test]
    fn push_json_delivers_bridge_version_string_when_sink_set() {
        let hub = BridgePushHub::new();
        let received: Arc<StdMutex<Option<String>>> = Arc::new(StdMutex::new(None));
        let received_c = Arc::clone(&received);
        hub.set_sink(Some(Arc::new(move |s: String| {
            *received_c.lock().unwrap() = Some(s);
        })));

        hub.push_json(
            "translation_event",
            serde_json::json!({
                "type": "delta",
                "sessionId": "b1:svc",
                "text": "你好"
            }),
        );

        let raw = received.lock().unwrap().clone().expect("应收到 push 字符串");
        assert!(raw.contains("bridgeVersion"), "应含 bridgeVersion: {raw}");
        assert!(
            raw.contains("\"bridgeVersion\":1"),
            "应含 bridgeVersion:1: {raw}"
        );
        assert!(raw.contains("translation_event"));
        assert!(raw.contains("sessionId"));
    }

    #[test]
    fn push_json_noop_without_sink() {
        let hub = BridgePushHub::new();
        // 不应 panic
        hub.push_json("show_context", serde_json::json!({ "sourceText": "" }));
    }
}
