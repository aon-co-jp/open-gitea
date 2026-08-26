; open-gitea Windowsインストーラー(Inno Setup)。
;
; ユーザー指示「パワーシェル版とリポジトリ名-installer.exeの二種類を
; ご用意して」への対応。既存の`install.ps1`(管理者権限のPowerShell、
; サービス自動登録)はそのまま維持し、これは第二の選択肢——ダブル
; クリックだけで進むGUIインストーラー(内部で既存install.ps1を呼ぶ
; だけで、サービス登録ロジック自体は再実装しない)。
;
; 正直な開示: open-giteaはWindowsサービスとして自動登録・起動される
; 設計のため(既存install.ps1のデフォルト動作)、`PrivilegesRequired=
; admin`とし、Inno Setup標準のUAC昇格プロンプトを1回表示する
; (「管理者権限のPowerShellを手動で探して起動する」手間を無くす
; ことが目的、aruaru-dbと同じ方針)。
;
; ビルド方法: リポジトリルートで`cargo build --release --bin
; open-gitea`を実行した後、このディレクトリで`ISCC.exe
; open-gitea.iss`を実行する。

#define MyAppName "open-gitea"
#ifndef MyAppVersion
  #define MyAppVersion "0.0.0-local-build"
#endif
#define MyAppPublisher "aon-co-jp"
#define MyAppURL "https://github.com/aon-co-jp/open-gitea"
#define MyAppExeName "open-gitea.exe"

[Setup]
PrivilegesRequired=admin
AppId={{C3D4E5F6-8888-4A5B-8C6D-2E7F9A0B1C2D}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}/releases
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
UninstallDisplayIcon={app}\{#MyAppExeName}
Compression=lzma2
SolidCompression=yes
OutputDir=dist
OutputBaseFilename=open-gitea-installer
ArchitecturesInstallIn64BitMode=x64compatible
DisableProgramGroupPage=yes

[Languages]
Name: "japanese"; MessagesFile: "compiler:Languages\Japanese.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Files]
Source: "..\..\target\release\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\static\*"; DestDir: "{app}\static"; Flags: ignoreversion recursesubdirs
Source: "..\..\install.ps1"; DestDir: "{app}"; Flags: ignoreversion

[Run]
; 既存install.ps1のサービス登録ロジック(New-Service/Start-Service)を
; そのまま再利用する(重複実装を避ける)。
Filename: "powershell.exe"; \
    Parameters: "-NoProfile -ExecutionPolicy Bypass -File ""{app}\install.ps1"""; \
    StatusMsg: "Registering open-gitea as a Windows service... / open-giteaをWindowsサービスとして登録中..."; \
    Flags: waituntilterminated

[UninstallRun]
Filename: "powershell.exe"; \
    Parameters: "-NoProfile -Command ""Stop-Service open-gitea -ErrorAction SilentlyContinue; sc.exe delete open-gitea"""; \
    Flags: runhidden waituntilterminated; RunOnceId: "RemoveOpenGiteaService"

[UninstallDelete]
Type: filesandordirs; Name: "{app}"
