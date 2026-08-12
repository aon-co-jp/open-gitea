# open-gitea インストールスクリプト(Windows / Windows Server 共通)。
#
# 使い方(管理者権限のPowerShellで):
#   Invoke-WebRequest -Uri "https://github.com/aon-co-jp/open-gitea/releases/latest/download/open-gitea-windows-x86_64.zip" -OutFile open-gitea.zip
#   Expand-Archive open-gitea.zip -DestinationPath open-gitea
#   cd open-gitea
#   .\install.ps1
#
# git本体(git http-backend経由でclone/pushを処理するため)は別途インストール
# されている必要があります(https://git-scm.com/download/win)。
#
# 実バグ修正(2026-08-12、ユーザー指示「aruaru-dbやaruaru-llmなどの
# インストールもより簡単にして」を受けたエコシステム横断調査で発覚):
# 2026-07-22にリポジトリ・バイナリ名が`RGit`から`open-gitea`へ改名され
# (`CLAUDE.md`のHANDOFF参照、VPS本番も既に`open-gitea.service`として
# 稼働中)、release.ymlも同時に更新されたはずだったが、この
# install.ps1(ダウンロードURL・バイナリ名`rgit.exe`・インストール先
# `C:\Program Files\RGit`・サービス名`RGit`)は追従漏れのまま放置されて
# いた——本番と食い違う名前で新規インストールが構築される状態だった。
# **環境変数名(`RGIT_*`)は既存デプロイとの後方互換のため意図的に
# そのまま維持している**(2026-07-27 HANDOFFの決定を踏襲)。
# また、従来はサービス登録コマンドを画面に表示するだけでユーザー自身が
# コピペ実行する必要があったが、今回サービス未登録の場合は自動登録・
# 自動起動まで行うよう変更した(`-SkipServiceRegistration`で従来通り
# 印刷のみに戻せる)。

#Requires -RunAsAdministrator

param(
    [switch]$SkipServiceRegistration
)

$ErrorActionPreference = "Stop"

$InstallDir = "C:\Program Files\open-gitea"
$DataDir = "C:\ProgramData\open-gitea\data"
$ServiceName = "open-gitea"

Write-Host "==> インストール先: $InstallDir"
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
New-Item -ItemType Directory -Force -Path $DataDir | Out-Null

$BinSrc = Join-Path $PSScriptRoot "open-gitea.exe"
if (-not (Test-Path $BinSrc)) {
    Write-Error "open-gitea.exe が見つかりません($BinSrc)。zipを展開したディレクトリで実行してください。"
    exit 1
}
Copy-Item $BinSrc -Destination $InstallDir -Force

$StaticSrc = Join-Path $PSScriptRoot "static"
if (Test-Path $StaticSrc) {
    Write-Host "==> WASM UI(static/)を配置"
    Copy-Item $StaticSrc -Destination $InstallDir -Recurse -Force
}

if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
    Write-Warning "git コマンドが見つかりません。open-giteaはgit http-backend経由でclone/pushを処理するため、Git for Windowsを別途インストールしてください(https://git-scm.com/download/win)。"
}

# 電源プロファイル選択(エコシステム標準方針、open-raid-z/CLAUDE.md参照、
# 2026-08-07追加)。省電力・省メモリ・常時電源接続はチェックボックス相当
# (自由に複数選択可、番号をカンマ/スペース区切りで入力)。非対話実行
# (自動化パイプライン等)では入力を求めず既定の「通常」のまま進む。
$PowerProfileTokens = New-Object System.Collections.Generic.List[string]
if ([Environment]::UserInteractive -and -not [Console]::IsInputRedirected) {
    Write-Host ""
    Write-Host "==> 電源プロファイルを選択してください(複数選択可、カンマ/スペース区切りで番号入力、Enterのみで「通常」):"
    Write-Host "    1) 省電力 (power-saving)"
    Write-Host "    2) 省メモリ (low-memory)"
    Write-Host "    3) 常時電源接続 (always-on、NPU/GPU自動検出が有効になります)"
    $profileChoice = Read-Host "    番号"
    foreach ($choice in ($profileChoice -split '[,\s]+')) {
        switch ($choice.Trim()) {
            "1" { $PowerProfileTokens.Add("power_save") }
            "2" { $PowerProfileTokens.Add("memory_saver") }
            "3" { $PowerProfileTokens.Add("always_on") }
        }
    }
} else {
    Write-Host "==> 非対話実行のため電源プロファイル選択をスキップ(既定: 通常)"
}
$PowerProfile = [string]::Join(",", $PowerProfileTokens)
Write-Host "==> 選択された電源プロファイル: $(if ($PowerProfile) { $PowerProfile } else { '(通常、未選択)' })"
# 正直な開示: 常時電源接続(always_on)を選んでもこのバイナリ自体は
# NPU/GPU自動検出を実装していない(open-cuda連携は未着手、
# CLAUDE.mdのHANDOFF参照)——現状はブラウザUIのチェックボックス初期値
# 設定のみの効果(このデプロイを初めて開いたときだけ適用、以後は
# ユーザー自身の選択を優先)。

$existing = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
if ($existing) {
    Write-Host "==> 既存のWindowsサービスが見つかったため、バイナリのみ更新しました(再起動は行いません)"
    Write-Host "    手動で再起動する場合: Restart-Service $ServiceName"
} elseif ($SkipServiceRegistration) {
    Write-Host "==> -SkipServiceRegistration指定のため、Windowsサービスとしての登録はスキップしました。手動で登録する場合の手順:"
    Write-Host "      [Environment]::SetEnvironmentVariable('RGIT_ADMIN_EMAIL', 'admin@example.com', 'Machine')"
    Write-Host "      [Environment]::SetEnvironmentVariable('RGIT_DATA_DIR', '$DataDir', 'Machine')"
    Write-Host "      [Environment]::SetEnvironmentVariable('RGIT_STATIC_DIR', '$InstallDir\static', 'Machine')"
    Write-Host "      [Environment]::SetEnvironmentVariable('RGIT_PORT', '8090', 'Machine')"
    Write-Host "      [Environment]::SetEnvironmentVariable('RGIT_POWER_PROFILE', '$PowerProfile', 'Machine')"
    Write-Host "      New-Service -Name $ServiceName -BinaryPathName '$InstallDir\open-gitea.exe' -DisplayName 'open-gitea' -StartupType Automatic"
    Write-Host "      Start-Service $ServiceName"
} else {
    Write-Host "==> Windowsサービスとして自動登録します($ServiceName)"
    Write-Host "    正直な開示: 管理者メール(RGIT_ADMIN_EMAIL)・SMTP設定は未設定のままです。"
    Write-Host "    OTPログイン機能を使う場合は別途システム環境変数を設定してから 'Restart-Service $ServiceName' してください。"
    [Environment]::SetEnvironmentVariable('RGIT_DATA_DIR', $DataDir, 'Machine')
    [Environment]::SetEnvironmentVariable('RGIT_STATIC_DIR', "$InstallDir\static", 'Machine')
    [Environment]::SetEnvironmentVariable('RGIT_PORT', '8090', 'Machine')
    [Environment]::SetEnvironmentVariable('RGIT_POWER_PROFILE', $PowerProfile, 'Machine')
    New-Service -Name $ServiceName -BinaryPathName "$InstallDir\open-gitea.exe" -DisplayName "open-gitea" -StartupType Automatic | Out-Null
    Start-Service -Name $ServiceName
    Write-Host "==> サービス '$ServiceName' を登録・起動しました(Get-Service $ServiceName で確認できます)"
}

Write-Host "==> 完了。"
