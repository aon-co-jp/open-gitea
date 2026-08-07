//! RGit WASMフロントエンド(v0.1.0)。
//!
//! `/api/repos`・`/api/repos/:name/readme`(RGit本体、poem/tokio/hyper製)を
//! `fetch`で叩き、Markdown→HTML変換をブラウザ側(WASM)で行う。サーバー側は
//! JSONを返すだけで済むため、GitHubのREADME表示相当の機能をサーバー負荷
//! 最小(計算をクライアントに逃がす)で実現する狙い。
//!
//! **省メモリ最適化(このパスで実施)**: `serde`/`serde_json`はWASM
//! バイナリサイズへの影響が大きいため使わない。JSONパースも当初は
//! ブラウザ組み込みの`JSON.parse`(`js_sys::JSON`)へ委譲する案だったが、
//! それだと`Reflect::get`でフィールドを読むたびにWASM↔JS境界を1回ずつ
//! 跨ぐ。以前はこのクレート内に自作の最小JSONパーサを持っていたが、
//! [aon-co-jp/RJSON](https://github.com/aon-co-jp/RJSON)(`rust-json`
//! クレート)の`light`モジュールへ統合した(2026-07-21)——依存ゼロの
//! ブラウザ`JSON.parse`相当を1回パースし、以降はネイティブRust値
//! (`String`/`Vec`)として扱う——境界越えの呼び出し回数そのものを削減する。
//! `rust-json`は`default-features = false`(`Cargo.toml`参照)で依存し、
//! `serde_json`を要求する`full` featureはビルド対象に含まれない。
//! 加えて`opt-level="z"`+LTO+`panic=abort`+`strip=true`
//! (`Cargo.toml`参照)でバイナリを極小化している。
//!
//! **正直な開示**: v0.1.0はリポジトリ一覧+README表示のみ。GitHubにある
//! ディレクトリツリー表示・コミット履歴・シンタックスハイライト等は未実装。

mod admin;
mod auth;
mod wiki;

use rust_json::{parse_light, LightValue};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Document, Element};

fn document() -> Document {
    web_sys::window().expect("no window").document().expect("no document")
}

/// `auth::api_url`と同じ接頭辞規約(`/open-gitea`マウント、モジュールdoc参照)。
pub(crate) async fn fetch_text(url: &str) -> Result<String, JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
    let resp_value = JsFuture::from(window.fetch_with_str(&auth::api_url(url))).await?;
    let resp: web_sys::Response = resp_value.dyn_into()?;
    let text_value = JsFuture::from(resp.text()?).await?;
    Ok(text_value.as_string().unwrap_or_default())
}

pub(crate) fn markdown_to_html(src: &str) -> String {
    let parser = pulldown_cmark::Parser::new(src);
    let mut html_out = String::new();
    pulldown_cmark::html::push_html(&mut html_out, parser);
    html_out
}

fn show_status(msg: &str) {
    if let Some(el) = document().get_element_by_id("status") {
        el.set_text_content(Some(msg));
    }
}

/// `rust_json::parse_light`(RJSONの`light`モジュール)で文字列配列を
/// パースする。
pub(crate) fn parse_string_array(text: &str) -> Vec<String> {
    let Ok(value) = parse_light(text) else { return Vec::new() };
    let Some(items) = value.as_array() else { return Vec::new() };
    items.iter().filter_map(LightValue::as_str).map(str::to_string).collect()
}

/// `{"branch": "...", "content": "..."}`から2フィールドを
/// `rust_json::parse_light`で直接読む(型を作らず、必要な2値だけを
/// その場で取り出す)。
fn parse_readme_fields(text: &str) -> Option<(String, String)> {
    let value = parse_light(text).ok()?;
    let branch = value.get("branch")?.as_str()?.to_string();
    let content = value.get("content")?.as_str()?.to_string();
    Some((branch, content))
}

