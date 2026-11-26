> Latest translations / 翻訳の最新版: [0-LATEST-TRANSLATIONS.md](0-LATEST-TRANSLATIONS.md)

# open-gitea

Giteaが持つ機能のうち、まずgit clone/push・OTPログイン・アクセス制御・README閲覧機能だけを実装した、Go言語版GiteaのRust＋RPoem版です。元の機能を素直に移植致しました。自己ホスト型Git forge。

**開発開始日: 2026-07-21**(このリポジトリのGitHub作成日、2026-07-22に`RGit`から`open-gitea`へ改名)

## 現状(v0.1.0)

> ⚠️ **正直な開示**: Gitea/GitBucketが持つIssue・Pull Request・
> Webhookは**まだ実装していません**。Wikiは実装しました(2026-07-22)。
> それ以外(Web UI・認証・アクセス制御)は実装済みです。実装済みなのは
> 以下の通りです。

- `git clone` / `git push` / `git fetch`(git smart HTTPプロトコル、`git http-backend`をCGI橋渡し、アクセス権限に応じてpush可否を判定) — 実機確認済み
- `PUT /repos/:name` によるbareリポジトリの新規作成(ディスク容量に応じた自動判定付き)
- `GET /repos` によるリポジトリ一覧取得
- `GET /healthz` ヘルスチェック
- **OTPメールログイン**(管理者+管理者が許可した登録アカウント)
- **だれでもログイン(デモ用)**: OTP不要、閲覧・ダウンロード・README閲覧のみ可能、pushは管理者のみ
- **アクセス制御**: private/public/groupの3モード、アカウント単位で閲覧・ダウンロード・push権限を個別設定
- **自己申請フロー**: 誰でもアクセス許可を申請でき、管理者がメールで気づいて承認/却下(閲覧/DL/push個別許可)
- **Wiki**: 各リポジトリ`<name>.git`の兄弟として`<name>.wiki.git`という素のbareリポジトリを自動作成(GitHub/GitLab/Gitea同様の設計)。閲覧はWeb UI(`GET /api/repos/:name/wiki`・`/wiki/:page`)、編集は`git clone`/`git push`で行う(Web版ページエディタは無い、正直な開示)。アクセス権限は本体リポジトリと共有(別権限系統は持たない)。
- **WASM製Web UI**(`/ui/`): リポジトリ一覧・README閲覧・Wiki閲覧・ログイン・管理パネル(申請一覧・アカウント管理・グループ管理・リポジトリ別アクセス設定)
- **電源プロファイル(省電力/省メモリ/常時電源接続)**: Web UI上のチェックボックスで切替可能。`install.sh`/`install.ps1`実行時にも選択でき、`RGIT_POWER_PROFILE`環境変数(例: `power_save,memory_saver`)としてサービスに設定される。ブラウザは`GET /api/power-profile`で初回訪問時だけこの既定値を読み込み、以後はユーザー自身の選択(`localStorage`)を優先する。

## 起動方法

```
RGIT_DATA_DIR=./data/repos RGIT_PORT=8090 cargo run --release
```

## なぜRustで作り直すか

Gitea(Go製)は512MBのメモリでも動作する実績があるが、それでもJVM製の
GitBucket等と比べて軽量。RS-Gitはさらに、Rustの所有権システムによる
メモリ安全性・省メモリ性を活かし、小規模VPSでの自己ホストをより
現実的にすることを目指す。

## ライセンス

Apache-2.0

## API 方針: REST 全廃・gRPC / Connect へ (2026-11-04)

