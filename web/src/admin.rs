//! 管理者向けUI(アクセス申請一覧・登録アカウント管理・グループ管理・
//! リポジトリ別アクセス設定)。
//!
//! `web/src/auth.rs`と同じ方針を踏襲する: 認証付き通信は
//! `auth::authorized_fetch`(`Authorization: Bearer <token>`付与)、JSON
//! パースは`rust_json::parse_light`のみ使用(serde/serde_jsonは使わない)。
//! `light`モジュールは**パース専用でシリアライズ機能を持たない**ため、
//! サーバーへ送るJSON文字列は`auth::json_escape`を使って手組みする
//! (`web/src/auth.rs`のOTPリクエスト構築と同じ手法)。
//!
//! この画面は管理者(`RGIT_ADMIN_EMAIL`)向けだが、WASM側は誰が管理者かを
//! 判定する手段を持たない(サーバー側`require_admin_session`が唯一の
//! ゲート)。そのため**ログインしていれば誰でもこのパネルを開こうとする**
//! 実装にしてあり、管理者でない場合は各セクションが`401`/`403`を受けて
//! エラーメッセージを表示するだけに留める(クラッシュさせない、という
//! タスク要件通り)。`RGIT_ACCOUNTS_LOCKED`(既定`true`)による
//! 管理者以外お断りの`403`も同様にそのままメッセージ表示する。

use crate::parse_string_array;
use crate::auth::{authorized_fetch, json_escape};
use rust_json::{parse_light, LightValue};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{Document, Element, HtmlInputElement, HtmlSelectElement};

fn document() -> Document {
    web_sys::window().expect("no window").document().expect("no document")
}

fn set_text(id: &str, text: &str) {
    if let Some(el) = document().get_element_by_id(id) {
        el.set_text_content(Some(text));
    }
}

fn set_html(id: &str, html: &str) {
    if let Some(el) = document().get_element_by_id(id) {
        el.set_inner_html(html);
    }
}

fn show(id: &str, visible: bool) {
    if let Some(el) = document().get_element_by_id(id) {
        if visible {
            el.class_list().remove_1("hidden").ok();
        } else {
            el.class_list().add_1("hidden").ok();
        }
    }
}

fn input_value(id: &str) -> String {
    document()
        .get_element_by_id(id)
        .and_then(|el| el.dyn_into::<HtmlInputElement>().ok())
        .map(|input| input.value())
        .unwrap_or_default()
}

fn clear_input(id: &str) {
    if let Some(el) = document().get_element_by_id(id).and_then(|el| el.dyn_into::<HtmlInputElement>().ok()) {
        el.set_value("");
    }
}

fn select_value(id: &str) -> String {
    document()
        .get_element_by_id(id)
        .and_then(|el| el.dyn_into::<HtmlSelectElement>().ok())
        .map(|sel| sel.value())
        .unwrap_or_default()
}

fn set_select_value(id: &str, value: &str) {
    if let Some(sel) = document().get_element_by_id(id).and_then(|el| el.dyn_into::<HtmlSelectElement>().ok()) {
        sel.set_value(value);
    }
}

fn checkbox_checked(id: &str) -> bool {
    document()
        .get_element_by_id(id)
        .and_then(|el| el.dyn_into::<HtmlInputElement>().ok())
        .map(|input| input.checked())
        .unwrap_or(false)
}

fn set_checkbox(id: &str, checked: bool) {
    if let Some(input) = document().get_element_by_id(id).and_then(|el| el.dyn_into::<HtmlInputElement>().ok()) {
        input.set_checked(checked);
    }
}

/// `&`/`<`/`>`/`"`をエスケープする(自前で組み立てたHTML断片に、
/// メールアドレス・メッセージ等ユーザー由来の文字列を埋め込むため)。
fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

fn show_admin_error(msg: &str) {
    set_text("admin-error", msg);
}

fn closest(el: &Element, selector: &str) -> Option<Element> {
    el.closest(selector).ok().flatten()
}