async fn load_readme(repo: String) {
    show_status(&format!("{repo} のREADMEを読み込み中..."));
    if !is_memory_saver_mode() {
        wasm_bindgen_futures::spawn_local(wiki::load_wiki_list(repo.clone()));
    }
    let url = format!("/api/repos/{repo}/readme");
    match fetch_text(&url).await {
        Ok(text) => match parse_readme_fields(&text) {
            Some((branch, content)) => {
                if let Some(el) = document().get_element_by_id("readme") {
                    el.set_inner_html(&markdown_to_html(&content));
                }
                show_status(&format!("{repo} (branch: {branch})"));
            }
            None => {
                if let Some(el) = document().get_element_by_id("readme") {
                    el.set_inner_html("<p><em>README.md が見つかりませんでした。</em></p>");
                }
                show_status(&format!("{repo}: README.md無し"));
            }
        },
        Err(_) => show_status(&format!("{repo}: 読み込みに失敗しました")),
    }
}

fn render_repo_list(names: &[String]) {
    let doc = document();
    let Some(list) = doc.get_element_by_id("repo-list") else { return };
    list.set_inner_html("");
    for name in names {
        let li = doc.create_element("li").unwrap();
        let a = doc.create_element("a").unwrap();
        a.set_attribute("href", "#").ok();
        a.set_attribute("data-repo", name).ok();
        a.set_class_name("repo-link");
        a.set_text_content(Some(name));
        li.append_child(&a).ok();
        list.append_child(&li).ok();
    }
}

/// `#repo-list`へのクリックをイベント委譲で拾い、`data-repo`属性から
/// リポジトリ名を取り出して`load_readme`を起動する。
fn wire_repo_list_clicks() {
    let doc = document();
    let Some(list) = doc.get_element_by_id("repo-list") else { return };

    let closure = Closure::<dyn FnMut(web_sys::Event)>::new(move |event: web_sys::Event| {
        let Some(target) = event.target() else { return };
        let Ok(el) = target.dyn_into::<Element>() else { return };
        // クリックされたのが<a>内の子要素でも、data-repo属性を持つ祖先まで遡る。
        let mut node: Option<Element> = Some(el);
        while let Some(current) = node {
            if let Some(repo) = current.get_attribute("data-repo") {
                event.prevent_default();
                wasm_bindgen_futures::spawn_local(load_readme(repo));
                return;
            }
            node = current.parent_element();
        }
    });
    list.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref()).ok();
    closure.forget(); // リスナーはページ寿命全体で有効にするため意図的にリーク(SPA1ページのみのv0.1.0では許容)
}

async fn load_repo_list() {
    show_status("リポジトリ一覧を読み込み中...");
    match fetch_text("/api/repos").await {
        Ok(text) => {
            let names = parse_string_array(&text);
            render_repo_list(&names);
            wire_repo_list_clicks();
            show_status(&format!("{}件のリポジトリ", names.len()));
        }
        Err(_) => show_status("リポジトリ一覧の読み込みに失敗しました"),
    }
}

/// `{"allowed": bool, "free_bytes": u64, "min_free_bytes": u64}`を
/// `rust_json::parse_light`で直接読む。数値は`LightValue::as_f64`経由
/// (JSONに浮動小数として保持される、`u64`最大値付近の誤差は表示用途では
/// 許容)。
async fn load_capacity() {
    let Ok((_, text)) = auth::authorized_fetch("/api/capacity", "GET", None).await else { return };
    let Some(value) = parse_light(&text).ok() else { return };
    let allowed = value.get("allowed").and_then(LightValue::as_bool).unwrap_or(false);
    let free_bytes = value.get("free_bytes").and_then(LightValue::as_f64).unwrap_or(0.0);
    let free_gb = free_bytes / 1_073_741_824.0;
    let msg = if allowed {
        format!("空き容量: {free_gb:.1}GB (作成可)")
    } else {
        format!("空き容量: {free_gb:.1}GB (残量不足のため新規作成不可)")
    };
    if let Some(el) = document().get_element_by_id("capacity-status") {
        el.set_text_content(Some(&msg));
    }
}

/// ログイン成功後に呼ばれる。ログイン状態でリポジトリ一覧・容量表示が
/// 変わりうるため再読み込みする(v0.1.0はアクセス制御ありのプライベート
/// リポジトリ一覧切り替えを想定)。
fn reload_after_login() {
    wasm_bindgen_futures::spawn_local(load_repo_list());
    wasm_bindgen_futures::spawn_local(load_capacity());
    admin::refresh_all();
}

