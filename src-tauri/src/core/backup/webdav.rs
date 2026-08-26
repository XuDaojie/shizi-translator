//! 最小 WebDAV 客户端：Basic 认证 + PROPFIND / MKCOL / PUT / GET。

use std::time::Duration;

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use reqwest::{
    header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE},
    Client, Method, StatusCode,
};
use serde::Serialize;

use crate::core::config::normalize_webdav_remote_path;

#[derive(Debug, thiserror::Error)]
pub enum WebDavError {
    #[error("{0}")]
    Message(String),
    #[error("网络错误: {0}")]
    Network(String),
    #[error("HTTP {status}: {body}")]
    Http { status: u16, body: String },
}

impl WebDavError {
    pub fn msg(s: impl Into<String>) -> Self {
        Self::Message(s.into())
    }
}

#[derive(Debug, Clone)]
pub struct WebDavClient {
    base_url: String,
    username: String,
    password: String,
    client: Client,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteListItem {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub size_label: String,
}

impl WebDavClient {
    pub fn new(url: &str, username: &str, password: &str) -> Result<Self, WebDavError> {
        let base = url.trim().trim_end_matches('/').to_string();
        if base.is_empty() {
            return Err(WebDavError::msg("请填写服务器 URL"));
        }
        if !base.starts_with("https://") && !base.starts_with("http://") {
            return Err(WebDavError::msg("服务器 URL 需以 http:// 或 https:// 开头"));
        }
        if username.trim().is_empty() {
            return Err(WebDavError::msg("请填写用户名"));
        }
        if password.is_empty() {
            return Err(WebDavError::msg("请填写密码"));
        }
        let client = Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| WebDavError::Network(e.to_string()))?;
        Ok(Self {
            base_url: base,
            username: username.trim().to_string(),
            password: password.to_string(),
            client,
        })
    }

    fn auth_header(&self) -> HeaderValue {
        let token = B64.encode(format!("{}:{}", self.username, self.password));
        HeaderValue::from_str(&format!("Basic {token}"))
            .unwrap_or_else(|_| HeaderValue::from_static("Basic"))
    }

