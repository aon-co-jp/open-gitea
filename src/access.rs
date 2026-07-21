//! リポジトリごとのアクセス制御(公開範囲・グループ・閲覧/ダウンロード許可)。
//!
//! 管理者(ログイン済みセッション)が、リポジトリ単位で以下を設定できる:
//!
//! - **モード**: `private`(管理者のみ)・`public`(誰でも)・
//!   `group`(名前付きグループのトークン保持者のみ)
//! - **閲覧許可**(`allow_view`)と**ダウンロード許可**(`allow_download`)を
//!   個別にON/OFF——「一覧・READMEは見せるがダウンロードはさせない」
//!   といった制御が可能。
//!
//! グループはリポジトリ横断で管理者が名前を付けて作成する(例:
//! `team-a`・`class-2026`)。RGit自体はアカウント登録機能を持たない
//! ([`crate::auth`]は固定管理者1名のみ)ため、グループの「メンバー」は
//! 個別アカウントではなく、**グループ発行時に生成する共有トークン**を
//! 知っている人全員として扱う(招待リンク方式)。

use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Private,
    Public,
    Group,
}

impl Default for Mode {
    fn default() -> Self {
        Mode::Private
    }
}

/// 登録済みアカウント1件に対する、このリポジトリでの許可。
/// `mode`/`group`とは独立——「全体はprivateのまま、特定の登録メール
/// だけに個別に閲覧・ダウンロードを許可する」という第3の粒度。
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AccountPermission {
    pub allow_view: bool,
    pub allow_download: bool,
    /// `git push`(git-receive-pack)を許可するか。
    pub allow_push: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AccessConfig {
    pub mode: Mode,
    pub group: Option<String>,
    pub allow_view: bool,
    pub allow_download: bool,
    /// `mode`が`public`/`group`のとき、cloneだけでなく`git push`も
    /// 誰でも(またはグループ全員)許可するか。既定`false`
    /// (公開リポジトリでも書き込みは既定拒否、が安全側)。
    pub allow_push: bool,
    /// 登録メールアドレス → そのアカウント個別の許可。
    pub accounts: HashMap<String, AccountPermission>,
}

impl Default for AccessConfig {
    fn default() -> Self {
        Self { mode: Mode::Private, group: None, allow_view: false, allow_download: false, allow_push: false, accounts: HashMap::new() }
    }
}

fn access_config_path(repo_path: &Path) -> PathBuf {
    repo_path.join(".rgit-access.json")
}

/// リポジトリのアクセス設定を読む。ファイルが無い/壊れている場合は
/// 既定値(`private`、閲覧もダウンロードも不可)にフォールバックする——
/// 「設定し忘れたら非公開のまま」という安全側デフォルト。
pub async fn load(repo_path: &Path) -> AccessConfig {
    match tokio::fs::read(access_config_path(repo_path)).await {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
        Err(_) => AccessConfig::default(),
    }
}

pub async fn save(repo_path: &Path, config: &AccessConfig) -> std::io::Result<()> {
    let bytes = serde_json::to_vec_pretty(config).expect("AccessConfig serialization is infallible");
    tokio::fs::write(access_config_path(repo_path), bytes).await
}

/// 何をしようとしているか。閲覧(一覧・README・ファイルツリー表示)と
/// ダウンロード(個別ファイル・ZIP取得)を区別する。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Need {
    View,
    Download,
    Push,
}

/// `config`と、(グループモードの場合の)提示されたトークン・
/// (ログイン済みなら)アカウントのメールアドレスから、`need`の操作が
/// 許可されるかを判定する。管理者ログイン済みかどうかは呼び出し側
/// (`crate::check_access`)が別途見る——この関数はあくまで
/// 「一般公開ルール(public/group)またはアカウント個別許可として
/// 許可されるか」だけを見る。
pub fn is_allowed(config: &AccessConfig, need: Need, groups: &GroupStore, presented_token: Option<&str>, account_email: Option<&str>) -> bool {
    // アカウント個別の許可は`mode`(private/public/group)とは独立に効く
    // ——全体はprivateのまま特定アカウントにだけ許可する、という運用を
    // 可能にするため。
    if let Some(email) = account_email {
        if let Some(perm) = config.accounts.get(email) {
            let flag = match need {
                Need::View => perm.allow_view,
                Need::Download => perm.allow_download,
                Need::Push => perm.allow_push,
            };
            if flag {
                return true;
            }
        }
    }

    let flag = match need {
        Need::View => config.allow_view,
        Need::Download => config.allow_download,
        Need::Push => config.allow_push,
    };
    if !flag {
        return false;
    }
    match config.mode {
        Mode::Private => false,
        Mode::Public => true,
        Mode::Group => {
            let Some(group_name) = &config.group else { return false };
            let Some(token) = presented_token else { return false };
            groups.groups.get(group_name).map(|g| g.token == token).unwrap_or(false)
        }
    }
}

