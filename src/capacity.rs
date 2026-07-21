//! 新規リポジトリ作成を許可してよいかの自動判断。
//!
//! **正直な開示**: 「AIが自動で考慮する」という要件を、実際のディスク
//! 空き容量を計測するルールベースの自動判定として実装している
//! (機械学習モデルではない——`fs2::available_space`で実測した値を
//! 閾値と比較するだけの、単純だが実際にディスクの状態を見て動く判断)。
//! 管理者自身の作成リクエストにも同じ判定を適用する——ユーザー要件
//! 「許可するかどうかは管理者でも他人やチームなどに対しても、自動で
//! 考慮する」に対応するため、呼び出し元で権限チェックとは別枠で必ず通す。

use std::path::Path;

/// 新規リポジトリ作成を許可する最低空き容量(MB)。既定1024MB(1GB)。
/// `RGIT_MIN_FREE_DISK_MB`で変更可能。
fn min_free_bytes() -> u64 {
    std::env::var("RGIT_MIN_FREE_DISK_MB").ok().and_then(|v| v.parse::<u64>().ok()).unwrap_or(1024) * 1024 * 1024
}

#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct CapacityDecision {
    pub allowed: bool,
    pub free_bytes: u64,
    pub min_free_bytes: u64,
}

/// `repos_root`が乗っているボリュームの実際の空き容量を計測し、
/// 新規リポジトリ作成を許可してよいかを判断する。空き容量の計測自体に
/// 失敗した場合(パス不存在等)は、安全側に倒して不許可とする。
pub fn decide(repos_root: &Path) -> CapacityDecision {
    let min_free = min_free_bytes();
    match fs2::available_space(repos_root) {
        Ok(free) => CapacityDecision { allowed: free > min_free, free_bytes: free, min_free_bytes: min_free },
        Err(_) => CapacityDecision { allowed: false, free_bytes: 0, min_free_bytes: min_free },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decide_reflects_actual_free_space_on_current_volume() {
        // 実在するパス(このテストプロセスのカレントディレクトリ)で
        // 実際にディスク空き容量を計測できることを確認する
        // (モックではなく、本当にOSからバイト数を取得できるかの検証)。
        let decision = decide(Path::new("."));
        assert!(decision.free_bytes > 0, "expected to measure nonzero free space, got {decision:?}");
    }

    #[test]
    fn nonexistent_path_denies_safely() {
        let decision = decide(Path::new("Z:\\this-path-should-not-exist-anywhere-12345"));
        assert!(!decision.allowed);
    }
}