    fn headers_basic(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, self.auth_header());
        headers
    }

    pub fn resource_url(&self, remote_path: &str) -> String {
        join_webdav_url(&self.base_url, remote_path)
    }

    async fn send(
        &self,
        method: Method,
        url: &str,
        extra: impl FnOnce(&mut HeaderMap),
        body: Option<Vec<u8>>,
    ) -> Result<reqwest::Response, WebDavError> {
        let mut headers = self.headers_basic();
        extra(&mut headers);
        let mut req = self.client.request(method, url).headers(headers);
        if let Some(b) = body {
            req = req.body(b);
        }
        req.send()
            .await
            .map_err(|e| WebDavError::Network(e.to_string()))
    }

    pub async fn propfind(&self, remote_path: &str, depth: u32) -> Result<String, WebDavError> {
        let url = self.resource_url(remote_path);
        let body = br#"<?xml version="1.0" encoding="utf-8" ?>
<d:propfind xmlns:d="DAV:">
  <d:prop>
    <d:displayname/>
    <d:getcontentlength/>
    <d:resourcetype/>
    <d:getlastmodified/>
  </d:prop>
</d:propfind>"#
            .to_vec();
        let response = self
            .send(
                Method::from_bytes(b"PROPFIND").unwrap(),
                &url,
                |h| {
                    h.insert("Depth", HeaderValue::from_str(&depth.to_string()).unwrap());
                    h.insert(CONTENT_TYPE, HeaderValue::from_static("application/xml"));
                },
                Some(body),
            )
            .await?;
        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| WebDavError::Network(e.to_string()))?;
        // 207 Multi-Status 或 200
        if status == StatusCode::MULTI_STATUS
            || status.is_success()
            || status.as_u16() == 207
        {
            return Ok(text);
        }
        if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
            return Err(WebDavError::msg("认证失败，请检查用户名与密码（坚果云请用应用密码）"));
        }
        if status == StatusCode::NOT_FOUND {
            return Err(WebDavError::msg("远端路径不存在"));
        }
        Err(WebDavError::Http {
            status: status.as_u16(),
            body: text.chars().take(200).collect(),
        })
    }

    pub async fn mkcol(&self, remote_path: &str) -> Result<(), WebDavError> {
        let url = self.resource_url(remote_path);
        let response = self
            .send(
                Method::from_bytes(b"MKCOL").unwrap(),
                &url,
                |_| {},
                None,
            )
            .await?;
        let status = response.status();
        // 201 Created；已存在 405/409/301 等可忽略
        if status.is_success()
            || status == StatusCode::METHOD_NOT_ALLOWED
            || status == StatusCode::CONFLICT
            || status.as_u16() == 405
            || status.as_u16() == 301
            || status.as_u16() == 302
        {
            return Ok(());
        }
        if status == StatusCode::UNAUTHORIZED {
            return Err(WebDavError::msg("认证失败，无法创建目录"));
        }
        let body = response
            .text()
            .await
            .unwrap_or_default()
            .chars()
            .take(200)
            .collect::<String>();
        Err(WebDavError::Http {
            status: status.as_u16(),
            body,
        })
    }

    pub async fn put(&self, remote_path: &str, bytes: Vec<u8>) -> Result<(), WebDavError> {
        let url = self.resource_url(remote_path);
        let response = self
            .send(
                Method::PUT,
                &url,
                |h| {
                    h.insert(
                        CONTENT_TYPE,
                        HeaderValue::from_static("application/zip"),
                    );
                },
                Some(bytes),
            )
            .await?;
        let status = response.status();
        if status.is_success() || status == StatusCode::CREATED || status == StatusCode::NO_CONTENT {
            return Ok(());
        }
        if status == StatusCode::UNAUTHORIZED {
            return Err(WebDavError::msg("认证失败，无法上传备份"));
        }
        let body = response
            .text()
            .await
            .unwrap_or_default()
            .chars()
            .take(200)
            .collect::<String>();
        Err(WebDavError::Http {
            status: status.as_u16(),
            body,
        })
    }

    pub async fn get(&self, remote_path: &str) -> Result<Vec<u8>, WebDavError> {
        let url = self.resource_url(remote_path);
        let response = self
            .send(Method::GET, &url, |_| {}, None)
            .await?;
        let status = response.status();
        if !status.is_success() {
            if status == StatusCode::NOT_FOUND {
                return Err(WebDavError::msg("远端备份文件不存在"));
            }
            if status == StatusCode::UNAUTHORIZED {
                return Err(WebDavError::msg("认证失败，无法下载备份"));
            }
            let body = response
                .text()
                .await
                .unwrap_or_default()
                .chars()
                .take(200)
                .collect::<String>();
            return Err(WebDavError::Http {
                status: status.as_u16(),
                body,
            });
        }
        response
            .bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|e| WebDavError::Network(e.to_string()))
    }
}

/// 将 base（无尾 `/`）与 remote path 拼成完整 URL。
pub fn join_webdav_url(base: &str, remote_path: &str) -> String {
    let base = base.trim().trim_end_matches('/');
    let path = if remote_path.starts_with('/') {
        remote_path.to_string()
    } else {
        format!("/{remote_path}")
    };
    // 百分号编码路径段中的非 ASCII，保留 / 
    let encoded = path
        .split('/')
        .map(|seg| {
            if seg.is_empty() {
                String::new()
            } else {
                urlencoding_minimal(seg)
            }
        })
        .collect::<Vec<_>>()
        .join("/");
    format!("{base}{encoded}")
}