fn query_bool(scope: &Element, selector: &str) -> bool {
    scope
        .query_selector(selector)
        .ok()
        .flatten()
        .and_then(|el| el.dyn_into::<HtmlInputElement>().ok())
        .map(|input| input.checked())
        .unwrap_or(false)
}

// --- アクセス申請一覧 ---

async fn refresh_requests() {
    match authorized_fetch("/api/accounts/requests", "GET", None).await {
        Ok((200, text)) => {
            let Ok(value) = parse_light(&text) else {
                set_html("requests-list", "<li>応答の解析に失敗しました</li>");
                return;
            };
            let Some(items) = value.as_array() else {
                set_html("requests-list", "<li>申請はありません</li>");
                return;
            };
            if items.is_empty() {
                set_html("requests-list", "<li>保留中の申請はありません</li>");
                return;
            }
            let mut html = String::new();
            for item in items {
                let id = item.get("id").and_then(LightValue::as_str).unwrap_or("");
                let email = item.get("email").and_then(LightValue::as_str).unwrap_or("");
                let repo = item.get("repo").and_then(LightValue::as_str);
                let message = item.get("message").and_then(LightValue::as_str);
                let is_create = item.get("is_create_repo_request").and_then(LightValue::as_bool).unwrap_or(false);
                html.push_str(&format!(
                    "<li data-id=\"{id}\">\
                     <div><strong>{email}</strong> \
                     {repo_label}{create_label}</div>\
                     {message_html}\
                     <label><input type=\"checkbox\" class=\"req-view\" checked> 閲覧</label> \
                     <label><input type=\"checkbox\" class=\"req-dl\" checked> ダウンロード</label> \
                     <label><input type=\"checkbox\" class=\"req-push\"> push</label> \
                     <button type=\"button\" class=\"btn-approve\" data-id=\"{id}\">承認</button> \
                     <button type=\"button\" class=\"btn-deny\" data-id=\"{id}\">却下</button>\
                     </li>",
                    id = html_escape(id),
                    email = html_escape(email),
                    repo_label = repo.map(|r| format!(" → {}", html_escape(r))).unwrap_or_else(|| " (アカウント登録のみ)".to_string()),
                    create_label = if is_create { " [新規リポジトリ作成申請]" } else { "" },
                    message_html = message
                        .filter(|m| !m.is_empty())
                        .map(|m| format!("<div><em>{}</em></div>", html_escape(m)))
                        .unwrap_or_default(),
                ));
            }
            set_html("requests-list", &html);
        }
        Ok((401, _)) | Ok((403, _)) => set_html("requests-list", "<li>管理者ログインが必要です</li>"),
        _ => set_html("requests-list", "<li>読み込みに失敗しました</li>"),
    }
}

async fn decide_request(id: String, approve: bool, allow_view: bool, allow_download: bool, allow_push: bool) {
    show_admin_error("");
    let body = format!(
        r#"{{"approve":{approve},"allow_view":{allow_view},"allow_download":{allow_download},"allow_push":{allow_push}}}"#
    );
    let url = format!("/api/accounts/requests/{}/decide", json_escape(&id));
    match authorized_fetch(&url, "POST", Some(&body)).await {
        Ok((200, _)) => {
            refresh_requests().await;
            refresh_accounts().await;
        }
        Ok((403, text)) => show_admin_error(&format!("拒否されました: {text}")),
        Ok((status, text)) => show_admin_error(&format!("申請の処理に失敗しました(status {status}): {text}")),
        Err(_) => show_admin_error("申請の処理に失敗しました(通信エラー)"),
    }
}

fn wire_requests_list() {
    let doc = document();
    let Some(list) = doc.get_element_by_id("requests-list") else { return };
    let closure = Closure::<dyn FnMut(web_sys::Event)>::new(move |event: web_sys::Event| {
        let Some(target) = event.target() else { return };
        let Ok(el) = target.dyn_into::<Element>() else { return };
        let approve = el.class_list().contains("btn-approve");
        let deny = el.class_list().contains("btn-deny");
        if !approve && !deny {
            return;
        }
        let Some(id) = el.get_attribute("data-id") else { return };
        let Some(li) = closest(&el, "li") else { return };
        let allow_view = if approve { query_bool(&li, ".req-view") } else { false };
        let allow_download = if approve { query_bool(&li, ".req-dl") } else { false };
        let allow_push = if approve { query_bool(&li, ".req-push") } else { false };
        wasm_bindgen_futures::spawn_local(decide_request(id, approve, allow_view, allow_download, allow_push));
    });
    list.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref()).ok();
    closure.forget();
}

