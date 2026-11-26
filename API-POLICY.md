# API 方針 (open-gitea)

REST は全廃し、独自 API は Connect(`POST /open_gitea.v1.Gitea/<Method>`)。例外は規格が形を決めるものだけ: git smart HTTP、`/healthz`、静的ファイル、ブラウザが直接開くダウンロード GET(zip/raw)。Gitea 互換 API は目指していないため、`/api/repos` 等は独自設計として移行した。

## REST 全廃・Connect 移行の状況 (2026-11-07)

方針の正本は `aon-co-jp/runo` の `API-POLICY.md`。このリポジトリの要約は [`API-POLICY.md`](API-POLICY.md)。

- **移した(31本)**: 画面・外部クライアントが使う独自 API はすべて `POST /open_gitea.v1.Gitea/<Method>`(JSON、RS-JSON `parse_secure` でサイズ・深さ・重複キーを制限、エラーは `{"code","message"}`)へ。
  Repos(ListRepos/CreateRepo/GetReadme/GetReleases/ListWiki/GetWikiPage/GetTree/GetAccess/SetAccess)、Issues(ListIssues/CreateIssue/GetIssue/SetIssueStatus/UpdateIssueMetadata/CreateIssueComment)、
  Auth(RequestOtp/VerifyOtp/DemoLogin/Logout)、Groups(ListGroups/CreateGroup/DeleteGroup)、Accounts(ListAccounts/AddAccount/GetAccount/RemoveAccount/SetCreatePermission/RequestAccess/ListAccessRequests/DecideAccessRequest)、System(GetCapacity/GetPowerProfile)。
  旧 REST の経路(`/api/...`、`PUT /repos/:name`、`GET /repos`)は公開ルーターから削除済み。業務ロジック(認証・権限)は従来のハンドラを内部で共有する(`src/connect.rs` が橋渡し、対応表は `TABLE`)。
- **残した例外**: ① git smart HTTP(`info/refs`・`git-upload-pack`・`git-receive-pack`、Git の規格)、② `GET /healthz`、③ `/ui`(静的ファイル)。
  ④ **`GET /api/repos/:name/zip`・`GET /api/repos/:name/raw/*`**(2本): ブラウザが直接開く/リンクとして共有されるバイナリ配信で、`?token=` で認証する(方針の「ブラウザが直接開く GET」、副作用なし)。