fn urlencoding_minimal(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// 逐级 MKCOL 创建目录（如 `/shizi/`）。
pub async fn mkcol_recursive(client: &WebDavClient, dir: &str) -> Result<(), WebDavError> {
    let dir = normalize_webdav_remote_path(dir);
    let parts: Vec<&str> = dir.split('/').filter(|p| !p.is_empty()).collect();
    let mut acc = String::from("/");
    for part in parts {
        acc.push_str(part);
        acc.push('/');
        client.mkcol(&acc).await?;
    }
    Ok(())
}

/// PROPFIND 失败是否表示远端集合尚未就绪（应先 MKCOL）。
/// 坚果云对缺失祖先路径返回 409 AncestorsNotFound，而不是 404。
fn propfind_indicates_missing_collection(err: &WebDavError) -> bool {
    match err {
        WebDavError::Message(m) if m.contains("不存在") => true,
        WebDavError::Http { status: 404 | 409, .. } => true,
        WebDavError::Http { body, .. }
            if body.to_ascii_lowercase().contains("ancestorsnotfound") =>
        {
            true
        }
        _ => false,
    }
}

pub async fn test_connection(client: &WebDavClient, remote_dir: &str) -> Result<(), WebDavError> {
    let dir = normalize_webdav_remote_path(remote_dir);
    match client.propfind(&dir, 0).await {
        Ok(_) => Ok(()),
        Err(e) if propfind_indicates_missing_collection(&e) => {
            mkcol_recursive(client, &dir).await?;
            client.propfind(&dir, 0).await.map(|_| ())
        }
        Err(e) => Err(e),
    }
}

pub async fn put_bytes(
    client: &WebDavClient,
    remote_file_path: &str,
    bytes: Vec<u8>,
) -> Result<(), WebDavError> {
    // 确保父目录存在
    if let Some(parent) = remote_file_path.rsplit_once('/').map(|(p, _)| format!("{p}/")) {
        if parent != "/" {
            mkcol_recursive(client, &parent).await?;
        }
    }
    client.put(remote_file_path, bytes).await
}

pub async fn list_directory(
    client: &WebDavClient,
    remote_dir: &str,
) -> Result<Vec<RemoteListItem>, WebDavError> {
    let dir = normalize_webdav_remote_path(remote_dir);
    let xml = client.propfind(&dir, 1).await?;
    Ok(parse_propfind_list(&xml, &dir))
}

/// 从 PROPFIND multistatus 中提取文件名与大小（轻量字符串解析，不引 XML crate）。
pub fn parse_propfind_list(xml: &str, dir: &str) -> Vec<RemoteListItem> {
    let dir_norm = normalize_webdav_remote_path(dir);
    let mut items = Vec::new();
    // 按 response 块切分（兼容 d:response / D:response / response）
    let lower = xml.to_ascii_lowercase();
    let mut search_from = 0;
    while let Some(start_rel) = find_response_start(&lower[search_from..]) {
        let start = search_from + start_rel;
        let end = lower[start..]
            .find("</d:response>")
            .or_else(|| lower[start..].find("</response>"))
            .map(|i| start + i)
            .unwrap_or(xml.len());
        let block = &xml[start..end.min(xml.len())];
        search_from = end;

        let href = extract_tag_text(block, "href").unwrap_or_default();
        if href.is_empty() {
            continue;
        }
        // 解码路径中的 %XX，取路径部分
        let path = href_to_path(&href);
        if path.is_empty() {
            continue;
        }
        // 跳过目录自身
        let path_as_dir = if path.ends_with('/') {
            path.clone()
        } else {
            format!("{path}/")
        };
        if path_as_dir == dir_norm || path.trim_end_matches('/') == dir_norm.trim_end_matches('/') {
            continue;
        }
        // 集合资源（目录）跳过
        if block.to_ascii_lowercase().contains("<d:collection")
            || block.to_ascii_lowercase().contains("<collection")
            || path.ends_with('/')
        {
            continue;
        }
        let name = path
            .rsplit('/')
            .find(|s| !s.is_empty())
            .unwrap_or("")
            .to_string();
        if name.is_empty() {
            continue;
        }
        // 仅备份文件
        if !name.starts_with("shizi-backup-") || !name.to_ascii_lowercase().ends_with(".zip") {
            continue;
        }
        let size = extract_tag_text(block, "getcontentlength")
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);
        // 远端 path 用 dir + name 统一
        let remote_path = format!("{dir_norm}{name}");
        items.push(RemoteListItem {
            name: name.clone(),
            path: remote_path,
            size,
            size_label: format_size_label(size),
        });
    }
    items.sort_by(|a, b| b.name.cmp(&a.name));
    items
}

fn find_response_start(haystack: &str) -> Option<usize> {
    // haystack 已是 ascii lowercase
    ["<d:response", "<response"]
        .into_iter()
        .filter_map(|p| haystack.find(p))
        .min()
}

fn extract_tag_text(block: &str, local_name: &str) -> Option<String> {
    let lower = block.to_ascii_lowercase();
    let needle = format!(":{local_name}>");
    let bare = format!("<{local_name}>");
    let start = lower
        .find(&needle)
        .map(|i| {
            // 找到 :name> 后内容起点
            i + needle.len()
        })
        .or_else(|| {
            lower.find(&bare).map(|i| i + bare.len())
        })?;
    let rest_lower = &lower[start..];
    let end_rel = rest_lower
        .find("</")
        .unwrap_or(rest_lower.len());
    let raw = &block[start..start + end_rel];
    Some(raw.trim().to_string())
}