#[wasm_bindgen(start)]
pub fn start() {
    auth::wire_auth_ui();
    admin::wire_admin_ui();
    wire_feature_mode();
    wasm_bindgen_futures::spawn_local(apply_server_default_profile_once());
    wasm_bindgen_futures::spawn_local(load_repo_list());
    wasm_bindgen_futures::spawn_local(load_capacity());
    admin::refresh_all();
}

/// 電源/機能プロファイル(2026-08-01追加、エコシステム標準方針
/// `open-raid-z/CLAUDE.md`「GUIを持つ全リポジトリに設置する」への対応)。
///
/// `open-easy-web`の`power_profile.rs`と同じ設計(省電力/省メモリ/常時
/// 電源接続は**独立したチェックボックス**として組み合わせ可能、「通常」は
/// 3つとも未チェックの状態として表現する)を、チェックボックスUIとして
/// 移植する。「省機能表示」(非必須セクションのDOM非表示)だけは別軸の
/// ボタン切替のまま(open-redmine先行実装と同じ、UIの見せ方が異なる
/// ためチェックボックス化していない)。
///
/// **正直な開示**: このアプリにはバックグラウンドポーリングループが無い
/// (git操作・README/Wiki表示はいずれも都度のリクエスト駆動)ため、
/// `open-easy-web`のようなポーリング間隔の合成ロジックは意味を持たない。
/// 実際に効果があるのは1軸のみ: **省電力または省メモリのいずれかが
/// 有効ならリポジトリ選択時のWikiページ一覧自動取得(`wiki::
/// load_wiki_list`)を止め、常時電源接続が有効ならその抑制を上書きして
/// 常に自動取得する**(`open-easy-web`の「常時電源接続がバッテリー節約
/// 軸を上書きする」という合成ルールと同じ考え方)。省電力・常時電源接続の
/// チェックボックス自体は、将来ポーリング処理を追加する際にすぐ使える
/// ようこのエコシステム共通のUI規約として先行して用意してある。
const PROFILE_POWER_SAVE_KEY: &str = "open_gitea_profile_power_save";
const PROFILE_MEMORY_SAVER_KEY: &str = "open_gitea_profile_memory_saver";
const PROFILE_ALWAYS_ON_KEY: &str = "open_gitea_profile_always_on";
const MINIMAL_UI_KEY: &str = "open_gitea_minimal_ui_v1";
/// このブラウザで一度でも`GET /api/power-profile`のサーバー既定値を
/// 適用したかどうかのマーカー(2026-08-07追加、インストーラーの電源
/// プロファイル選択機能対応)。適用済みなら二度と上書きしない
/// ——ユーザーが手動でチェックボックスを外した選択を、リロードのたびに
/// インストーラー既定値へ引き戻してしまわないようにするため。
const SERVER_DEFAULT_APPLIED_KEY: &str = "open_gitea_server_default_applied_v1";
const MINIMAL_HIDDEN_SECTION_IDS: [&str; 2] = ["wiki-panel", "intro"];

fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

fn flag_get(key: &str) -> bool {
    local_storage().and_then(|s| s.get_item(key).ok().flatten()).as_deref() == Some("1")
}

fn flag_set(key: &str, value: bool) {
    if let Some(s) = local_storage() {
        let _ = s.set_item(key, if value { "1" } else { "0" });
    }
}

fn checkbox_checked(id: &str) -> bool {
    document().get_element_by_id(id).and_then(|el| el.dyn_into::<web_sys::HtmlInputElement>().ok()).map(|el| el.checked()).unwrap_or(false)
}

fn set_checkbox_checked(id: &str, checked: bool) {
    if let Some(el) = document().get_element_by_id(id).and_then(|el| el.dyn_into::<web_sys::HtmlInputElement>().ok()) {
        el.set_checked(checked);
    }
}

/// 省電力または省メモリのいずれかが有効で、かつ常時電源接続で上書き
/// されていない状態かどうか(=Wiki自動取得を止めるべきかどうか)。
pub(crate) fn is_memory_saver_mode() -> bool {
    let always_on = flag_get(PROFILE_ALWAYS_ON_KEY);
    let power_save = flag_get(PROFILE_POWER_SAVE_KEY);
    let memory_saver = flag_get(PROFILE_MEMORY_SAVER_KEY);
    !always_on && (power_save || memory_saver)
}

