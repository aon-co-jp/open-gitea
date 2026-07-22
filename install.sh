#!/bin/sh
# RGit インストールスクリプト(AlmaLinux/Ubuntu/Debian/Fedora/RHEL等、
# systemdを使う主要Linuxディストリ共通)。
#
# 静的リンクされたmuslバイナリを使うため、ディストリ固有のライブラリ依存は
# 無い。root権限で実行すること。git本体(git http-backendを使うため)は
# 別途インストールされている必要がある。
#
# 使い方:
#   curl -fsSL https://github.com/aon-co-jp/RGit/releases/latest/download/rgit-linux-x86_64.tar.gz | tar xz
#   sudo ./install.sh

set -eu

SRC_DIR="$(dirname "$0")"
BIN_SRC="${SRC_DIR}/rgit"
INSTALL_DIR="/usr/local/bin"
STATIC_DIR="/usr/local/share/rgit/static"
DATA_DIR="/var/lib/rgit"
SERVICE_FILE="/etc/systemd/system/rgit.service"

if [ "$(id -u)" -ne 0 ]; then
    echo "root権限で実行してください(例: sudo ./install.sh)" >&2
    exit 1
fi

if [ ! -f "$BIN_SRC" ]; then
    echo "rgit バイナリが見つかりません($BIN_SRC)。同梱のtar.gzを展開したディレクトリで実行してください。" >&2
    exit 1
fi

if ! command -v git >/dev/null 2>&1; then
    echo "警告: git コマンドが見つかりません。RGitはgit http-backend経由でclone/pushを処理するため、gitパッケージを別途インストールしてください(例: dnf install git / apt install git)。" >&2
fi

echo "==> バイナリを ${INSTALL_DIR}/rgit へ配置"
install -m 755 "$BIN_SRC" "${INSTALL_DIR}/rgit"

echo "==> WASM UI(static/)を ${STATIC_DIR} へ配置"
mkdir -p "$(dirname "$STATIC_DIR")"
rm -rf "$STATIC_DIR"
cp -r "${SRC_DIR}/static" "$STATIC_DIR"

echo "==> データディレクトリを ${DATA_DIR} に作成"
mkdir -p "$DATA_DIR"

if [ ! -f "$SERVICE_FILE" ]; then
    echo "==> systemdサービスを作成(${SERVICE_FILE})"
    cat > "$SERVICE_FILE" << EOF
[Unit]
Description=RGit - self-hosted Git forge (Rust)
After=network.target

[Service]
Type=simple
WorkingDirectory=${DATA_DIR}
Environment=RGIT_DATA_DIR=${DATA_DIR}
Environment=RGIT_STATIC_DIR=${STATIC_DIR}
Environment=RGIT_PORT=8090
# 管理者メール・SMTP設定は環境変数で指定すること(このファイルを直接
# 編集するか、/etc/systemd/system/rgit.service.d/override.confを
# 使うこと)。例:
#   Environment=RGIT_ADMIN_EMAIL=admin@example.com
#   Environment=RGIT_SMTP_HOST=smtp.example.com
ExecStart=${INSTALL_DIR}/rgit
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
echo "    sudo systemctl edit rgit  # Environment=RGIT_ADMIN_EMAIL=... 等を追記"
echo "    sudo systemctl enable --now rgit"