fn href_to_path(href: &str) -> String {
    // 可能是完整 URL 或绝对路径
    let path = if let Some(idx) = href.find("://") {
        let after_scheme = &href[idx + 3..];
        after_scheme
            .find('/')
            .map(|i| after_scheme[i..].to_string())
            .unwrap_or_else(|| "/".to_string())
    } else {
        href.to_string()
    };
    percent_decode(&path)
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (from_hex(bytes[i + 1]), from_hex(bytes[i + 2])) {
                out.push((h << 4) | l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn from_hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

pub fn format_size_label(size: u64) -> String {
    if size < 1024 {
        format!("{size} B")
    } else if size < 1024 * 1024 {
        format!("{:.0} KB", size as f64 / 1024.0)
    } else {
        format!("{:.1} MB", size as f64 / (1024.0 * 1024.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    fn http_reply(status: u16, reason: &str, body: &str) -> Vec<u8> {
        format!(
            "HTTP/1.1 {status} {reason}\r\nContent-Type: application/xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .into_bytes()
    }

    /// 读完整请求（头 + Content-Length 体）。Windows 上若未读完体就关连接会发 RST，
    /// reqwest 仍在发送 PROPFIND 体时会报 “error sending request”。
    fn read_request(stream: &mut TcpStream) -> Option<(String, String)> {
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .ok()?;
        let mut buf = Vec::new();
        let mut tmp = [0u8; 2048];
        let header_end = loop {
            let n = stream.read(&mut tmp).ok()?;
            if n == 0 {
                return None;
            }
            buf.extend_from_slice(&tmp[..n]);
            if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                break pos;
            }
            if buf.len() > 32 * 1024 {
                return None;
            }
        };
        let headers = String::from_utf8_lossy(&buf[..header_end]);
        let first = headers.lines().next()?;
        let mut parts = first.split_whitespace();
        let method = parts.next()?.to_string();
        let path = parts.next()?.to_string();
        let content_length = headers
            .lines()
            .find_map(|line| {
                let (k, v) = line.split_once(':')?;
                if k.eq_ignore_ascii_case("content-length") {
                    v.trim().parse::<usize>().ok()
                } else {
                    None
                }
            })
            .unwrap_or(0);
        let mut have = buf.len().saturating_sub(header_end + 4);
        while have < content_length {
            let n = stream.read(&mut tmp).ok()?;
            if n == 0 {
                break;
            }
            have += n;
        }
        Some((method, path))
    }

    fn accepted_blocking_stream(stream: TcpStream) -> TcpStream {
        let _ = stream.set_nonblocking(false);
        let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
        let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
        stream
    }

    fn parent_collection(path: &str) -> String {
        let trimmed = path.trim_end_matches('/');
        if trimmed.is_empty() {
            return "/".to_string();
        }
        match trimmed.rsplit_once('/') {
            Some(("", _)) | None => "/".to_string(),
            Some((parent, _)) => format!("{parent}/"),
        }
    }

    fn normalize_col(path: &str) -> String {
        let mut p = path.split('?').next().unwrap_or(path).to_string();
        if !p.starts_with('/') {
            p.insert(0, '/');
        }
        if !p.ends_with('/') {
            p.push('/');
        }
        p
    }

    /// 坚果云风格：祖先不存在时 PROPFIND 返回 409 AncestorsNotFound，而非 404。
    fn spawn_jianguoyun_like_dav() -> (String, Arc<AtomicBool>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock dav");
        let addr = listener.local_addr().expect("local addr");
        let stop = Arc::new(AtomicBool::new(false));
        let stop_flag = stop.clone();
        thread::spawn(move || {
            listener
                .set_nonblocking(true)
                .expect("nonblocking mock dav");
            let mut cols: HashSet<String> = HashSet::from(["/".to_string()]);
            while !stop_flag.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let mut stream = accepted_blocking_stream(stream);
                        let Some((method, raw_path)) = read_request(&mut stream) else {
                            continue;
                        };
                        let path = normalize_col(&raw_path);
                        let reply = match method.as_str() {
                            "PROPFIND" => {
                                if cols.contains(&path) {
                                    http_reply(
                                        207,
                                        "Multi-Status",
                                        r#"<?xml version="1.0"?><d:multistatus xmlns:d="DAV:"><d:response><d:href>/shizi/backups/</d:href><d:propstat><d:prop><d:resourcetype><d:collection/></d:resourcetype></d:prop></d:propstat></d:response></d:multistatus>"#,
                                    )
                                } else if !cols.contains(&parent_collection(&path)) {
                                    http_reply(
                                        409,
                                        "Conflict",
                                        r#"<?xml version="1.0" encoding="UTF-8" standalone="no"?><d:error xmlns:d="DAV:" xmlns:s="http://ns.jianguoyun.com"><s:exception>AncestorsNotFound</s:exception><s:message>The ancestors of this location do not exist</s:message></d:error>"#,
                                    )
                                } else {
                                    http_reply(404, "Not Found", "missing")
                                }
                            }
                            "MKCOL" => {
                                if cols.contains(&path) {
                                    http_reply(405, "Method Not Allowed", "")
                                } else if !cols.contains(&parent_collection(&path)) {
                                    http_reply(
                                        409,
                                        "Conflict",
                                        r#"<s:exception>AncestorsNotFound</s:exception>"#,
                                    )
                                } else {
                                    cols.insert(path);
                                    http_reply(201, "Created", "")
                                }
                            }
                            _ => http_reply(405, "Method Not Allowed", ""),
                        };
                        let _ = stream.write_all(&reply);
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(_) => break,
                }
            }
        });
        (format!("http://{addr}"), stop)
    }

    fn spawn_always_unauthorized() -> (String, Arc<AtomicBool>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock dav");
        let addr = listener.local_addr().expect("local addr");
        let stop = Arc::new(AtomicBool::new(false));
        let stop_flag = stop.clone();
        thread::spawn(move || {
            listener
                .set_nonblocking(true)
                .expect("nonblocking mock dav");
            while !stop_flag.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let mut stream = accepted_blocking_stream(stream);
                        if read_request(&mut stream).is_some() {
                            let reply = http_reply(401, "Unauthorized", "no");
                            let _ = stream.write_all(&reply);
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(_) => break,
                }
            }
        });
        (format!("http://{addr}"), stop)
    }

    #[tokio::test]
    async fn test_connection_creates_nested_dir_when_propfind_returns_409() {
        let (url, stop) = spawn_jianguoyun_like_dav();
        let client = WebDavClient::new(&url, "user", "app-password").unwrap();
        let result = test_connection(&client, "/shizi/backups/").await;
        stop.store(true, Ordering::SeqCst);
        result.expect("坚果云在祖先目录不存在时对 PROPFIND 返回 409，测试连接应逐级建目录后成功");
    }

    #[tokio::test]
    async fn test_connection_still_fails_on_unauthorized() {
        let (url, stop) = spawn_always_unauthorized();
        let client = WebDavClient::new(&url, "user", "wrong").unwrap();
        let result = test_connection(&client, "/shizi/backups/").await;
        stop.store(true, Ordering::SeqCst);
        let err = result.expect_err("认证失败不应被当成目录缺失");
        let msg = err.to_string();
        assert!(
            msg.contains("认证失败") || msg.contains("401"),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn join_url_encodes_path() {
        assert_eq!(
            join_webdav_url("https://dav.example.com/dav", "/shizi/backups/a.zip"),
            "https://dav.example.com/dav/shizi/backups/a.zip"
        );
    }

    #[test]
    fn parse_propfind_extracts_backup_zips() {
        let xml = r#"<?xml version="1.0"?>
<d:multistatus xmlns:d="DAV:">
  <d:response>
    <d:href>/dav/shizi/backups/</d:href>
    <d:propstat><d:prop><d:resourcetype><d:collection/></d:resourcetype></d:prop></d:propstat>
  </d:response>
  <d:response>
    <d:href>/dav/shizi/backups/shizi-backup-20260805-120000.zip</d:href>
    <d:propstat><d:prop>
      <d:getcontentlength>12345</d:getcontentlength>
      <d:resourcetype/>
    </d:prop></d:propstat>
  </d:response>
  <d:response>
    <d:href>/dav/shizi/backups/readme.txt</d:href>
    <d:propstat><d:prop><d:getcontentlength>1</d:getcontentlength></d:prop></d:propstat>
  </d:response>
</d:multistatus>"#;
        let items = parse_propfind_list(xml, "/shizi/backups/");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "shizi-backup-20260805-120000.zip");
        assert_eq!(items[0].size, 12345);
        assert_eq!(items[0].path, "/shizi/backups/shizi-backup-20260805-120000.zip");
    }

    #[test]
    fn format_size_label_works() {
        assert_eq!(format_size_label(500), "500 B");
        assert_eq!(format_size_label(2048), "2 KB");
    }
}
