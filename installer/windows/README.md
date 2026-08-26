# このフォルダについて / About this folder

`open-gitea.iss` はこのインストーラーを**作るための** [Inno Setup](https://jrsoftware.org/isinfo.php)
ビルドスクリプトです。`open-gitea-installer.exe` はこのフォルダ内に実体として置いてあります
(ユーザー指示により、ローカルビルドしたバイナリを直接コミット)。

**⬇ 今すぐダウンロード**: [open-gitea-installer.exe](open-gitea-installer.exe)

**正直な開示**: このファイルはビルド成果物であり、`open-gitea.iss`やソースコードを変更しても
自動的には更新されません(手動での再ビルド・再コミットが必要)。現時点でこのインストーラーを
自動ビルド・公開するCIは無いため、このファイルが唯一の配布経路です。

---

# About this folder

`open-gitea.iss` is the [Inno Setup](https://jrsoftware.org/isinfo.php) build script used to
**produce** this installer. `open-gitea-installer.exe` itself is committed directly into this
folder (per explicit user instruction, a locally-built binary is committed rather than only
published elsewhere).

**⬇ Download now**: [open-gitea-installer.exe](open-gitea-installer.exe)

**Honest disclosure**: this file is a build artifact. Changes to `open-gitea.iss` or the source
code do not automatically update it (a manual rebuild and re-commit is required). There is
currently no CI that automatically builds and publishes this installer, so this file is the only
distribution path.