fn show_section(id: &str, visible: bool) {
    let Some(el) = document().get_element_by_id(id) else { return };
    if visible {
        el.class_list().remove_1("hidden").ok();
    } else {
        el.class_list().add_1("hidden").ok();
    }
}

fn feature_profile_labels() -> Vec<&'static str> {
    let mut labels = Vec::new();
    if flag_get(PROFILE_POWER_SAVE_KEY) {
        labels.push("省電力");
    }
    if flag_get(PROFILE_MEMORY_SAVER_KEY) {
        labels.push("省メモリ");
    }
    if flag_get(PROFILE_ALWAYS_ON_KEY) {
        labels.push("常時電源接続");
    }
    labels
}

fn apply_feature_mode() {
    let minimal = flag_get(MINIMAL_UI_KEY);
    for id in MINIMAL_HIDDEN_SECTION_IDS {
        show_section(id, !minimal);
    }
    set_checkbox_checked("profile-power-save", flag_get(PROFILE_POWER_SAVE_KEY));
    set_checkbox_checked("profile-memory-saver", flag_get(PROFILE_MEMORY_SAVER_KEY));
    set_checkbox_checked("profile-always-on", flag_get(PROFILE_ALWAYS_ON_KEY));

    let mut labels = feature_profile_labels();
    if minimal {
        labels.push("省機能表示");
    }
    let text = if labels.is_empty() { "通常モード (normal mode)".to_string() } else { format!("有効: {} (active)", labels.join(" + ")) };
    if let Some(el) = document().get_element_by_id("power-profile-status") {
        el.set_text_content(Some(&text));
    }
}

/// `install.sh`/`install.ps1`が書き出した`RGIT_POWER_PROFILE`環境変数を
/// `GET /api/power-profile`経由で読み、**このブラウザで初めて開いた
/// ときだけ**チェックボックスの初期状態として反映する(2026-08-07追加、
/// `open-raid-z/CLAUDE.md`のインストーラー電源プロファイル選択方針への
/// 対応)。2回目以降のロードでは`SERVER_DEFAULT_APPLIED_KEY`が既に
/// `"1"`になっているため、ユーザー自身のチェックボックス操作を
/// インストーラー既定値で上書きすることはない。
async fn apply_server_default_profile_once() {
    if flag_get(SERVER_DEFAULT_APPLIED_KEY) {
        return;
    }
    flag_set(SERVER_DEFAULT_APPLIED_KEY, true);
    let Ok(text) = fetch_text("/api/power-profile").await else { return };
    let Ok(value) = parse_light(&text) else { return };
    if value.get("power_save").and_then(LightValue::as_bool).unwrap_or(false) {
        flag_set(PROFILE_POWER_SAVE_KEY, true);
    }
    if value.get("memory_saver").and_then(LightValue::as_bool).unwrap_or(false) {
        flag_set(PROFILE_MEMORY_SAVER_KEY, true);
    }
    if value.get("always_on").and_then(LightValue::as_bool).unwrap_or(false) {
        flag_set(PROFILE_ALWAYS_ON_KEY, true);
    }
    apply_feature_mode();
}

fn wire_feature_mode() {
    auth::wire_click("profile-power-save", || {
        flag_set(PROFILE_POWER_SAVE_KEY, checkbox_checked("profile-power-save"));
        apply_feature_mode();
    });
    auth::wire_click("profile-memory-saver", || {
        flag_set(PROFILE_MEMORY_SAVER_KEY, checkbox_checked("profile-memory-saver"));
        apply_feature_mode();
    });
    auth::wire_click("profile-always-on", || {
        flag_set(PROFILE_ALWAYS_ON_KEY, checkbox_checked("profile-always-on"));
        apply_feature_mode();
    });
    auth::wire_click("power-profile-minimal-btn", || {
        flag_set(MINIMAL_UI_KEY, !flag_get(MINIMAL_UI_KEY));
        apply_feature_mode();
    });
    auth::wire_click("power-profile-restore-btn", || {
        flag_set(PROFILE_POWER_SAVE_KEY, false);
        flag_set(PROFILE_MEMORY_SAVER_KEY, false);
        flag_set(PROFILE_ALWAYS_ON_KEY, false);
        flag_set(MINIMAL_UI_KEY, false);
        apply_feature_mode();
    });
    apply_feature_mode();
}