// --- 登録アカウント ---

async fn refresh_accounts() {
    match authorized_fetch("/api/accounts", "GET", None).await {
        Ok((200, text)) => {
            let emails = parse_string_array(&text);
            if emails.is_empty() {
                set_html("accounts-list", "<li>登録アカウントはありません</li>");
                return;
            }
            let mut html = String::new();
            for email in &emails {
                html.push_str(&format!(
                    "<li data-email=\"{email}\">{email} \
                     <button type=\"button\" class=\"btn-allow-create\" data-email=\"{email}\">作成許可ON</button> \
                     <button type=\"button\" class=\"btn-deny-create\" data-email=\"{email}\">作成許可OFF</button> \
                     <button type=\"button\" class=\"btn-remove-account\" data-email=\"{email}\">削除</button></li>",
                    email = html_escape(email)
                ));
            }
            set_html("accounts-list", &html);
        }
        Ok((401, _)) | Ok((403, _)) => set_html("accounts-list", "<li>管理者ログインが必要です</li>"),
        _ => set_html("accounts-list", "<li>読み込みに失敗しました</li>"),
    }
}

async fn add_account() {
    show_admin_error("");
    let email = input_value("new-account-email");
    if !email.contains('@') {
        show_admin_error("メールアドレスを入力してください");
        return;
    }
    let body = format!(r#"{{"email":"{}"}}"#, json_escape(&email));
    match authorized_fetch("/api/accounts", "POST", Some(&body)).await {
        Ok((201, _)) => {
            clear_input("new-account-email");
            refresh_accounts().await;
        }
        Ok((403, text)) => show_admin_error(&format!("アカウント登録が制限されています: {text}")),
        Ok((status, text)) => show_admin_error(&format!("アカウント追加に失敗しました(status {status}): {text}")),
        Err(_) => show_admin_error("アカウント追加に失敗しました(通信エラー)"),
    }
}

async fn remove_account(email: String) {
    show_admin_error("");
    let url = format!("/api/accounts/{}", json_escape(&email));
    match authorized_fetch(&url, "DELETE", None).await {
        Ok((200, _)) => refresh_accounts().await,
        Ok((status, text)) => show_admin_error(&format!("削除に失敗しました(status {status}): {text}")),
        Err(_) => show_admin_error("削除に失敗しました(通信エラー)"),
    }
}

async fn set_create_permission(email: String, allow: bool) {
    show_admin_error("");
    let body = format!(r#"{{"allow":{allow}}}"#);
    let url = format!("/api/accounts/{}/create-permission", json_escape(&email));
    match authorized_fetch(&url, "PUT", Some(&body)).await {
        Ok((200, _)) => {}
        Ok((status, text)) => show_admin_error(&format!("作成許可の変更に失敗しました(status {status}): {text}")),
        Err(_) => show_admin_error("作成許可の変更に失敗しました(通信エラー)"),
    }
}

fn wire_accounts_list() {
    let doc = document();
    let Some(list) = doc.get_element_by_id("accounts-list") else { return };
    let closure = Closure::<dyn FnMut(web_sys::Event)>::new(move |event: web_sys::Event| {
        let Some(target) = event.target() else { return };
        let Ok(el) = target.dyn_into::<Element>() else { return };
        let Some(email) = el.get_attribute("data-email") else { return };
        if el.class_list().contains("btn-remove-account") {
            wasm_bindgen_futures::spawn_local(remove_account(email));
        } else if el.class_list().contains("btn-allow-create") {
            wasm_bindgen_futures::spawn_local(set_create_permission(email, true));
        } else if el.class_list().contains("btn-deny-create") {
            wasm_bindgen_futures::spawn_local(set_create_permission(email, false));
        }
    });
    list.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref()).ok();
    closure.forget();
}

// --- グループ ---

async fn refresh_groups() {
    match authorized_fetch("/api/groups", "GET", None).await {
        Ok((200, text)) => {
            let names = parse_string_array(&text);
            if names.is_empty() {
                set_html("groups-list", "<li>グループはありません</li>");
                return;
            }
            let mut html = String::new();
            for name in &names {
                html.push_str(&format!(
                    "<li data-name=\"{name}\">{name} <button type=\"button\" class=\"btn-remove-group\" data-name=\"{name}\">削除</button></li>",
                    name = html_escape(name)
                ));
            }
            set_html("groups-list", &html);
        }
        Ok((401, _)) | Ok((403, _)) => set_html("groups-list", "<li>管理者ログインが必要です</li>"),
        _ => set_html("groups-list", "<li>読み込みに失敗しました</li>"),
    }
}

async fn create_group() {
    show_admin_error("");
    let name = input_value("new-group-name");
    if name.is_empty() {
        show_admin_error("グループ名を入力してください");
        return;
    }
    let body = format!(r#"{{"name":"{}"}}"#, json_escape(&name));
    match authorized_fetch("/api/groups", "POST", Some(&body)).await {
        Ok((201, text)) => {
            clear_input("new-group-name");
            let token = parse_light(&text).ok().and_then(|v| v.get("token").and_then(LightValue::as_str).map(str::to_string));
            if let Some(token) = token {
                set_text(
                    "group-token-display",
                    &format!("グループ「{name}」を作成しました。招待トークン(この画面でしか表示されません): {token}"),
                );
            }
            refresh_groups().await;
        }
        Ok((409, _)) => show_admin_error("同名のグループが既に存在します"),
        Ok((status, text)) => show_admin_error(&format!("グループ作成に失敗しました(status {status}): {text}")),
        Err(_) => show_admin_error("グループ作成に失敗しました(通信エラー)"),
    }
}

async fn remove_group(name: String) {
    show_admin_error("");
    let url = format!("/api/groups/{}", json_escape(&name));
    match authorized_fetch(&url, "DELETE", None).await {
        Ok((200, _)) => refresh_groups().await,
        Ok((status, text)) => show_admin_error(&format!("削除に失敗しました(status {status}): {text}")),
        Err(_) => show_admin_error("削除に失敗しました(通信エラー)"),
    }
}

fn wire_groups_list() {
    let doc = document();
    let Some(list) = doc.get_element_by_id("groups-list") else { return };
    let closure = Closure::<dyn FnMut(web_sys::Event)>::new(move |event: web_sys::Event| {
        let Some(target) = event.target() else { return };
        let Ok(el) = target.dyn_into::<Element>() else { return };
        if !el.class_list().contains("btn-remove-group") {
            return;
        }
        let Some(name) = el.get_attribute("data-name") else { return };
        wasm_bindgen_futures::spawn_local(remove_group(name));
    });
    list.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref()).ok();
    closure.forget();
}

// --- リポジトリ別アクセス設定 ---

async fn refresh_repo_select() {
    match authorized_fetch("/api/repos", "GET", None).await {
        Ok((200, text)) => {
            let names = parse_string_array(&text);
            let mut html = String::new();
            for name in &names {
                html.push_str(&format!("<option value=\"{name}\">{name}</option>", name = html_escape(name)));
            }
            set_html("access-repo-select", &html);
        }
        _ => set_html("access-repo-select", ""),
    }
}

fn add_access_account_row(email: &str, view: bool, download: bool, push: bool) {
    let doc = document();
    let Some(container) = doc.get_element_by_id("access-accounts-rows") else { return };
    let row = doc.create_element("li").unwrap();
    row.set_attribute("data-email", email).ok();
    row.set_inner_html(&format!(
        "<strong>{email}</strong> \
         <label><input type=\"checkbox\" class=\"acc-view\" {view_checked}> 閲覧</label> \
         <label><input type=\"checkbox\" class=\"acc-dl\" {dl_checked}> ダウンロード</label> \
         <label><input type=\"checkbox\" class=\"acc-push\" {push_checked}> push</label> \
         <button type=\"button\" class=\"btn-remove-access-row\">削除</button>",
        email = html_escape(email),
        view_checked = if view { "checked" } else { "" },
        dl_checked = if download { "checked" } else { "" },
        push_checked = if push { "checked" } else { "" },
    ));
    container.append_child(&row).ok();
}

async fn load_access() {
    show_admin_error("");
    set_text("access-status", "");
    let repo = select_value("access-repo-select");
    if repo.is_empty() {
        show_admin_error("リポジトリを選択してください");
        return;
    }
    let url = format!("/api/repos/{}/access", json_escape(&repo));
    match authorized_fetch(&url, "GET", None).await {
        Ok((200, text)) => {
            let Ok(value) = parse_light(&text) else {
                show_admin_error("応答の解析に失敗しました");
                return;
            };
            let mode = value.get("mode").and_then(LightValue::as_str).unwrap_or("private");
            let group = value.get("group").and_then(LightValue::as_str).unwrap_or("");
            set_select_value("access-mode", mode);
            if let Some(input) = document().get_element_by_id("access-group-name").and_then(|el| el.dyn_into::<HtmlInputElement>().ok()) {
                input.set_value(group);
            }
            set_checkbox("access-allow-view", value.get("allow_view").and_then(LightValue::as_bool).unwrap_or(false));
            set_checkbox("access-allow-download", value.get("allow_download").and_then(LightValue::as_bool).unwrap_or(false));
            set_checkbox("access-allow-push", value.get("allow_push").and_then(LightValue::as_bool).unwrap_or(false));

            set_html("access-accounts-rows", "");
            if let Some(LightValue::Object(entries)) = value.get("accounts") {
                for (email, perm) in entries {
                    let view = perm.get("allow_view").and_then(LightValue::as_bool).unwrap_or(false);
                    let dl = perm.get("allow_download").and_then(LightValue::as_bool).unwrap_or(false);
                    let push = perm.get("allow_push").and_then(LightValue::as_bool).unwrap_or(false);
                    add_access_account_row(email, view, dl, push);
                }
            }
            show("access-form", true);
            set_text("access-status", &format!("{repo} の現在の設定を読み込みました"));
        }
        Ok((401, _)) | Ok((403, _)) => show_admin_error("管理者ログインが必要です"),
        Ok((404, _)) => show_admin_error("リポジトリが見つかりません"),
        Ok((status, text)) => show_admin_error(&format!("読み込みに失敗しました(status {status}): {text}")),
        Err(_) => show_admin_error("読み込みに失敗しました(通信エラー)"),
    }
}

fn add_access_account_row_from_form() {
    let email = input_value("access-new-account-email");
    if !email.contains('@') {
        show_admin_error("追加するアカウントのメールアドレスを入力してください");
        return;
    }
    let view = checkbox_checked("access-new-account-view");
    let dl = checkbox_checked("access-new-account-dl");
    let push = checkbox_checked("access-new-account-push");
    add_access_account_row(&email, view, dl, push);
    clear_input("access-new-account-email");
    set_checkbox("access-new-account-view", false);
    set_checkbox("access-new-account-dl", false);
    set_checkbox("access-new-account-push", false);
}

async fn save_access() {
    show_admin_error("");
    let repo = select_value("access-repo-select");
    if repo.is_empty() {
        show_admin_error("リポジトリを選択してください");
        return;
    }
    let mode = select_value("access-mode");
    let group_raw = input_value("access-group-name");
    let group_json = if mode == "group" && !group_raw.is_empty() {
        format!("\"{}\"", json_escape(&group_raw))
    } else {
        "null".to_string()
    };
    let allow_view = checkbox_checked("access-allow-view");
    let allow_download = checkbox_checked("access-allow-download");
    let allow_push = checkbox_checked("access-allow-push");

    let mut accounts_json = String::new();
    if let Some(container) = document().get_element_by_id("access-accounts-rows") {
        let rows = container.children();
        for i in 0..rows.length() {
            let Some(row) = rows.item(i) else { continue };
            let Some(email) = row.get_attribute("data-email") else { continue };
            let view = query_bool(&row, ".acc-view");
            let dl = query_bool(&row, ".acc-dl");
            let push = query_bool(&row, ".acc-push");
            if !accounts_json.is_empty() {
                accounts_json.push(',');
            }
            accounts_json.push_str(&format!(
                "\"{}\":{{\"allow_view\":{view},\"allow_download\":{dl},\"allow_push\":{push}}}",
                json_escape(&email)
            ));
        }
    }

    let body = format!(
        r#"{{"mode":"{mode}","group":{group_json},"allow_view":{allow_view},"allow_download":{allow_download},"allow_push":{allow_push},"accounts":{{{accounts_json}}}}}"#,
        mode = json_escape(&mode),
    );
    let url = format!("/api/repos/{}/access", json_escape(&repo));
    match authorized_fetch(&url, "PUT", Some(&body)).await {
        Ok((200, _)) => set_text("access-status", &format!("{repo} の設定を保存しました")),
        Ok((400, text)) => show_admin_error(&format!("入力内容が不正です: {text}")),
        Ok((401, _)) | Ok((403, _)) => show_admin_error("管理者ログインが必要です"),
        Ok((status, text)) => show_admin_error(&format!("保存に失敗しました(status {status}): {text}")),
        Err(_) => show_admin_error("保存に失敗しました(通信エラー)"),
    }
}

fn wire_access_accounts_rows() {
    let doc = document();
    let Some(list) = doc.get_element_by_id("access-accounts-rows") else { return };
    let closure = Closure::<dyn FnMut(web_sys::Event)>::new(move |event: web_sys::Event| {
        let Some(target) = event.target() else { return };
        let Ok(el) = target.dyn_into::<Element>() else { return };
        if !el.class_list().contains("btn-remove-access-row") {
            return;
        }
        if let Some(li) = closest(&el, "li") {
            if let Some(parent) = li.parent_element() {
                parent.remove_child(&li).ok();
            }
        }
    });
    list.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref()).ok();
    closure.forget();
}