### 日本語
- **REST は全廃する。新しい API は REST で作らない。** ブラウザや `curl` から呼ぶ API は **Connect の単純呼び出し**(`POST /<パッケージ>.<Service>/<Method>`、`content-type: application/json`、本文は JSON)、サービス間は **gRPC / gRPC-web**(RPoem の管理API `open_runo.admin.v1.Admin` と同じ流儀)。ストリーミングは、必要になってから。
- **既存の REST は、触るときに順に移す。** 移したら旧 REST の経路は消す。移行が終わるまで残るが、**新規には追加しない**。
- **JSON は RS-JSON に統一**する(読み込みは `from_str_secure` / `parse_secure` でサイズ・深さ・重複キーの上限つき、書き出しは `rust_json::fast`)。ブラウザの WASM に `prost` は入れない(サイズが増えるため)。
- **例外(自分たちの API ではなく、相手の規格が決めているもの)**: ACME、OAuth/OIDC のコールバック、第三者(GitHub など)からの Webhook、ヘルスチェック、静的ファイル配信、OTLP など。これらは規格どおりの HTTP を受ける。
- 参考実装: `RPoem`(`open_runo.admin.v1.Admin`)、`open-easy-web`(`server/src/connect.rs`、`proto/open_easy_web/admin/v1/admin.proto`)。
- ※ このリポジトリのコードが既に方針どおりかどうかは、各リポジトリの現状による。この節は方針の記録であり、コードの変更を意味しない。

