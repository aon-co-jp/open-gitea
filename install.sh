#!/bin/sh
# open-gitea インストールスクリプト(AlmaLinux/Ubuntu/Debian/Fedora/RHEL等、
# systemdを使う主要Linuxディストリ共通)。
#
# 静的リンクされたmuslバイナリを使うため、ディストリ固有のライブラリ依存は
# 無い。root権限で実行すること。git本体(git http-backendを使うため)は
# 別途インストールされている必要がある。
#
# 使い方:
#   curl -fsSL https://github.com/aon-co-jp/open-gitea/releases/latest/download/open-gitea-linux-x86_64.tar.gz | tar xz
#   sudo ./install.sh
#
# 実バグ修正(2026-08-12): 2026-07-22にリポジトリ・バイナリ名が`RGit`から
# `open-gitea`へ改名され(VPS本番も既に`open-gitea.service`として稼働中)、
# 環境変数名(`RGIT_*`)は後方互換のため意図的に維持しつつバイナリ・
# パス・サービス名は追従するはずだったが、このスクリプトは追従漏れの
# ままだった——本番と食い違う名前で新規インストールが構築される状態を
# 修正。

set -eu

SRC_DIR="$(dirname "$0")"
BIN_SRC="${SRC_DIR}/open-gitea"
INSTALL_DIR="/usr/local/bin"
STATIC_DIR="/usr/local/share/open-gitea/static"
DATA_DIR="/var/lib/open-gitea"
SERVICE_FILE="/etc/systemd/system/open-gitea.service"

if [ "$(id -u)" -ne 0 ]; then
    echo "root権限で実行してください(例: sudo ./install.sh)" >&2
    exit 1
fi

if [ ! -f "$BIN_SRC" ]; then
    echo "open-gitea バイナリが見つかりません($BIN_SRC)。同梱のtar.gzを展開したディレクトリで実行してください。" >&2
    exit 1
fi

if ! command -v git >/dev/null 2>&1; then
    echo "警告: git コマンドが見つかりません。open-giteaはgit http-backend経由でclone/pushを処理するため、gitパッケージを別途インストールしてください(例: dnf install git / apt install git)。" >&2
fi

echo "==> バイナリを ${INSTALL_DIR}/open-gitea へ配置"
install -m 755 "$BIN_SRC" "${INSTALL_DIR}/open-gitea"

echo "==> WASM UI(static/)を ${STATIC_DIR} へ配置"
mkdir -p "$(dirname "$STATIC_DIR")"
rm -rf "$STATIC_DIR"
cp -r "${SRC_DIR}/static" "$STATIC_DIR"

echo "==> データディレクトリを ${DATA_DIR} に作成"
mkdir -p "$DATA_DIR"

# 電源プロファイル選択(エコシステム標準方針、open-raid-z/CLAUDE.md参照、
# 2026-08-07追加)。省電力・省メモリ・常時電源接続はチェックボックス相当
# (自由に複数選択可、番号をスペース区切りで入力)。常時電源接続を選ぶと
# NPU/GPU自動検出が有効になる(open-cuda側のGpuDevice抽象化を利用する
# 想定、このバイナリ自体はまだGPU非依存のためこのフラグは今のところ
# 効果を持たない——正直な開示、下記コメント参照)。非対話実行
# (CI・パイプ経由の`curl | sh`等でstdinが端末でない場合)は入力を求めず
# 既定の「通常」(いずれも未選択)のまま進む。
POWER_PROFILE=""
if [ -t 0 ]; then
    echo ""
    echo "==> 電源プロファイルを選択してください(複数選択可、スペース区切りで番号入力、Enterのみで「通常」):"
    echo "    1) 省電力 (power-saving)"
    echo "    2) 省メモリ (low-memory)"
    echo "    3) 常時電源接続 (always-on、NPU/GPU自動検出が有効になります)"
    printf "    番号> "
    read -r PROFILE_CHOICE || PROFILE_CHOICE=""
    for choice in $PROFILE_CHOICE; do
        case "$choice" in
            1) POWER_PROFILE="${POWER_PROFILE}power_save," ;;
            2) POWER_PROFILE="${POWER_PROFILE}memory_saver," ;;
            3) POWER_PROFILE="${POWER_PROFILE}always_on," ;;
        esac
    done
else
    echo "==> 非対話実行のため電源プロファイル選択をスキップ(既定: 通常)"
fi
echo "==> 選択された電源プロファイル: ${POWER_PROFILE:-(通常、未選択)}"

if [ ! -f "$SERVICE_FILE" ]; then
    echo "==> systemdサービスを作成(${SERVICE_FILE})"
    cat > "$SERVICE_FILE" << EOF
[Unit]
Description=open-gitea - self-hosted Git forge (Rust)
After=network.target

[Service]
Type=simple
WorkingDirectory=${DATA_DIR}
Environment=RGIT_DATA_DIR=${DATA_DIR}
Environment=RGIT_STATIC_DIR=${STATIC_DIR}
Environment=RGIT_PORT=8090
Environment=RGIT_POWER_PROFILE=${POWER_PROFILE}
# 管理者メール・SMTP設定は環境変数で指定すること(このファイルを直接
# 編集するか、/etc/systemd/system/open-gitea.service.d/override.confを
# 使うこと)。例:
#   Environment=RGIT_ADMIN_EMAIL=admin@example.com
#   Environment=RGIT_SMTP_HOST=smtp.example.com
# RGIT_POWER_PROFILEは上記インストール時の選択(省電力/省メモリ/常時
# 電源接続をカンマ区切り、例: power_save,memory_saver)。ブラウザ側は
# このデプロイを初めて開いたときだけこの既定値をチェックボックスの
# 初期状態として使い、以後はユーザー自身の選択(localStorage)を優先する。
# 変更したい場合はこの行を編集して`systemctl restart open-gitea`すること。
# 正直な開示: 常時電源接続(always_on)を選んでもこのバイナリ自体は
# NPU/GPU自動検出を実装していない(open-cuda連携は未着手、
# CLAUDE.mdのHANDOFF参照)——現状はUIチェックボックスの初期値設定のみの
# 効果。
ExecStart=${INSTALL_DIR}/open-gitea
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF
    systemctl daemon-reload
else
    echo "==> 既存のsystemdサービスが見つかったため上書きしません(${SERVICE_FILE})"
fi

echo "==> 完了。次のコマンドで管理者メール等を設定してから起動してください:"
echo "    sudo systemctl edit open-gitea  # Environment=RGIT_ADMIN_EMAIL=... 等を追記"
echo "    sudo systemctl enable --now open-gitea"
