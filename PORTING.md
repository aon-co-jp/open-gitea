# PORTING

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

再開時は `src/connect.rs` の `TABLE` が Connect メソッドと内部経路の対応表。メソッド追加は `TABLE` と `build_routes` の内部ルーターの両方へ。

## 再開用メッセージ / Resume message (2026-11-15、別アカウントで再開用 / for resuming on another account)

### 日本語
**到達点**: open-gitea の REST 全廃(Connect 化)は完了し、本番(VPS)へも反映済み(2026-11-07)。移した31本・例外(git smart HTTP・`/healthz`・`/ui`・`zip`/`raw` の GET)・試験結果は上の「REST 全廃・Connect 移行の状況」を参照。`cargo test` 全37件成功、実サーバー+ブラウザで確認済み。関連して `RS-Sync` の open-gitea プロバイダ(`2aa17fb`)も Connect 対応にして VPS の `rs-sync` を入れ替え済み。

**再開するリポジトリ**: `open-gitea`(作業用 clone は `F:\open-gitea-rest`。`F:\open-gitea` は未コミット変更があるため触らない)/ `RS-Sync`(`F:\RS-Sync-gitea`)/ メタ索引 `F:\runo`(別セッションが更新中。ここでは触らない)。

**未了・次の手**
1. **ログイン後の通し操作**: 管理者の OTP ログイン→リポジトリ作成→Issue/Wiki/アクセス設定を画面で通す(今回のブラウザ確認はデモログインまで。管理パネルはデモユーザーでは 401 が正常)。
2. **`/` が 404**: 画面は `/ui/` 配下の仕様。**2026-11-17 確認: 反映前のバイナリ(VPSのバックアップ)でも `/`=404・`/ui/`=404(新は `/ui/`=200)で、今回の変更による退行ではない**。残りは入口(`/` から `/ui/` への振り替え)をするかの判断のみ。
3. **RS-Sync の実同期**: VPS の `rs-sync` を Connect 対応版に入れ替えたが、open-gitea へ実際に同期が通る確認は未了(起動と HTTP 200 までは確認)。
4. **VPS の git clone** `/root/open-gitea` は未コミット変更(`Cargo.lock`)があり `git pull` していない。バイナリと `static/` だけ置換済み。必要なら整理を判断する。
5. 紹介ページ `webpage/index.html` は追加済み(`d71328e`)だが、`easy-web.tokyo/open-gitea` が稼働中のアプリのため VPS へは未配置(置き場所の判断待ち)。

**落とし穴**: ビルドは `F:\` 直下に `RS-JSON`・`RPoem`・`open-web-server`・`RS-SmartTCP`・`open-raid-z`・`open-cpu` が必要(パス依存)。WSL ビルドは `MSYS_NO_PATHCONV=1 wsl -d Ubuntu -- bash <script>`、VPS の glibc は 2.39。`cargo fmt --all` 禁止。VPS では**ビルドしない**。秘密(SMTP 等)は画面に出さない(`systemctl cat` は伏せて見る)。入れ替えのバックアップは VPS の `/root/open-gitea-bin.bak.*`・`/root/open-gitea-static.bak.*`・`/root/rs-sync-bin.bak.*`。

**そのまま貼り付ける再開用メッセージ**
> open-gitea の REST 全廃(Connect 化)は完了・本番反映済みです。`F:\open-gitea-rest`(open-gitea の clone)の `README.md` / `CLAUDE.md` / `PORTING.md` 末尾の「再開用メッセージ」を読み、未了(管理者ログイン後の通し操作、`/` が404の扱い、RS-Sync の open-gitea への実同期確認、VPS の clone の整理)から再開してください。`F:\runo` と `F:\open-gitea`(未コミット変更あり)は触らないこと。チャットは日本語のみ。

### English
**Where things stand**: abolishing REST in open-gitea (move to Connect) is done and deployed to production (VPS) on 2026-11-07. See "REST 全廃・Connect 移行の状況" above for the 31 migrated routes, the exceptions (git smart HTTP, `/healthz`, `/ui`, `zip`/`raw` GETs) and test results. `cargo test` passes 37/37; checked against a real server and in a browser. The `RS-Sync` open-gitea provider (`2aa17fb`) was also moved to Connect and the VPS `rs-sync` binary was swapped.

**Repositories to resume**: `open-gitea` (work clone `F:\open-gitea-rest`; do not touch `F:\open-gitea`, which has uncommitted changes) / `RS-Sync` (`F:\RS-Sync-gitea`) / the meta index `F:\runo` (another session is editing it; leave it alone).

**Open items / next steps**
1. **Authenticated end-to-end flow**: admin OTP login → create a repository → Issues / Wiki / access settings in the UI. (The browser check only covered demo login; the admin panels returning 401 for the demo user is expected.)
2. **`/` returns 404**: the UI is served under `/ui/` by design. **Checked 2026-11-17: the pre-deploy binary (VPS backup) also returns 404 for `/` (and for `/ui/`; the new one returns 200 for `/ui/`), so this is not a regression.** Only the decision on redirecting `/` to `/ui/` remains.
3. **RS-Sync real sync**: the VPS `rs-sync` was swapped to the Connect-aware build, but an actual sync into open-gitea has not been verified (only startup and HTTP 200).
4. **VPS git clone** `/root/open-gitea` has an uncommitted change (`Cargo.lock`), so `git pull` was not run; only the binary and `static/` were replaced. Decide whether to tidy it.
5. The intro page `webpage/index.html` exists (`d71328e`) but is not deployed to the VPS because `easy-web.tokyo/open-gitea` is the running app (needs a placement decision).

**Pitfalls**: the build needs `RS-JSON`, `RPoem`, `open-web-server`, `RS-SmartTCP`, `open-raid-z` and `open-cpu` next to the repo under `F:\` (path dependencies). WSL builds: `MSYS_NO_PATHCONV=1 wsl -d Ubuntu -- bash <script>`; the VPS glibc is 2.39. No `cargo fmt --all`. Never build on the VPS. Never print secrets (SMTP etc.; view `systemctl cat` with secrets hidden). Rollback backups on the VPS: `/root/open-gitea-bin.bak.*`, `/root/open-gitea-static.bak.*`, `/root/rs-sync-bin.bak.*`.

**Paste-ready resume message**
> Abolishing REST in open-gitea (move to Connect) is finished and deployed. Read the "再開用メッセージ / Resume message" section at the end of `README.md` / `CLAUDE.md` / `PORTING.md` in `F:\open-gitea-rest` (the open-gitea clone) and resume from the open items: the authenticated end-to-end flow, the `/` 404 handling, verifying a real RS-Sync → open-gitea sync, and tidying the VPS clone. Do not touch `F:\runo` or `F:\open-gitea` (uncommitted changes). Chat in Japanese only.