### English
- **REST is being abolished. New APIs are not built as REST.** APIs called from browsers or `curl` use **Connect unary calls** (`POST /<package>.<Service>/<Method>`, `content-type: application/json`, JSON body); service-to-service calls use **gRPC / gRPC-web** (the same style as RPoem's admin API `open_runo.admin.v1.Admin`). Streaming only when it is actually needed.
- **Existing REST endpoints are migrated one by one, when they are touched.** The old REST route is removed once migrated. Until migration is finished they remain, but **no new ones are added**.
- **JSON is unified on RS-JSON**: reading with `from_str_secure` / `parse_secure` (size, depth and duplicate-key limits), writing with `rust_json::fast`. `prost` is not put into browser WASM (it would grow the binary).
- **Exceptions (shapes dictated by someone else's standard, not our own API)**: ACME, OAuth/OIDC callbacks, webhooks from third parties (e.g. GitHub), health checks, static file serving, OTLP, and so on. These accept plain HTTP as the standard requires.
- Reference implementations: `RPoem` (`open_runo.admin.v1.Admin`) and `open-easy-web` (`server/src/connect.rs`, `proto/open_easy_web/admin/v1/admin.proto`).
- Note: whether the code in a given repository already follows this policy depends on that repository. This section records the policy; it does not mean the code has been changed.

### Deutsch (KI-Übersetzung, redaktionell überarbeitet; noch nicht von Muttersprachlern geprüft)
- **REST wird vollständig abgeschafft; neue APIs entstehen nicht mehr als REST.** APIs, die aus dem Browser oder per `curl` aufgerufen werden, verwenden **unäre Connect-Aufrufe** (`POST /<Paket>.<Service>/<Methode>`, `content-type: application/json`, JSON-Body). Zwischen Diensten kommen **gRPC bzw. gRPC-web** zum Einsatz, im selben Stil wie die Admin-API von RPoem (`open_runo.admin.v1.Admin`). Streaming wird erst ergänzt, wenn es tatsächlich gebraucht wird.
- **Bestehende REST-Endpunkte werden schrittweise umgestellt, und zwar immer dann, wenn man ohnehin an ihnen arbeitet.** Sobald ein Endpunkt umgestellt ist, wird die alte REST-Route entfernt. Bis zum Abschluss der Migration bleiben die übrigen bestehen, es werden aber **keine neuen mehr hinzugefügt**.
- **JSON wird einheitlich über RS-JSON verarbeitet**: Gelesen wird mit `from_str_secure` bzw. `parse_secure` (mit Obergrenzen für Größe, Verschachtelungstiefe und doppelte Schlüssel), geschrieben mit `rust_json::fast`. `prost` wird nicht in das Browser-WASM aufgenommen, da es die Datei vergrößern würde.
- **Ausnahmen (Schnittstellen, deren Form ein fremder Standard vorgibt und die nicht zu unserer eigenen API gehören)**: ACME, OAuth/OIDC-Callbacks, Webhooks von Drittanbietern (z. B. GitHub), Health-Checks, die Auslieferung statischer Dateien, OTLP und Ähnliches. Diese nehmen, wie vom jeweiligen Standard verlangt, gewöhnliches HTTP entgegen.
- Referenzimplementierungen: `RPoem` (`open_runo.admin.v1.Admin`) und `open-easy-web` (`server/src/connect.rs`, `proto/open_easy_web/admin/v1/admin.proto`).
- Hinweis: Ob der Code eines einzelnen Repositorys diese Richtlinie bereits erfüllt, hängt vom jeweiligen Repository ab. Dieser Abschnitt dokumentiert lediglich die Richtlinie; er besagt nicht, dass der Code schon angepasst wurde.

### Русский (перевод выполнен с помощью ИИ и отредактирован; носителями языка пока не проверен)
- **От REST мы полностью отказываемся; новые API больше не создаются в стиле REST.** API, к которым обращаются из браузера или через `curl`, используют **унарные вызовы Connect** (`POST /<пакет>.<Service>/<Method>`, `content-type: application/json`, тело запроса — JSON). Для взаимодействия между сервисами применяются **gRPC и gRPC-web** — в том же стиле, что и административный API RPoem (`open_runo.admin.v1.Admin`). Потоковая передача данных добавляется лишь тогда, когда в ней действительно возникает необходимость.
- **Существующие REST-эндпоинты переводятся поэтапно — по мере того, как с ними приходится работать.** После перевода старый REST-маршрут удаляется. Пока миграция не завершена, оставшиеся эндпоинты сохраняются, но **новые не добавляются**.
- **Работа с JSON унифицирована на основе RS-JSON**: чтение — через `from_str_secure` или `parse_secure` (с ограничениями на размер, глубину вложенности и повторяющиеся ключи), запись — через `rust_json::fast`. `prost` в браузерный WASM не включается, поскольку это увеличило бы размер файла.
- **Исключения (интерфейсы, форму которых определяет чужой стандарт, а не наш собственный API)**: ACME, обратные вызовы OAuth/OIDC, вебхуки сторонних сервисов (например, GitHub), проверки работоспособности, раздача статических файлов, OTLP и тому подобное. Такие интерфейсы принимают обычный HTTP, как того требует соответствующий стандарт.
- Эталонные реализации: `RPoem` (`open_runo.admin.v1.Admin`) и `open-easy-web` (`server/src/connect.rs`, `proto/open_easy_web/admin/v1/admin.proto`).
- Примечание: соответствует ли код конкретного репозитория этой политике, зависит от самого репозитория. Данный раздел лишь фиксирует политику и не означает, что код уже изменён.

## API 方針の補足: 用語の整理と使い分け (2026-11-04)

### 日本語
**1つに統一するものではなく、別の層の話です。** 「REST の代わり」になるのは Connect・gRPC・GraphQL だけで、WASM は画面を作る技術です。

| 用語 | 何か | REST の代わりか |
|---|---|---|
| REST | URL と HTTP メソッドで操作を表す API の作り方 | 全廃する側 |
| **Connect** | `POST /サービス名/メソッド名` に JSON を送る形式。`curl` でもブラウザの `fetch` でも呼べる | はい(**採用**) |
| **gRPC** | 同じ「サービス/メソッド」の考え方を、バイナリ(protobuf)と HTTP/2 で行う形式。速く、ストリーミングも得意 | はい(**サービス同士に採用**) |
| gRPC-web | gRPC のブラウザ向けの変形版 | はい(必要なら) |
| GraphQL | 欲しい項目を問い合わせる形式 | 一部だけ(**問い合わせ(読み取り)専用**) |
| WASM | Rust をブラウザの中で動かす仕組み | いいえ(画面を作る技術) |

- **ブラウザの画面や `curl` から呼ぶ API は Connect**(POST、JSON を RS-JSON で読み書き)。ブラウザの WASM は素の gRPC を話せず、Connect なら `fetch` のままで、`prost` を足さずに済む(画面のサイズも増えない)。
- **サービス同士の通信(サーバー間、重い処理、ストリーミング)は gRPC。** 多くは Connect で足りるので、ストリーミングが必要になった所だけ gRPC に上げる。
- **GraphQL は、まとめて問い合わせたいときの読み取り専用。** 書き込み(操作)には使わない。
- **WASM は画面を作る技術として、そのまま使う。** 画面から呼ぶ API が Connect になっただけ。
- 外部の規格が形を決めるもの(ACME、OAuth のコールバック、第三者の Webhook、ヘルスチェック、静的ファイル、OTLP)は、規格どおりの HTTP のまま。
- **迷ったら Connect。**

### English
**These are not alternatives to unify; they sit at different layers.** Only Connect, gRPC and GraphQL can replace REST; WASM is a way to build the UI.

| Term | What it is | Replaces REST? |
|---|---|---|
| REST | API style that expresses operations through URLs and HTTP methods | The thing being abolished |
| **Connect** | `POST /Service/Method` with a JSON body; callable from `curl` and from a browser `fetch` | Yes (**adopted**) |
| **gRPC** | The same service/method idea over binary (protobuf) and HTTP/2; fast, good at streaming | Yes (**for service-to-service**) |
| gRPC-web | A browser-friendly variant of gRPC | Yes (if needed) |
| GraphQL | A query language: ask for exactly the fields you want | Partly (**read-only queries**) |
| WASM | Runs Rust inside the browser | No (a UI technology) |

- **APIs called from browser screens or `curl` use Connect** (POST, JSON read and written with RS-JSON). Browser WASM cannot speak plain gRPC; with Connect it stays a `fetch`, and `prost` is not needed (the binary does not grow).
- **Service-to-service calls (server to server, heavy work, streaming) use gRPC.** Connect is enough in most places; move a call to gRPC only where streaming is actually needed.
- **GraphQL is read-only**, for queries that gather several pieces of data. It is not used for writes (operations).
- **WASM stays as the way to build the UI.** The API it calls simply became Connect.
- Interfaces whose shape an outside standard dictates (ACME, OAuth callbacks, third-party webhooks, health checks, static files, OTLP) stay plain HTTP.
- **When in doubt, use Connect.**

### Deutsch (KI-Übersetzung, sprachlich überarbeitet; noch nicht von Muttersprachlern geprüft)
**Es geht nicht darum, eine einzige Technik auszuwählen – die Begriffe liegen auf unterschiedlichen Ebenen.** Als Ersatz für REST kommen nur Connect, gRPC und GraphQL in Frage; WASM dagegen ist eine Technik, mit der die Benutzeroberfläche gebaut wird.

| Begriff | Was ist das? | Ersatz für REST? |
|---|---|---|
| REST | Ein API-Stil, bei dem Operationen über URLs und HTTP-Methoden ausgedrückt werden | wird abgeschafft |
| **Connect** | `POST /Dienst/Methode` mit JSON im Body; aufrufbar mit `curl` ebenso wie mit `fetch` im Browser | ja (**gewählt**) |
| **gRPC** | Dasselbe Prinzip aus Dienst und Methode, aber über binäres protobuf und HTTP/2; schnell und gut für Streaming geeignet | ja (**zwischen Diensten**) |
| gRPC-web | Eine für den Browser angepasste Variante von gRPC | ja (bei Bedarf) |
| GraphQL | Abfragesprache, mit der man genau die benötigten Felder anfordert | teilweise (**nur für lesende Abfragen**) |
| WASM | Führt Rust-Code direkt im Browser aus | nein (Technik für die Oberfläche) |

- **APIs, die von Browseroberflächen oder per `curl` aufgerufen werden, verwenden Connect** (POST; JSON wird mit RS-JSON gelesen und geschrieben). WASM im Browser kann kein reines gRPC sprechen. Mit Connect bleibt es bei einem gewöhnlichen `fetch`, und `prost` ist nicht nötig – die WASM-Datei wird dadurch nicht größer.
- **Für die Kommunikation zwischen Diensten (Server zu Server, rechenintensive Aufgaben, Streaming) wird gRPC eingesetzt.** In den meisten Fällen reicht Connect aus; auf gRPC wird nur dort umgestellt, wo Streaming tatsächlich gebraucht wird.
- **GraphQL dient ausschließlich lesenden Abfragen**, wenn mehrere Informationen gebündelt abgefragt werden sollen. Für Schreibzugriffe (Operationen) wird es nicht verwendet.
- **WASM bleibt die Technik für die Oberfläche.** Geändert hat sich lediglich, dass die aufgerufene API nun auf Connect beruht.
- Schnittstellen, deren Form durch einen externen Standard festgelegt ist (ACME, OAuth-Callbacks, Webhooks von Drittanbietern, Health-Checks, statische Dateien, OTLP), bleiben gewöhnliches HTTP.
- **Im Zweifel: Connect.**

### Русский (перевод выполнен с помощью ИИ и литературно отредактирован; носителями языка пока не проверен)
**Это не варианты, из которых нужно выбрать один, а технологии разных уровней.** Заменить REST могут только Connect, gRPC и GraphQL; WASM же — это способ создания пользовательского интерфейса.

| Термин | Что это | Заменяет REST? |
|---|---|---|
| REST | Стиль построения API, при котором операции выражаются через URL и HTTP-методы | отменяется |
| **Connect** | `POST /Сервис/Метод` с телом в формате JSON; вызывается как из `curl`, так и из браузерного `fetch` | да (**выбран**) |
| **gRPC** | Тот же принцип «сервис — метод», но поверх двоичного формата protobuf и HTTP/2; работает быстро и хорошо подходит для потоковой передачи | да (**для связи между сервисами**) |
| gRPC-web | Адаптированный для браузера вариант gRPC | да (при необходимости) |
| GraphQL | Язык запросов: клиент запрашивает ровно те поля, которые ему нужны | частично (**только запросы на чтение**) |
| WASM | Позволяет выполнять код на Rust прямо в браузере | нет (технология для интерфейса) |

- **API, которые вызываются из браузерных интерфейсов или через `curl`, строятся на Connect** (POST; JSON читается и записывается через RS-JSON). Браузерный WASM не умеет работать с «чистым» gRPC. С Connect достаточно обычного `fetch`, а `prost` не требуется — размер WASM-файла не увеличивается.
- **Для обмена между сервисами (сервер — сервер, ресурсоёмкие задачи, потоковая передача) применяется gRPC.** В большинстве случаев достаточно Connect; на gRPC переходят только там, где потоковая передача действительно необходима.
- **GraphQL используется только для чтения**, когда нужно получить сразу несколько связанных данных. Для записи (операций) он не применяется.
- **WASM по-прежнему остаётся технологией для интерфейса.** Изменилось лишь то, что вызываемый им API теперь построен на Connect.
- Интерфейсы, форма которых задана внешним стандартом (ACME, обратные вызовы OAuth, вебхуки сторонних сервисов, проверки работоспособности, статические файлы, OTLP), остаются обычным HTTP.
- **Если есть сомнения — выбирайте Connect.**

## 補足: ストリーミングの対応状況 (2026-11-07)

### 日本語
- **server-streaming は対応済み**(`RPoem` の `open-runo-connect-*`、RPoem main `1570371`・`34cb595`)。Connect は `application/connect+json` のエンベロープ、gRPC / gRPC-web でも受ける。途中エラーは終端フレームで返し、件数・時間の上限、切断時の資源解放、実 TCP での `WatchEvents` 試験を通過。**VPS には未反映**。
- **使いどころ**: 変更の監視、進捗の逐次通知、長い生成の逐次返却など、サーバーから複数回返したい所。1回で済む呼び出しは従来どおり Connect の単純呼び出し。client-streaming / 双方向は、必要になってから。
- 独語・露語は未追記(日英のみ)。

### English
- **Server-streaming is supported** (`open-runo-connect-*` in RPoem, main `1570371` / `34cb595`): `application/connect+json` envelopes, also via gRPC / gRPC-web; mid-stream errors end with a terminal frame; item/time limits, resource release on disconnect, and a real-TCP `WatchEvents` test pass. **Not yet deployed to the VPS.**
- **When to use**: watching changes, progress updates, incremental results. One-shot calls stay plain Connect unary. Client-streaming / bidirectional only when needed.
- German and Russian not yet added.

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

## 再開用メッセージ / Resume message (2026-11-15、別アカウントで再開用 / for resuming on another account)

### 日本語
**到達点**: open-gitea の REST 全廃(Connect 化)は完了し、本番(VPS)へも反映済み(2026-11-07)。移した31本・例外(git smart HTTP・`/healthz`・`/ui`・`zip`/`raw` の GET)・試験結果は上の「REST 全廃・Connect 移行の状況」を参照。`cargo test` 全37件成功、実サーバー+ブラウザで確認済み。関連して `RS-Sync` の open-gitea プロバイダ(`2aa17fb`)も Connect 対応にして VPS の `rs-sync` を入れ替え済み。

**再開するリポジトリ**: `open-gitea`(作業用 clone は `F:\open-gitea-rest`。`F:\open-gitea` は未コミット変更があるため触らない)/ `RS-Sync`(`F:\RS-Sync-gitea`)/ メタ索引 `F:\runo`(別セッションが更新中。ここでは触らない)。

**未了・次の手**
1. **ログイン後の通し操作**: 管理者の OTP ログイン→リポジトリ作成→Issue/Wiki/アクセス設定を画面で通す(今回のブラウザ確認はデモログインまで。管理パネルはデモユーザーでは 401 が正常)。
2. **`/` が 404**: 画面は `/ui/` 配下の仕様。**2026-11-17 確認: 反映前のバイナリ(VPSのバックアップ)でも `/`=404・`/ui/`=404(新は `/ui/`=200)で、今回の変更による退行ではない**。**対応済み(2026-11-17、`ec68cb8`): `GET /` は相対リダイレクト(307)で `ui/` へ。本番反映済み(`easy-web.tokyo/open-gitea/` → `/open-gitea/ui/` が 200)。**
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
2. **`/` returns 404**: the UI is served under `/ui/` by design. **Checked 2026-11-17: the pre-deploy binary (VPS backup) also returns 404 for `/` (and for `/ui/`; the new one returns 200 for `/ui/`), so this is not a regression.** **Done 2026-11-17 (`ec68cb8`): `GET /` now returns a relative 307 to `ui/`; deployed (`easy-web.tokyo/open-gitea/` → `/open-gitea/ui/` returns 200).**
3. **RS-Sync real sync**: the VPS `rs-sync` was swapped to the Connect-aware build, but an actual sync into open-gitea has not been verified (only startup and HTTP 200).
4. **VPS git clone** `/root/open-gitea` has an uncommitted change (`Cargo.lock`), so `git pull` was not run; only the binary and `static/` were replaced. Decide whether to tidy it.
5. The intro page `webpage/index.html` exists (`d71328e`) but is not deployed to the VPS because `easy-web.tokyo/open-gitea` is the running app (needs a placement decision).

**Pitfalls**: the build needs `RS-JSON`, `RPoem`, `open-web-server`, `RS-SmartTCP`, `open-raid-z` and `open-cpu` next to the repo under `F:\` (path dependencies). WSL builds: `MSYS_NO_PATHCONV=1 wsl -d Ubuntu -- bash <script>`; the VPS glibc is 2.39. No `cargo fmt --all`. Never build on the VPS. Never print secrets (SMTP etc.; view `systemctl cat` with secrets hidden). Rollback backups on the VPS: `/root/open-gitea-bin.bak.*`, `/root/open-gitea-static.bak.*`, `/root/rs-sync-bin.bak.*`.

**Paste-ready resume message**
> Abolishing REST in open-gitea (move to Connect) is finished and deployed. Read the "再開用メッセージ / Resume message" section at the end of `README.md` / `CLAUDE.md` / `PORTING.md` in `F:\open-gitea-rest` (the open-gitea clone) and resume from the open items: the authenticated end-to-end flow, the `/` 404 handling, verifying a real RS-Sync → open-gitea sync, and tidying the VPS clone. Do not touch `F:\runo` or `F:\open-gitea` (uncommitted changes). Chat in Japanese only.

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
