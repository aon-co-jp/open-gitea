# 開発方針＆開発環境ルール(RGit)

作業ドライブは`F:\runo`。この節は[`open-raid-z`](https://github.com/aon-co-jp/open-raid-z)の
`CLAUDE.md`を正本とし、各プロジェクトへコピーして同期する方針に準じる。
GitHubリポジトリ: [aon-co-jp/RGit](https://github.com/aon-co-jp/RGit)。

> ⚠️ **正直な開示(最重要)**: v0.1.0時点では、git smart HTTPプロトコル
> によるclone/push/fetchのみを実装。Gitea/GitBucketが持つWeb UI・Issue・
> Pull Request・Wiki・ユーザー認証・Webhookは一切無い。`README.md`参照。

## このプロジェクトの役割

Gitea(Go製)のRust版を目指す、自己ホスト型Git forge。GitHub上の
`aon-co-jp`組織の全リポジトリをバックアップ目的で自己ホスト環境へ
ミラーする用途を最初の実用シナリオとする(GitBucket/Gitea導入の代替)。

## 技術スタック

`aruaru-llm`・`e-gov.info`と同じ方針: `poem`クレートを直接利用する
単純なHTTPサービス。DB非依存(Gitリポジトリ自体がデータストア)。

## 実装方式

Gitプロトコル自体を再実装せず、`git http-backend`(gitに標準同梱される
CGIプログラム)をサブプロセスとして起動し、HTTPリクエストをCGI環境変数
(`PATH_INFO`/`QUERY_STRING`/`REQUEST_METHOD`/`CONTENT_TYPE`)へ変換して
橋渡しする(`src/main.rs`の`git_http_backend`関数)。認証は未実装
(`REMOTE_USER`は固定値"rgit")。

## HANDOFF

- **2026-07-21 新規作成・実機検証**: `runo-forge`という仮称で開発を
  開始した後、`aon-co-jp/RGit`という既存の空リポジトリ(説明文
  「Gitea(Go製)のRust版」)が見つかったため、正式名称を`RGit`に統一。
  ローカルで実機検証済み: `PUT /repos/:name`でbareリポジトリ作成→
  `git clone`→ファイル追加・commit→`git push`→別ディレクトリへ再clone
  →push内容が正しく取得できることを確認(モックではなく実際の`git`
  コマンドとの相互運用性を確認)。
  - 次にすべきこと: (1) GitHubの空リポジトリへ初回push、(2) VPS
    (conoha)へのデプロイ(systemdサービス化)、(3) `aon-co-jp`組織の
    全リポジトリをバックアップ目的でミラーする同期スクリプトとの接続。

- **2026-07-21 GitHub初回push・VPSデプロイ完了、README表示機能に着手
  (未検証部分あり、雷雨のため中断・チェックポイント)**:
  1. **完了・実機検証済み**: GitHubへの初回push成功
     ([aon-co-jp/RGit](https://github.com/aon-co-jp/RGit))。VPS(conoha)
     上でclone→`cargo build --release`→systemdサービス化
     (`/etc/systemd/system/rgit.service`)し、`healthz`で稼働確認済み
     (メモリ使用量1.5MB)。
  2. **完了・実機検証済み**: バックエンドに`GET /api/repos`
     (リポジトリ一覧、既存`list_repos`を再利用)・
     `GET /api/repos/:name/readme`(`git show <branch>:README.md`を
     サブプロセス実行してJSON化)を追加、`cargo build`成功を確認。
     `poem`の`static-files` feature有効化、`/ui`配下で`static/`を配信する
     設定を追加。
  3. **未検証(雷雨のため中断)**: GitHub README表示機能をWASMフロント
     エンド(`web/`、新規crate`rgit-web`)として実装。ユーザー指示により
     「省メモリ・ハイスピード」を追求する方向で、以下の判断を経た:
     - 当初`serde`/`serde_json`を使う設計→WASMバイナリサイズへの影響が
       大きいとユーザー指摘を受け撤回。
     - 次に`js_sys::JSON::parse`(ブラウザ組み込み)+`Reflect`でのJSON
       パースに変更→「JSON.parseをRJSON.parseとして開発して」という
       ユーザー指示を受け、自作の最小JSONパーサ`web/src/rjson.rs`
       (`RJson`、文字列エスケープ・`\uXXXX`・サロゲートペア対応の
       再帰下降パーサ、単体テスト4件同梱)を新規実装し、
       `js_sys`/`Reflect`依存も撤去。WASM↔JS境界を跨ぐ呼び出し回数の
       削減が狙い。
     - `web/Cargo.toml`に`opt-level="z"`+LTO+`panic=abort`+`strip=true`の
       release profileを追加(バイナリ極小化)。
     - **`cargo build --target wasm32-unknown-unknown --release`は
       雷雨によるシャットダウンのため未実行**。`rjson`の単体テスト
       (ネイティブターゲットでの`cargo test`)も未実行。次回セッション
       開始時に最優先で検証すること(型チェックだけで「完了」と
       報告しない、というこのエコシステム共通のルール通り)。
  - 次にすべきこと: (1) `web/`のネイティブテスト実行(`rjson`パーサの
    正しさ検証)、(2) `wasm32-unknown-unknown`ターゲットでのビルド、
    (3) `wasm-bindgen` CLIでJSグルーコード生成し`static/`へ配置、
    (4) 実ブラウザでリポジトリ一覧・README表示が実際に動くことを確認、
    (5) VPSへの再デプロイ、(6) 外部バックアップ同期スクリプトへの
    RGit自身の組み込み(同期先の詳細はVPS上の設定のみで管理し、
    このリポジトリには記載しない方針、次項参照)。

> ⚠️ **運用ルール(2026-07-21追記)**: 外部バックアップ先(アカウント名・
> ホスト名・トークン等)は、このリポジトリを含むいかなるGitリポジトリの
> コミット・ドキュメントにも記載しない。関連設定はVPS上の環境変数・
> 認証情報ファイル(`/root/.secrets/`等)のみで管理する。