fn wire_click(id: &str, f: impl Fn() + 'static) {
    let doc = document();
    let Some(el) = doc.get_element_by_id(id) else { return };
    let closure = Closure::<dyn FnMut()>::new(f);
    el.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref()).ok();
    closure.forget();
}

/// ログイン中(ローカルにトークンがある)なら管理パネルを表示して各
/// セクションを再読み込みする。未ログインなら隠す。
pub fn refresh_all() {
    if crate::auth::stored_email().is_none() {
        show("admin-panel", false);
        return;
    }
    show("admin-panel", true);
    wasm_bindgen_futures::spawn_local(refresh_requests());
    wasm_bindgen_futures::spawn_local(refresh_accounts());
    wasm_bindgen_futures::spawn_local(refresh_groups());
    wasm_bindgen_futures::spawn_local(refresh_repo_select());
}

/// 管理パネルのイベントリスナーを配線する。`start()`から一度だけ呼ぶ。
pub fn wire_admin_ui() {
    wire_requests_list();
    wire_accounts_list();
    wire_groups_list();
    wire_access_accounts_rows();

    wire_click("btn-add-account", || wasm_bindgen_futures::spawn_local(add_account()));
    wire_click("btn-refresh-requests", || wasm_bindgen_futures::spawn_local(refresh_requests()));
    wire_click("btn-refresh-accounts", || wasm_bindgen_futures::spawn_local(refresh_accounts()));
    wire_click("btn-create-group", || wasm_bindgen_futures::spawn_local(create_group()));
    wire_click("btn-refresh-groups", || wasm_bindgen_futures::spawn_local(refresh_groups()));
    wire_click("btn-load-access", || wasm_bindgen_futures::spawn_local(load_access()));
    wire_click("btn-save-access", || wasm_bindgen_futures::spawn_local(save_access()));
    wire_click("btn-add-access-account-row", add_access_account_row_from_form);
}
