//! Releases(実Gitea〈about.gitea.com〉との機能差分解消、2026-07-27追加)。
//!
//! 実Giteaのように別テーブルでリリースノート・添付ファイルを管理する
//!独立エンティティは持たない——**gitタグ自体をリリース一覧の実体として
//! 扱う**軽量な実装。タグの注釈メッセージ(annotated tag)があれば
//! リリースノートとして流用し、無ければ空文字列。添付ファイル
//! (バイナリ配布物のアップロード)は今回のスコープ外(正直な開示)。
//!
//! `git http-backend`橋渡し・README表示と同じ「gitコマンドに任せる」
//! 方針を踏襲し、`git for-each-ref`/`git tag`をサブプロセス実行する。

use serde::Serialize;
use std::path::Path;
use tokio::process::Command;

#[derive(Debug, Clone, Serialize)]
pub struct ReleaseTag {
    pub name: String,
    pub commit_sha: String,
    /// annotated tagのメッセージ(無ければ空文字列。lightweight tagには
    /// メッセージが存在しないため、実Giteaのような「リリースノート必須」
    /// ではなく、無くても一覧に出す)。
    pub message: String,
    /// タグが指すコミットの作成日時(ISO 8601、`git log -1 --format=%aI`)。
    pub created_at: String,
}

/// 全タグを一覧する(新しい順)。タグが1つも無いリポジトリでは空配列を返す
/// (エラーにしない、`get_wiki_pages`と同じ「まだ何も無い」の扱い方)。
pub async fn list(repo_path: &Path) -> Vec<ReleaseTag> {
    let tag_names_out = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .arg("tag")
        .arg("--sort=-creatordate")
        .output()
        .await;
    let Ok(tag_names_out) = tag_names_out else {
        return Vec::new();
    };
    if !tag_names_out.status.success() {
        return Vec::new();
    }
    let tag_names: Vec<String> = String::from_utf8_lossy(&tag_names_out.stdout)
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    let mut releases = Vec::with_capacity(tag_names.len());
    for name in tag_names {
        if let Some(release) = describe_tag(repo_path, &name).await {
            releases.push(release);
        }
    }
    releases
}

async fn describe_tag(repo_path: &Path, name: &str) -> Option<ReleaseTag> {
    let sha_out = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .arg("rev-list")
        .arg("-n")
        .arg("1")
        .arg(name)
        .output()
        .await
        .ok()?;
    if !sha_out.status.success() {
        return None;
    }
    let commit_sha = String::from_utf8_lossy(&sha_out.stdout).trim().to_string();

    // annotated tag(タグ自身が独立したtagオブジェクトを持つ)かどうかを
    // `git cat-file -t`で判定する。lightweight tagは単なるコミットへの
    // 参照(`refs/tags/<name>`がcommitオブジェクトを直接指す)なので、
    // `for-each-ref --format=%(contents)`をそのまま使うと**タグ先の
    // コミットメッセージ**が誤って「リリースノート」として拾われてしまう
    // (実際に単体テストでこの誤りを検出した)。type==tagの場合のみ
    // `%(contents)`(タグオブジェクト自身の注釈メッセージ)を採用する。
    let type_out = Command::new("git").arg("-C").arg(repo_path).arg("cat-file").arg("-t").arg(format!("refs/tags/{name}")).output().await.ok()?;
    let is_annotated = type_out.status.success() && String::from_utf8_lossy(&type_out.stdout).trim() == "tag";

    let message = if is_annotated {
        let msg_out = Command::new("git")
            .arg("-C")
            .arg(repo_path)
            .arg("for-each-ref")
            .arg(format!("refs/tags/{name}"))
            .arg("--format=%(contents)")
            .output()
            .await
            .ok()?;
        if msg_out.status.success() {
            String::from_utf8_lossy(&msg_out.stdout).trim().to_string()
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    let date_out = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .arg("log")
        .arg("-1")
        .arg("--format=%aI")
        .arg(&commit_sha)
        .output()
        .await
        .ok()?;
    let created_at = if date_out.status.success() {
        String::from_utf8_lossy(&date_out.stdout).trim().to_string()
    } else {
        String::new()
    };

    Some(ReleaseTag { name: name.to_string(), commit_sha, message, created_at })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command as StdCommand;

    fn init_repo_with_tags(dir: &Path) {
        let run = |args: &[&str]| {
            let status = StdCommand::new("git").arg("-C").arg(dir).args(args).status().unwrap();
            assert!(status.success(), "git {args:?} failed");
        };
        run(&["init", "-q", "--initial-branch=main"]);
        run(&["config", "user.email", "test@example.com"]);
        run(&["config", "user.name", "Test"]);
        std::fs::write(dir.join("a.txt"), "hello").unwrap();
        run(&["add", "a.txt"]);
        run(&["commit", "-q", "-m", "first commit"]);
        run(&["tag", "-a", "v0.1.0", "-m", "First release\n\nInitial version."]);
        std::fs::write(dir.join("a.txt"), "hello again").unwrap();
        run(&["add", "a.txt"]);
        run(&["commit", "-q", "-m", "second commit"]);
        run(&["tag", "v0.2.0-lightweight"]);
    }

    #[tokio::test]
    async fn list_returns_empty_for_repo_without_tags() {
        let dir = tempfile::tempdir().unwrap();
        let status = std::process::Command::new("git").arg("-C").arg(dir.path()).args(["init", "-q"]).status().unwrap();
        assert!(status.success());
        let releases = list(dir.path()).await;
        assert!(releases.is_empty());
    }

    #[tokio::test]
    async fn list_returns_annotated_and_lightweight_tags_with_expected_fields() {
        let dir = tempfile::tempdir().unwrap();
        init_repo_with_tags(dir.path());

        let releases = list(dir.path()).await;
        assert_eq!(releases.len(), 2);

        let annotated = releases.iter().find(|r| r.name == "v0.1.0").expect("v0.1.0 must be present");
        assert!(annotated.message.contains("First release"));
        assert!(!annotated.commit_sha.is_empty());
        assert!(!annotated.created_at.is_empty());

        let lightweight = releases.iter().find(|r| r.name == "v0.2.0-lightweight").expect("lightweight tag must be present");
        assert!(lightweight.message.is_empty(), "lightweight tags have no message");
        assert!(!lightweight.commit_sha.is_empty());
    }
}
