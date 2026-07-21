//! SMTP経由でのOTPメール送信。[`open-easy-web`]の`server/src/mail.rs`と
//! 同じ設計(`lettre`の同期SMTPクライアントを`spawn_blocking`でオフロード)。
//!
//! [`open-easy-web`]: https://github.com/aon-co-jp/open-easy-web

use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};

#[derive(Debug, Clone)]
pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub from: String,
}

impl SmtpConfig {
    pub fn from_env() -> Option<Self> {
        Some(Self {
            host: std::env::var("RGIT_SMTP_HOST").ok()?,
            port: std::env::var("RGIT_SMTP_PORT").ok().and_then(|v| v.parse().ok()).unwrap_or(587),
            username: std::env::var("RGIT_SMTP_USERNAME").ok()?,
            password: std::env::var("RGIT_SMTP_PASSWORD").ok()?,
            from: std::env::var("RGIT_SMTP_FROM").ok()?,
        })
    }
}

#[derive(Debug)]
pub enum MailError {
    Build(String),
    Send(String),
}

impl std::fmt::Display for MailError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MailError::Build(e) => write!(f, "メール作成失敗: {e}"),
            MailError::Send(e) => write!(f, "メール送信失敗: {e}"),
        }
    }
}

fn build_and_send(config: &SmtpConfig, to: &str, subject: &str, body: String) -> Result<(), MailError> {
    let email = Message::builder()
        .from(config.from.parse().map_err(|e| MailError::Build(format!("{e}")))?)
        .to(to.parse().map_err(|e| MailError::Build(format!("{e}")))?)
        .subject(subject)
        .header(ContentType::TEXT_PLAIN)
        .body(body)
        .map_err(|e| MailError::Build(format!("{e}")))?;

    let creds = Credentials::new(config.username.clone(), config.password.clone());
    let mailer =
        SmtpTransport::starttls_relay(&config.host).map_err(|e| MailError::Send(format!("{e}")))?.port(config.port).credentials(creds).build();

    mailer.send(&email).map_err(|e| MailError::Send(format!("{e}")))?;
    Ok(())
}

pub async fn send_otp(config: SmtpConfig, to: String, code: String) -> Result<(), MailError> {
    let body = format!(
        "RGit ログイン用ワンタイムパスワード\n\n\
         コード: {code}\n\
         このコードは5分間有効です。\n\n\
         心当たりがない場合はこのメールを無視してください。"
    );
    tokio::task::spawn_blocking(move || build_and_send(&config, &to, "RGit ログインコード", body))
        .await
        .map_err(|e| MailError::Send(format!("task panicked: {e}")))?
}

/// 誰かが`POST /api/accounts/request`でアクセス許可を申請したことを
/// 管理者へ通知する。
pub async fn send_access_request_notice(
    config: SmtpConfig,
    admin_email: String,
    request_email: String,
    repo: Option<String>,
    message: Option<String>,
) -> Result<(), MailError> {
    let repo_line = repo.as_deref().unwrap_or("(指定なし・アカウント全体)");
    let message_line = message.as_deref().unwrap_or("(メッセージなし)");
    let body = format!(
        "RGitへのアクセス許可申請が届きました。\n\n\
         申請者メール: {request_email}\n\
         対象リポジトリ: {repo_line}\n\
         メッセージ: {message_line}\n\n\
         管理者としてログインし、GET /api/accounts/requests で申請一覧を確認、\n\
         POST /api/accounts/requests/:id/decide で閲覧/ダウンロード/push を\n\
         個別に選んで許可・不許可を決定してください。"
    );
    tokio::task::spawn_blocking(move || build_and_send(&config, &admin_email, "RGit アクセス許可申請", body))
        .await
        .map_err(|e| MailError::Send(format!("task panicked: {e}")))?
}

/// アクセス許可申請の審査結果(承認/却下)を申請者へ通知する。
pub async fn send_access_decision(config: SmtpConfig, to: String, approved: bool, repo: Option<String>) -> Result<(), MailError> {
    let repo_line = repo.as_deref().unwrap_or("アカウント全体");
    let body = if approved {
        format!("RGitへのアクセス申請({repo_line})が承認されました。付与された権限の範囲でログイン・操作が可能です。")
    } else {
        format!("RGitへのアクセス申請({repo_line})は承認されませんでした。")
    };
    let subject = if approved { "RGit アクセス申請: 承認されました" } else { "RGit アクセス申請: 却下されました" };
    tokio::task::spawn_blocking(move || build_and_send(&config, &to, subject, body))
        .await
        .map_err(|e| MailError::Send(format!("task panicked: {e}")))?
}
