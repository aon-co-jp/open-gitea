//! 登録済みメールアドレスの管理、および「誰でも申請できる」アクセス
//! リクエストの受付。
//!
//! アカウント登録には2つの経路がある:
//!
//! 1. **管理者が直接登録**(`POST /api/accounts`、管理者のみ)。
//! 2. **誰でも申請できる自己サービス方式**(`POST /api/accounts/request`、
//!    認証不要): メールアドレス・対象リポジトリ(任意)・メッセージ(任意)
//!    を送ると[`AccessRequest`]として保留リストに入り、管理者へ通知
//!    メールが飛ぶ。管理者は`GET /api/accounts/requests`で一覧を確認し、
//!    `POST /api/accounts/requests/:id/decide`で**閲覧/ダウンロード/
//!    push を個別に選んで**許可・不許可を判断する(却下も可能)。
//!    承認するとメールアドレスが[`AccountStore::emails`]へ登録され、
//!    (対象リポジトリを指定していれば)そのリポジトリの
//!    `access::AccessConfig::accounts`にも許可が書き込まれる。

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessRequest {
    pub id: String,
    pub email: String,
    pub repo: Option<String>,
    pub message: Option<String>,
    /// `true`なら「既存リポジトリへのアクセス許可」ではなく
    /// 「新規リポジトリ作成の許可」を求める申請(`repo`にはその場合、
    /// 作成したいリポジトリ名が入る)。
    #[serde(default)]
    pub is_create_repo_request: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AccountStore {
    pub emails: HashSet<String>,
    /// 新規リポジトリ作成が許可されているアカウント
    /// (`emails`に含まれることが前提、さらにこの集合にも入っている
    /// 必要がある——「ログインはできるが作成はできない」が既定)。
    pub can_create_repos: HashSet<String>,
    pub pending_requests: Vec<AccessRequest>,
}

fn accounts_path(repos_root: &Path) -> PathBuf {
    repos_root.join(".rgit-accounts.json")
}

pub async fn load(repos_root: &Path) -> AccountStore {
    match tokio::fs::read(accounts_path(repos_root)).await {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
        Err(_) => AccountStore::default(),
    }
}

pub async fn save(repos_root: &Path, store: &AccountStore) -> std::io::Result<()> {
    let bytes = serde_json::to_vec_pretty(store).expect("AccountStore serialization is infallible");
    tokio::fs::write(accounts_path(repos_root), bytes).await
}

pub fn generate_request_id() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let bytes: [u8; 12] = rng.gen();
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
