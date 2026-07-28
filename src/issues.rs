//! Gitea/GitBucketが持つ機能のうち、Issue(課題管理)機能。Pull Request・
//! Webhookは今回のスコープ外(未着手のまま正直に記録)。
//!
//! 実体はWikiと同様、対象リポジトリのbareリポジトリディレクトリ直下に
//! JSONファイル(`.open-gitea-issues.json`)を1本置くだけ(DB非依存という
//! 既存方針を踏襲)。アクセス制御はWikiと同じく本体リポジトリの
//! [`crate::access::AccessConfig`]をそのまま流用する(Issue専用の権限
//! 系統は持たない)——閲覧は`Need::View`、作成・コメント・ステータス
//! 変更は`Need::Push`(このリポジトリへの書き込み権を持つ人だけが
//! Issueも編集できる、という素直な対応付け)。
//!
//! **2026-07-27追記(実Gitea〈about.gitea.com〉との機能差分の一部解消)**:
//! `labels`(自由記述タグ、色・説明文は持たない簡易版)・`assignee`
//! (担当者メールアドレス1名のみ、複数担当者やチーム割当は非対応)・
//! `milestone`(自由記述の名前のみ、期日・進捗率を持つ独立エンティティ
//! ではない)を追加した。実Giteaのような色付きLabel管理画面・
//! マイルストーン一覧/進捗バーは持たない——あくまで既存のIssue
//! JSONファイルへのフィールド追加による軽量な近似。

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueStatus {
    Open,
    Closed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comment {
    pub author: String,
    pub body: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    pub id: u64,
    pub title: String,
    pub body: String,
    pub status: IssueStatus,
    pub author: String,
    pub created_at: String,
    #[serde(default)]
    pub comments: Vec<Comment>,
    /// 自由記述タグ(実Giteaの色付きLabelとは異なり、色・説明文の管理は
    /// 持たない簡易版)。`#[serde(default)]`で既存の保存データとの
    /// 後方互換を維持する(2026-07-27追記、実Gitea機能差分の一部解消)。
    #[serde(default)]
    pub labels: Vec<String>,
    /// 担当者のメールアドレス(未割当なら`None`)。`open-redmine`の
    /// `assignee`と同じ設計方針(登録済みアカウントかどうかの検証は
    /// 呼び出し側`main.rs`が行う——このモジュール自体は文字列を
    /// そのまま保持する汎用フィールド)。
    #[serde(default)]
    pub assignee: Option<String>,
    /// マイルストーン名(自由記述の名前のみ、実Giteaのような期日・進捗率・
    /// 説明文を持つ独立エンティティではない簡易版——正直な開示として
    /// モジュールdocに明記)。
    #[serde(default)]
    pub milestone: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct IssueStore {
    next_id: u64,
    issues: Vec<Issue>,
}

fn issues_path(repo_path: &Path) -> PathBuf {
    repo_path.join(".open-gitea-issues.json")
}

async fn load(repo_path: &Path) -> IssueStore {
    match tokio::fs::read(issues_path(repo_path)).await {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
        Err(_) => IssueStore::default(),
    }
}

async fn save(repo_path: &Path, store: &IssueStore) -> std::io::Result<()> {
    let bytes = serde_json::to_vec_pretty(store).expect("IssueStore serialization is infallible");
    tokio::fs::write(issues_path(repo_path), bytes).await
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

pub async fn list(repo_path: &Path) -> Vec<Issue> {
    load(repo_path).await.issues
}

pub async fn get(repo_path: &Path, id: u64) -> Option<Issue> {
    load(repo_path).await.issues.into_iter().find(|i| i.id == id)
}

pub async fn create(repo_path: &Path, title: String, body: String, author: String) -> std::io::Result<Issue> {
    let mut store = load(repo_path).await;
    let id = store.next_id;
    store.next_id += 1;
    let issue = Issue {
        id,
        title,
        body,
        status: IssueStatus::Open,
        author,
        created_at: now(),
        comments: Vec::new(),
        labels: Vec::new(),
        assignee: None,
        milestone: None,
    };
    store.issues.push(issue.clone());
    save(repo_path, &store).await?;
    Ok(issue)
}

#[derive(Debug, Clone, Copy)]
pub enum UpdateIssueError {
    NotFound,
}

/// labels/assignee/milestoneをまとめて更新する(`None`は「変更しない」、
/// 明示的に空へ戻したい場合は`labels: Some(vec![])`/`assignee:
/// Some(None)`のように二重`Option`で渡す——`main.rs`側のリクエスト
/// body(`serde_json::Value`)から個別に組み立てる想定)。
pub async fn update_metadata(
    repo_path: &Path,
    id: u64,
    labels: Option<Vec<String>>,
    assignee: Option<Option<String>>,
    milestone: Option<Option<String>>,
) -> Result<Issue, UpdateIssueError> {
    let mut store = load(repo_path).await;
    let Some(issue) = store.issues.iter_mut().find(|i| i.id == id) else {
        return Err(UpdateIssueError::NotFound);
    };
    if let Some(labels) = labels {
        issue.labels = labels;
    }
    if let Some(assignee) = assignee {
        issue.assignee = assignee;
    }
    if let Some(milestone) = milestone {
        issue.milestone = milestone;
    }
    let updated = issue.clone();
    let _ = save(repo_path, &store).await;
    Ok(updated)
}

#[derive(Debug, Clone, Copy)]
pub enum SetStatusError {
    NotFound,
}

pub async fn set_status(repo_path: &Path, id: u64, status: IssueStatus) -> Result<(), SetStatusError> {
    let mut store = load(repo_path).await;
    let Some(issue) = store.issues.iter_mut().find(|i| i.id == id) else {
        return Err(SetStatusError::NotFound);
    };
    issue.status = status;
    let _ = save(repo_path, &store).await;
    Ok(())
}

pub async fn add_comment(repo_path: &Path, id: u64, author: String, body: String) -> Option<Comment> {
    let mut store = load(repo_path).await;
    let issue = store.issues.iter_mut().find(|i| i.id == id)?;
    let comment = Comment { author, body, created_at: now() };
    issue.comments.push(comment.clone());
    let _ = save(repo_path, &store).await;
    Some(comment)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn create_list_and_get_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let repo_path = dir.path();
        let issue = create(repo_path, "Bug".to_string(), "It's broken".to_string(), "alice@example.com".to_string()).await.unwrap();
        assert_eq!(issue.id, 0);
        assert_eq!(issue.status, IssueStatus::Open);

        let listed = list(repo_path).await;
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].title, "Bug");

        let fetched = get(repo_path, 0).await.unwrap();
        assert_eq!(fetched.body, "It's broken");
    }

    #[tokio::test]
    async fn ids_increment_across_multiple_issues() {
        let dir = tempfile::tempdir().unwrap();
        let repo_path = dir.path();
        create(repo_path, "A".to_string(), "".to_string(), "a@x.com".to_string()).await.unwrap();
        let second = create(repo_path, "B".to_string(), "".to_string(), "a@x.com".to_string()).await.unwrap();
        assert_eq!(second.id, 1);
    }

    #[tokio::test]
    async fn set_status_updates_and_persists() {
        let dir = tempfile::tempdir().unwrap();
        let repo_path = dir.path();
        create(repo_path, "A".to_string(), "".to_string(), "a@x.com".to_string()).await.unwrap();
        set_status(repo_path, 0, IssueStatus::Closed).await.unwrap();
        let fetched = get(repo_path, 0).await.unwrap();
        assert!(matches!(fetched.status, IssueStatus::Closed));
    }

    #[tokio::test]
    async fn set_status_on_missing_issue_errors() {
        let dir = tempfile::tempdir().unwrap();
        let result = set_status(dir.path(), 999, IssueStatus::Closed).await;
        assert!(matches!(result, Err(SetStatusError::NotFound)));
    }

    #[tokio::test]
    async fn add_comment_appends_and_persists() {
        let dir = tempfile::tempdir().unwrap();
        let repo_path = dir.path();
        create(repo_path, "A".to_string(), "".to_string(), "a@x.com".to_string()).await.unwrap();
        add_comment(repo_path, 0, "bob@example.com".to_string(), "Looking into it".to_string()).await.unwrap();
        let fetched = get(repo_path, 0).await.unwrap();
        assert_eq!(fetched.comments.len(), 1);
        assert_eq!(fetched.comments[0].author, "bob@example.com");
    }

    #[tokio::test]
    async fn add_comment_on_missing_issue_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let result = add_comment(dir.path(), 999, "bob@example.com".to_string(), "x".to_string()).await;
        assert!(result.is_none());
    }

    /// 2026-07-27追記: labels/assignee/milestoneが更新・永続化され、
    /// `None`を渡したフィールドは変更されない(部分更新)ことを確認する。
    #[tokio::test]
    async fn update_metadata_sets_labels_assignee_and_milestone_and_persists() {
        let dir = tempfile::tempdir().unwrap();
        let repo_path = dir.path();
        create(repo_path, "A".to_string(), "".to_string(), "a@x.com".to_string()).await.unwrap();

        let updated = update_metadata(
            repo_path,
            0,
            Some(vec!["bug".to_string(), "priority-high".to_string()]),
            Some(Some("dev@example.com".to_string())),
            Some(Some("v1.0".to_string())),
        )
        .await
        .unwrap();
        assert_eq!(updated.labels, vec!["bug", "priority-high"]);
        assert_eq!(updated.assignee.as_deref(), Some("dev@example.com"));
        assert_eq!(updated.milestone.as_deref(), Some("v1.0"));

        let fetched = get(repo_path, 0).await.unwrap();
        assert_eq!(fetched.labels, vec!["bug", "priority-high"]);
        assert_eq!(fetched.assignee.as_deref(), Some("dev@example.com"));
        assert_eq!(fetched.milestone.as_deref(), Some("v1.0"));

        // labelsだけ変更し、assignee/milestoneは`None`渡しで維持されることを確認。
        let updated2 = update_metadata(repo_path, 0, Some(vec!["bug".to_string()]), None, None).await.unwrap();
        assert_eq!(updated2.labels, vec!["bug"]);
        assert_eq!(updated2.assignee.as_deref(), Some("dev@example.com"));
        assert_eq!(updated2.milestone.as_deref(), Some("v1.0"));
    }

    #[tokio::test]
    async fn update_metadata_on_missing_issue_errors() {
        let dir = tempfile::tempdir().unwrap();
        let result = update_metadata(dir.path(), 999, Some(vec![]), None, None).await;
        assert!(matches!(result, Err(UpdateIssueError::NotFound)));
    }

    /// 既存データとの後方互換: `labels`/`assignee`/`milestone`フィールドを
    /// 持たない古い形式のJSONも`#[serde(default)]`で正しくロードできる
    /// ことを確認する(実データの破損防止)。
    #[tokio::test]
    async fn loads_pre_existing_issue_json_without_new_fields() {
        let dir = tempfile::tempdir().unwrap();
        let repo_path = dir.path();
        let old_json = serde_json::json!({
            "next_id": 1,
            "issues": [{
                "id": 0,
                "title": "Old issue",
                "body": "",
                "status": "open",
                "author": "a@x.com",
                "created_at": "2026-01-01T00:00:00Z",
                "comments": []
            }]
        });
        tokio::fs::write(issues_path(repo_path), serde_json::to_vec(&old_json).unwrap()).await.unwrap();

        let fetched = get(repo_path, 0).await.unwrap();
        assert!(fetched.labels.is_empty());
        assert!(fetched.assignee.is_none());
        assert!(fetched.milestone.is_none());
    }
}
