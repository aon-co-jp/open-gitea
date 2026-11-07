//! Connect の「単純呼び出し(unary)」で受ける API の入口(2026-11-07、REST 全廃方針に基づき新設)。
//!
//! 形式は `POST /open_gitea.v1.Gitea/<Method>`、本文は JSON(`content-type: application/json`)。
//! 読み込みは RS-JSON の `parse_secure`(サイズ・深さ・重複キーの上限つき)。
//! エラーは Connect の形式 `{"code": "...", "message": "..."}`。
//!
//! 旧 REST の経路(`/api/...`・`PUT /repos/:name`)は**公開ルーターから外し**、この入口が
//! 内部の(外から見えない)ルーターへ橋渡しするだけにした。業務ロジック(認証・権限・検証)は
//! 従来のハンドラのまま共有する(`open-easy-web` の `connect.rs` の `finish` と同じ作り)。
//! 認証は従来どおり `Authorization: Bearer <セッション>`(git CLI は Basic)。
//! グループ用トークンは `X-RGit-Group-Token` ヘッダ、またはクエリ `?token=`。
//!
//! 例外(Connect にしないもの): git smart HTTP(`info/refs`・`git-upload-pack`・`git-receive-pack`、Git の規格)、
//! `/healthz`、`/ui`(静的ファイル)、ブラウザが直接開くダウンロード用 GET の
//! `/api/repos/:name/zip`・`/api/repos/:name/raw/*`(リンクとして共有され `?token=` で認証されるバイナリ配信)。

use std::sync::Arc;

use poem::http::{header, Method, StatusCode};
use poem::{Endpoint, EndpointExt, Request, Response};
use serde_json::Value;

pub const PREFIX: &str = "/open_gitea.v1.Gitea/";
/// 本文の上限(Issue 本文などを考慮して 1 MiB)。
const BODY_MAX: usize = 1024 * 1024;

/// (メソッド名, HTTP 動詞, 内部パス, パスに入れる JSON フィールド)。
/// 動詞が GET/DELETE の呼び出しは本文を内部へ渡さない。
const TABLE: &[(&str, &str, &str, &[&str])] = &[
    ("ListRepos", "GET", "/api/repos", &[]),
    ("CreateRepo", "PUT", "/repos/{}", &["name"]),
    ("GetReadme", "GET", "/api/repos/{}/readme", &["name"]),
    ("GetReleases", "GET", "/api/repos/{}/releases", &["name"]),
    ("ListWiki", "GET", "/api/repos/{}/wiki", &["name"]),
    ("GetWikiPage", "GET", "/api/repos/{}/wiki/{}", &["name", "page"]),
    ("GetTree", "GET", "/api/repos/{}/tree", &["name"]),
    ("ListIssues", "GET", "/api/repos/{}/issues", &["name"]),
    ("CreateIssue", "POST", "/api/repos/{}/issues", &["name"]),
    ("GetIssue", "GET", "/api/repos/{}/issues/{}", &["name", "id"]),
    ("SetIssueStatus", "PUT", "/api/repos/{}/issues/{}", &["name", "id"]),
    ("UpdateIssueMetadata", "PATCH", "/api/repos/{}/issues/{}/metadata", &["name", "id"]),
    ("CreateIssueComment", "POST", "/api/repos/{}/issues/{}/comments", &["name", "id"]),
    ("GetAccess", "GET", "/api/repos/{}/access", &["name"]),
    ("SetAccess", "PUT", "/api/repos/{}/access", &["name"]),
    ("RequestOtp", "POST", "/api/auth/request-otp", &[]),
    ("VerifyOtp", "POST", "/api/auth/verify-otp", &[]),
    ("DemoLogin", "POST", "/api/auth/demo-login", &[]),
    ("Logout", "POST", "/api/auth/logout", &[]),
    ("ListGroups", "GET", "/api/groups", &[]),
    ("CreateGroup", "POST", "/api/groups", &[]),
    ("DeleteGroup", "DELETE", "/api/groups/{}", &["name"]),
    ("ListAccounts", "GET", "/api/accounts", &[]),
    ("AddAccount", "POST", "/api/accounts", &[]),
    ("GetAccount", "GET", "/api/accounts/{}", &["email"]),
    ("RemoveAccount", "DELETE", "/api/accounts/{}", &["email"]),
    ("SetCreatePermission", "PUT", "/api/accounts/{}/create-permission", &["email"]),
    ("RequestAccess", "POST", "/api/accounts/request", &[]),
    ("ListAccessRequests", "GET", "/api/accounts/requests", &[]),
    ("DecideAccessRequest", "POST", "/api/accounts/requests/{}/decide", &["id"]),
    ("GetCapacity", "GET", "/api/capacity", &[]),
    ("GetPowerProfile", "GET", "/api/power-profile", &[]),
];