- **呼び出し元も同時に変更**: WASM 画面(`web/src`、`static/open_gitea_web*` を再ビルド)、`static/index.html` のデモログイン(古い接頭辞 `/rgit` が残っていたため `/open-gitea` に修正)、
  [`RS-Sync`](https://github.com/aon-co-jp/RS-Sync) の open-gitea プロバイダ(`2aa17fb`)。open-github-backup / backup-to-discord-by-github に open-gitea の API 呼び出しは無かった。
- **試験**: `cargo test` 全37件成功(旧 REST が使えないこと、401/400/重複キー400/405/404 を新規テスト)。実サーバーを `curl` で確認。ブラウザで画面を開き、デモログインと `ListRepos`/`GetCapacity` が 200、白画面なし。
- **本番(VPS)へ反映済み(2026-11-07)**: WSL でリリースビルド(glibc 2.39)→ `/root/open-gitea` のバイナリと `static/` を入れ替え(バックアップは `/root/open-gitea-bin.bak.*`・`open-gitea-static.bak.*`、失敗時は自動ロールバック)。公開URLで `ListRepos` 200・旧 `/api/repos` 404・`/ui/` 200 を確認。VPS 上の `rs-sync` も Connect 対応版に入れ替え済み(open-gitea と同期するため)。VPS の git clone は未コミット変更(Cargo.lock)があるため `git pull` していない(バイナリとstaticのみ置換)。

## 2026-11-23 現状 / Status (日本語・English)
**日本語**: 本番確認済み。`/open-gitea/` は307で `ui/` へ、`/ui/` 200、旧 `/api/repos` 404、Connect `ListRepos` 200。VPSの `open-gitea`・`rs-sync` は active、直近2日のエラー/panic 0。依存は base64 0.23 まで更新済み(`d223513`)、`docs-latest` のCIは成功。**未了**: ログイン後の管理操作の通し(所有者のメールOTPが必要)、RS-Syncから実際の同期、VPSの `/root/open-gitea` クローンの整理(分岐の可能性、`git pull` しない・要確認)、紹介ページの置き場所の判断。再開するリポジトリ: `open-gitea`・`RS-Sync`・`runo`。
**English**: Verified in production: `/open-gitea/` returns 307 to `ui/`, `/ui/` 200, old `/api/repos` 404, Connect `ListRepos` 200. VPS `open-gitea` and `rs-sync` are active with 0 errors/panics in 2 days. Dependencies are updated through base64 0.23 (`d223513`) and the `docs-latest` CI passes. **Open**: authenticated admin end-to-end (needs the owner's e-mail OTP), a real RS-Sync sync, tidying the VPS `/root/open-gitea` clone (may have diverged; do not `git pull`, ask first), and deciding where the intro page lives. Repos to resume: `open-gitea`, `RS-Sync`, `runo`.

## 2026-11-26 Versionless 別名 / Versionless alias
**日本語**: Versionless API 方針に沿い、版なしの `POST /open_gitea.Gitea/<Method>` を追加しました(`.v1` は呼び出し側を壊さないため当面残し、段階的に畳みます)。本番反映は未実施。
**English**: Following the Versionless API policy, `POST /open_gitea.Gitea/<Method>` (no version) was added; `.v1` stays for now and will be retired in stages. Not yet deployed to production.

## 2026-11-26 版なし別名を本番へ反映 / Versionless alias deployed
**日本語**: `a3bceb5` を WSL Ubuntu-24.04(glibc 2.39、検査OK)でビルドし、VPS の open-gitea を入れ替えました。確認: 版なし `/open_gitea.Gitea/ListRepos` 200、`.v1` 200、旧 `/api/repos` 404、公開URL `/open-gitea/` 307・`/ui/` 200、エラーログ0。旧版は `open-gitea.before-versionless-20261126`(戻すときはこれを元の名前へ戻して再起動)。
**English**: `a3bceb5` was built in WSL Ubuntu-24.04 (glibc 2.39, check OK) and swapped into the VPS. Verified: versionless `/open_gitea.Gitea/ListRepos` 200, `.v1` 200, old `/api/repos` 404, public `/open-gitea/` 307 and `/ui/` 200, error log 0. Rollback: restore `open-gitea.before-versionless-20261126` and restart.

## 2026-11-26 `.v1` を削除・本番反映と再開用メッセージ / `.v1` removed, deployed, and resume message
**日本語**: ユーザー指示で `.v1` を猶予なしに削除し、Connect の入口は版なし `POST /open_gitea.Gitea/<Method>` だけにしました(`.v1` が404になる試験を追加、テスト全成功)。呼び出し側の RS-Sync(`src/provider.rs`)も版なしへ直し、**先に RS-Sync を、次に open-gitea を**WSL Ubuntu-24.04(glibc 2.39、検査OK)でビルドしてバックアップ付きで本番入れ替えしました(旧版 `rs-sync.before-v1del-20261126`・`open-gitea.before-v1del-20261126`)。実測: 版なし ListRepos 200、`.v1` 404、旧 `/api/repos` 404、healthz 200、`/open-gitea/ui/` 200、`rs-sync`・`rs-sync-demo`・`open-gitea` active、エラーログ0。**未了**: ログイン後の管理操作の通し(所有者のメールOTPが要る)、RS-Sync から実際の同期の確認(トークンが要る)、VPS の `/root/open-gitea` クローンの整理(分岐の可能性、`git pull` しない・要確認)、紹介ページの置き場所の判断。
**再開用メッセージ(別アカウントでそのまま貼り付け)**: 前の続きです。`open-gitea`(作業は `F:\open-gitea-rest`、`F:\open-gitea` は触らない)と `RS-Sync`(`F:\RS-Sync-gitea`)の README/CLAUDE/PORTING 末尾「2026-11-26」を読んでから、未了(上記)を確認なしで進めてください。決まり: 新しいRESTは作らない(Connectのみ・版なし)、秘密は表示しない、VPSは配備と確認だけ(ビルドは WSL `Ubuntu-24.04`+`check-glibc.sh`、バックアップ付き)、実機で確認してから完了と書く、返答は日本語のみ、`git pull` ではなく `fetch`+`merge --ff-only`。
**English**: At the user's instruction `.v1` was removed with no grace period; the only Connect entrance is the versionless `POST /open_gitea.Gitea/<Method>` (a test that `.v1` returns 404 was added; all tests pass). The caller RS-Sync (`src/provider.rs`) was switched to the versionless path too, and **RS-Sync first, then open-gitea** were built in WSL Ubuntu-24.04 (glibc 2.39, check OK) and swapped into production with backups (`rs-sync.before-v1del-20261126`, `open-gitea.before-v1del-20261126`). Measured: versionless ListRepos 200, `.v1` 404, old `/api/repos` 404, healthz 200, `/open-gitea/ui/` 200, `rs-sync`, `rs-sync-demo` and `open-gitea` active, error log 0. **Open**: authenticated admin end-to-end (needs the owner's e-mail OTP), a real RS-Sync sync (needs tokens), tidying the VPS `/root/open-gitea` clone (may have diverged; do not `git pull`, ask first), and deciding where the intro page lives.
**Resume message (paste as is on another account)**: Continuing from before. Read the "2026-11-26" section at the end of README/CLAUDE/PORTING in `open-gitea` (work in `F:\open-gitea-rest`; do not touch `F:\open-gitea`) and `RS-Sync` (`F:\RS-Sync-gitea`), then work through the open items above without asking. Rules: no new REST (Connect only, versionless), never print secrets, VPS only for deploy and verification (build in WSL `Ubuntu-24.04` + `check-glibc.sh`, with backups), verify on real systems before saying done, reply in Japanese only, use `fetch` + `merge --ff-only`, never `git pull`.