// --- グループ管理(リポジトリ横断、`repos_root`直下に1ファイルで管理) ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupInfo {
    pub token: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GroupStore {
    pub groups: HashMap<String, GroupInfo>,
}

fn groups_path(repos_root: &Path) -> PathBuf {
    repos_root.join(".rgit-groups.json")
}

pub async fn load_groups(repos_root: &Path) -> GroupStore {
    match tokio::fs::read(groups_path(repos_root)).await {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
        Err(_) => GroupStore::default(),
    }
}

pub async fn save_groups(repos_root: &Path, store: &GroupStore) -> std::io::Result<()> {
    let bytes = serde_json::to_vec_pretty(store).expect("GroupStore serialization is infallible");
    tokio::fs::write(groups_path(repos_root), bytes).await
}

pub fn generate_token() -> String {
    let mut rng = rand::thread_rng();
    let bytes: [u8; 16] = rng.gen();
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(mode: Mode, group: Option<&str>, allow_view: bool, allow_download: bool, allow_push: bool) -> AccessConfig {
        AccessConfig { mode, group: group.map(str::to_string), allow_view, allow_download, allow_push, accounts: HashMap::new() }
    }

    #[test]
    fn private_repo_denies_regardless_of_flags() {
        let config = cfg(Mode::Private, None, true, true, true);
        let groups = GroupStore::default();
        assert!(!is_allowed(&config, Need::View, &groups, None, None));
        assert!(!is_allowed(&config, Need::Download, &groups, None, None));
        assert!(!is_allowed(&config, Need::Push, &groups, None, None));
    }

    #[test]
    fn public_repo_respects_view_download_and_push_flags_independently() {
        let view_only = cfg(Mode::Public, None, true, false, false);
        let groups = GroupStore::default();
        assert!(is_allowed(&view_only, Need::View, &groups, None, None));
        assert!(!is_allowed(&view_only, Need::Download, &groups, None, None));
        assert!(!is_allowed(&view_only, Need::Push, &groups, None, None));
    }

    #[test]
    fn group_repo_requires_matching_token() {
        let mut groups = GroupStore::default();
        groups.groups.insert("team-a".to_string(), GroupInfo { token: "secret123".to_string() });
        let config = cfg(Mode::Group, Some("team-a"), true, true, true);

        assert!(!is_allowed(&config, Need::View, &groups, None, None));
        assert!(!is_allowed(&config, Need::View, &groups, Some("wrong-token"), None));
        assert!(is_allowed(&config, Need::View, &groups, Some("secret123"), None));
        assert!(is_allowed(&config, Need::Download, &groups, Some("secret123"), None));
        assert!(is_allowed(&config, Need::Push, &groups, Some("secret123"), None));
    }

    #[test]
    fn group_repo_with_unknown_group_name_denies() {
        let groups = GroupStore::default();
        let config = cfg(Mode::Group, Some("no-such-group"), true, true, true);
        assert!(!is_allowed(&config, Need::View, &groups, Some("anything"), None));
    }

    #[test]
    fn default_config_is_private_and_denies_everything() {
        let config = AccessConfig::default();
        let groups = GroupStore::default();
        assert!(!is_allowed(&config, Need::View, &groups, None, None));
        assert!(!is_allowed(&config, Need::Download, &groups, None, None));
        assert!(!is_allowed(&config, Need::Push, &groups, None, None));
    }

    #[test]
    fn account_specific_grant_works_even_when_repo_is_otherwise_private() {
        let mut config = cfg(Mode::Private, None, false, false, false);
        config.accounts.insert(
            "member@example.com".to_string(),
            AccountPermission { allow_view: true, allow_download: false, allow_push: false },
        );
        let groups = GroupStore::default();
        assert!(is_allowed(&config, Need::View, &groups, None, Some("member@example.com")));
        assert!(!is_allowed(&config, Need::Download, &groups, None, Some("member@example.com")));
        // 別アカウントには権限が無い。
        assert!(!is_allowed(&config, Need::View, &groups, None, Some("someone-else@example.com")));
    }

    #[test]
    fn account_grant_can_include_push() {
        let mut config = cfg(Mode::Private, None, false, false, false);
        config.accounts.insert(
            "collaborator@example.com".to_string(),
            AccountPermission { allow_view: true, allow_download: true, allow_push: true },
        );
        let groups = GroupStore::default();
        assert!(is_allowed(&config, Need::Push, &groups, None, Some("collaborator@example.com")));
    }
}