fn code_of(status: StatusCode) -> &'static str {
    match status.as_u16() {
        400 => "invalid_argument",
        401 => "unauthenticated",
        403 => "permission_denied",
        404 => "not_found",
        405 | 501 => "unimplemented",
        409 => "aborted",
        413 | 429 => "resource_exhausted",
        502 | 503 | 504 => "unavailable",
        s if s >= 500 => "internal",
        _ => "unknown",
    }
}

fn fail(status: StatusCode, message: &str) -> Response {
    let body = serde_json::json!({ "code": code_of(status), "message": message });
    Response::builder()
        .status(status)
        .content_type("application/json")
        .header("cache-control", "no-store")
        .body(serde_json::to_vec(&body).unwrap_or_default())
}

fn encode_segment(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// 内部ルーターを包む Connect の入口エンドポイント。
pub fn endpoint(inner: impl Endpoint + 'static) -> impl Endpoint<Output = Response> {
    let inner = Arc::new(inner.map_to_response().boxed());
    poem::endpoint::make(move |mut req: Request| {
        let inner = inner.clone();
        async move {
            if req.method() != Method::POST {
                return fail(StatusCode::METHOD_NOT_ALLOWED, "POST only");
            }
            let name = req.uri().path().strip_prefix(PREFIX).unwrap_or("").to_string();
            let Some(&(_, verb, template, keys)) = TABLE.iter().find(|(n, ..)| *n == name) else {
                return fail(StatusCode::NOT_FOUND, "unknown method");
            };
            let ct = req.header(header::CONTENT_TYPE).unwrap_or("").to_ascii_lowercase();
            if !ct.starts_with("application/json") {
                return fail(StatusCode::BAD_REQUEST, "content-type must be application/json");
            }
            let bytes = match req.take_body().into_bytes_limit(BODY_MAX).await {
                Ok(b) => b,
                Err(_) => return fail(StatusCode::PAYLOAD_TOO_LARGE, "request body too large"),
            };
            let mut obj = if bytes.iter().all(|b| b.is_ascii_whitespace()) {
                serde_json::Map::new()
            } else {
                let Ok(text) = std::str::from_utf8(&bytes) else {
                    return fail(StatusCode::BAD_REQUEST, "body must be UTF-8");
                };
                // サイズ・深さ・重複キーの上限つきで検査してから読む。
                if let Err(e) = rust_json::parse_secure(text, &rust_json::Limits::small()) {
                    return fail(StatusCode::BAD_REQUEST, &e.to_string());
                }
                match serde_json::from_str::<Value>(text) {
                    Ok(Value::Object(m)) => m,
                    Ok(_) => return fail(StatusCode::BAD_REQUEST, "body must be a JSON object"),
                    Err(e) => return fail(StatusCode::BAD_REQUEST, &e.to_string()),
                }
            };
            let mut path = String::new();
            let mut parts = template.split("{}");
            path.push_str(parts.next().unwrap_or(""));
            for (key, tail) in keys.iter().zip(parts) {
                let v = match obj.remove(*key) {
                    Some(Value::String(s)) if !s.is_empty() => s,
                    Some(Value::Number(n)) => n.to_string(),
                    _ => return fail(StatusCode::BAD_REQUEST, &format!("missing field: {key}")),
                };
                path.push_str(&encode_segment(&v));
                path.push_str(tail);
            }
            if let Some(q) = req.uri().query() {
                path.push('?');
                path.push_str(q);
            }
            let has_body = !matches!(verb, "GET" | "DELETE");
            let method = Method::from_bytes(verb.as_bytes()).unwrap_or(Method::GET);
            let mut b = Request::builder().method(method).uri_str(&path);
            for h in [header::AUTHORIZATION.as_str(), "x-rgit-group-token"] {
                if let Some(v) = req.header(h) {
                    b = b.header(h, v);
                }
            }
            let inner_req = if has_body {
                b.content_type("application/json").body(serde_json::to_vec(&Value::Object(obj)).unwrap_or_default())
            } else {
                b.body(())
            };
            let resp = inner.get_response(inner_req).await;
            let status = resp.status();
            let out = resp.into_body().into_bytes().await.unwrap_or_default();
            if status.is_success() {
                let body = if out.is_empty() { b"{}".to_vec() } else { out.to_vec() };
                return Response::builder()
                    .status(StatusCode::OK)
                    .content_type("application/json")
                    .header("cache-control", "no-store")
                    .body(body);
            }
            let text = String::from_utf8_lossy(&out);
            // 内部が JSON で返した場合は本文から message を取り出す。
            let msg = serde_json::from_str::<Value>(&text)
                .ok()
                .and_then(|v| v.get("error").or(v.get("message")).and_then(|m| m.as_str().map(str::to_string)))
                .unwrap_or_else(|| text.to_string());
            fail(status, &msg)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn table_placeholders_match_keys() {
        for (n, _, t, keys) in TABLE {
            assert_eq!(t.matches("{}").count(), keys.len(), "{n}");
        }
    }
    #[test]
    fn encode() {
        assert_eq!(encode_segment("a@b.c/d"), "a%40b.c%2Fd");
    }
}
