# RGit インストールスクリプト(Windows / Windows Server 共通)。
#
# 使い方(管理者権限のPowerShellで):
#   Invoke-WebRequest -Uri "https://github.com/aon-co-jp/RGit/releases/latest/download/rgit-windows-x86_64.zip" -OutFile rgit.zip
#   Expand-Archive rgit.zip -DestinationPath rgit
#   cd rgit
#   .\install.ps1
#
# git本体(git http-backend経由でclone/pushを処理するため)は別途インストール
# されている必要があります(https://git-scm.com/download/win)。

#Requires -RunAsAdministrator

$ErrorActionPreference = "Stop"

$InstallDir = "C:\Program Files\RGit"
$DataDir = "C:\ProgramData\RGit\data"
$ServiceName = "RGit"

Write-Host "==> インストール先: $InstallDir"
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
New-Item -ItemType Directory -Force -Path $DataDir | Out-Null

$BinSrc = Join-Path $PSScriptRoot "rgit.exe"
if (-not (Test-Path $BinSrc)) {
    Write-Error "rgit.exe が見つかりません($BinSrc)。zipを展開したディレクトリで実行してください。"
    exit 1
}
Copy-Item $BinSrc -Destination $InstallDir -Force

$StaticSrc = Join-Path $PSScriptRoot "static"
if (Test-Path $StaticSrc) {
    Write-Host "==> WASM UI(static/)を配置"
    Copy-Item $StaticSrc -Destination $InstallDir -Recurse -Force
}

if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
    Write-Warning "git コマンドが見つかりません。RGitはgit http-backend経由でclone/pushを処理するため、Git for Windowsを別途インストールしてください(https://git-scm.com/download/win)。"
}

$existing = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
if ($existing) {
    Write-Host "==> 既存のWindowsサービスが見つかったため、バイナリのみ更新しました(再起動は行いません)"
    Write-Host "    手動で再起動する場合: Restart-Service $ServiceName"
} else {
    Write-Host "==> Windowsサービスとして登録($ServiceName)"
    Write-Host "    管理者メール・SMTP設定は環境変数で指定する必要があります。"
    Write-Host "    例(サービス登録前に環境変数を設定する場合、システム環境変数として設定してください):"
    Write-Host "      [Environment]::SetEnvironmentVariable('RGIT_ADMIN_EMAIL', 'admin@example.com', 'Machine')"
    Write-Host "      [Environment]::SetEnvironmentVariable('RGIT_DATA_DIR', '$DataDir', 'Machine')"
    Write-Host "      [Environment]::SetEnvironmentVariable('RGIT_STATIC_DIR', '$InstallDir\static', 'Machine')"
    Write-Host "      [Environment]::SetEnvironmentVariable('RGIT_PORT', '8090', 'Machine')"
    Write-Host ""
    Write-Host "    環境変数を設定した後、以下でサービス登録・起動してください:"
    Write-Host "      New-Service -Name $ServiceName -BinaryPathName '$InstallDir\rgit.exe' -DisplayName 'RGit' -StartupType Automatic"
    Write-Host "      Start-Service $ServiceName"
}

Write-Host "==> 完了。"
