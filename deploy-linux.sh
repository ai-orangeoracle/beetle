#!/bin/bash
# Beetle Linux Deployment Script
#
# What this is for (vs manual scp):
# - One flow: pick arch → SSH target → upload /opt/beetle/beetle (+ optional systemd unit)
# - Reuses one SSH connection (multiplexing) so password is not asked per file
# - Remembers last IP/user/port in ~/.config/beetle/
# - If packaging/linux/embed-deps/<arch>/ contains iw/hostapd/dnsmasq, uploads to /opt/beetle/bin/
# - Before upload: if device lacks those tools AND embed-deps is empty, asks whether to continue
# - Optional: BEETLE_EMBED_DEPS_URL=https://.../beetle-linux-deps-armv7.tar.xz downloads & extracts
#   into packaging/linux/embed-deps/<arch>/ (must match published ABI; see docs)

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo ""
echo "=========================================="
echo "  Beetle Linux Deployment"
echo "=========================================="
echo ""

# Optional: download a **project-published** tarball of iw/hostapd/dnsmasq into embed-deps before upload.
# ABI must match the device — only use URLs from beetle releases built for that class of board.
fetch_embed_deps_from_url() {
    if [ -z "${BEETLE_EMBED_DEPS_URL:-}" ]; then
        return 0
    fi
    case "$BEETLE_EMBED_DEPS_URL" in
        https://* | http://*) ;;
        *)
            echo -e "${RED}BEETLE_EMBED_DEPS_URL must be http(s)${NC}"
            exit 1
            ;;
    esac

    local dest="$SCRIPT_DIR/packaging/linux/embed-deps/$SELECTED_ARCH"
    mkdir -p "$dest"

    echo "========== Fetch embed-deps (BEETLE_EMBED_DEPS_URL) =========="
    echo ""
    echo "  URL: $BEETLE_EMBED_DEPS_URL"
    echo "  → $dest"
    echo ""

    local tmp
    tmp=$(mktemp "${TMPDIR:-/tmp}/beetle-deps.XXXXXX")
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL "$BEETLE_EMBED_DEPS_URL" -o "$tmp"
    elif command -v wget >/dev/null 2>&1; then
        wget -q -O "$tmp" "$BEETLE_EMBED_DEPS_URL"
    else
        echo -e "${RED}Install curl or wget to use BEETLE_EMBED_DEPS_URL${NC}"
        rm -f "$tmp"
        exit 1
    fi

    if [ -n "${BEETLE_EMBED_DEPS_SHA256:-}" ]; then
        local got
        got=$(shasum -a 256 "$tmp" | awk '{print $1}')
        if [ "$got" != "$BEETLE_EMBED_DEPS_SHA256" ]; then
            echo -e "${RED}SHA256 mismatch (expected $BEETLE_EMBED_DEPS_SHA256 got $got)${NC}"
            rm -f "$tmp"
            exit 1
        fi
        echo -e "${GREEN}✓ SHA256 OK${NC}"
    fi

    local exdir
    exdir=$(mktemp -d "${TMPDIR:-/tmp}/beetle-deps-ex.XXXXXX")
    if ! tar -xf "$tmp" -C "$exdir" 2>/dev/null; then
        echo -e "${RED}Failed to extract archive (need .tar / .tar.gz / .tar.xz)${NC}"
        rm -rf "$exdir" "$tmp"
        exit 1
    fi
    rm -f "$tmp"

    local n=0
    local f
    while IFS= read -r f; do
        case "$(basename "$f")" in
            iw | hostapd | dnsmasq)
                cp -f "$f" "$dest/"
                chmod +x "$dest/$(basename "$f")"
                n=$((n + 1))
                ;;
        esac
    done < <(find "$exdir" -type f \( -name iw -o -name hostapd -o -name dnsmasq \) 2>/dev/null)

    rm -rf "$exdir"

    if [ "$n" -lt 1 ]; then
        echo -e "${YELLOW}Warning: archive contained no iw/hostapd/dnsmasq; nothing copied.${NC}"
    else
        echo -e "${GREEN}✓ Placed $n helper(s) under packaging/linux/embed-deps/$SELECTED_ARCH/${NC}"
    fi
    echo ""
}

# Detect available binaries
detect_binaries() {
    local binaries=()

    if [ -f "target/x86_64-unknown-linux-musl/release/beetle" ]; then
        binaries+=("x86_64")
    fi

    if [ -f "target/armv7-unknown-linux-musleabihf/release/beetle" ]; then
        binaries+=("armv7")
    fi

    if [ -f "target/aarch64-unknown-linux-musl/release/beetle" ]; then
        binaries+=("aarch64")
    fi

    echo "${binaries[@]}"
}

# Select architecture
select_arch() {
    local available=($(detect_binaries))

    if [ ${#available[@]} -eq 0 ]; then
        echo -e "${RED}Error: No compiled binaries found${NC}"
        echo "Please run ./build.sh first"
        exit 1
    fi

    echo "Available builds:"
    local i=1
    for arch in "${available[@]}"; do
        echo "  $i) $arch"
        ((i++))
    done
    echo ""

    if [ ${#available[@]} -eq 1 ]; then
        SELECTED_ARCH="${available[0]}"
        echo -e "${GREEN}Auto-selected: $SELECTED_ARCH${NC}"
    else
        read -p "Select architecture [1-${#available[@]}]: " choice
        choice=${choice:-1}
        SELECTED_ARCH="${available[$((choice-1))]}"
    fi

    case "$SELECTED_ARCH" in
        x86_64)
            BINARY_PATH="target/x86_64-unknown-linux-musl/release/beetle"
            ;;
        armv7)
            BINARY_PATH="target/armv7-unknown-linux-musleabihf/release/beetle"
            ;;
        aarch64)
            BINARY_PATH="target/aarch64-unknown-linux-musl/release/beetle"
            ;;
    esac

    echo ""
}

# Persist last successful target (IP / user / port). Password is never stored.
# Path: ~/.config/beetle/deploy-linux.defaults (mode 600).
DEPLOY_DEFAULTS_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/beetle"
DEPLOY_DEFAULTS_FILE="$DEPLOY_DEFAULTS_DIR/deploy-linux.defaults"

load_deploy_defaults() {
    DEFAULT_DEVICE_IP=""
    DEFAULT_DEVICE_USER="root"
    DEFAULT_SSH_PORT="22"
    if [ ! -f "$DEPLOY_DEFAULTS_FILE" ]; then
        return 0
    fi
    while IFS= read -r line || [ -n "$line" ]; do
        case "$line" in
            ''|\#*) continue ;;
        esac
        case "$line" in
            DEVICE_IP=*) DEFAULT_DEVICE_IP="${line#DEVICE_IP=}" ;;
            DEVICE_USER=*) DEFAULT_DEVICE_USER="${line#DEVICE_USER=}" ;;
            SSH_PORT=*) DEFAULT_SSH_PORT="${line#SSH_PORT=}" ;;
        esac
    done <"$DEPLOY_DEFAULTS_FILE"
}

save_deploy_defaults() {
    mkdir -p "$DEPLOY_DEFAULTS_DIR"
    (
        umask 077
        {
            echo "# beetle deploy-linux — last successful target (do not commit)"
            printf 'DEVICE_IP=%s\n' "$DEVICE_IP"
            printf 'DEVICE_USER=%s\n' "$DEVICE_USER"
            printf 'SSH_PORT=%s\n' "$SSH_PORT"
        } >"${DEPLOY_DEFAULTS_FILE}.tmp"
        mv "${DEPLOY_DEFAULTS_FILE}.tmp" "$DEPLOY_DEFAULTS_FILE"
    )
}

# Input device information
input_device_info() {
    echo "========== Device Information =========="
    echo ""
    load_deploy_defaults

    if [ -n "$DEFAULT_DEVICE_IP" ]; then
        echo -e "${GREEN}Saved target: ${DEFAULT_DEVICE_USER}@${DEFAULT_DEVICE_IP}:${DEFAULT_SSH_PORT}${NC}"
        echo "(Press Enter to keep; password is not saved — use SSH keys for passwordless login)"
        echo ""
    fi

    if [ -n "$DEFAULT_DEVICE_IP" ]; then
        read -p "Device IP address [$DEFAULT_DEVICE_IP]: " DEVICE_IP
    else
        read -p "Device IP address: " DEVICE_IP
    fi
    DEVICE_IP=${DEVICE_IP:-$DEFAULT_DEVICE_IP}
    if [ -z "$DEVICE_IP" ]; then
        echo -e "${RED}Error: IP address cannot be empty${NC}"
        exit 1
    fi

    read -p "Username [${DEFAULT_DEVICE_USER}]: " DEVICE_USER
    DEVICE_USER=${DEVICE_USER:-$DEFAULT_DEVICE_USER}

    read -p "SSH port [${DEFAULT_SSH_PORT}]: " SSH_PORT
    SSH_PORT=${SSH_PORT:-$DEFAULT_SSH_PORT}

    if ! [[ "$SSH_PORT" =~ ^[0-9]+$ ]] || [ "$SSH_PORT" -lt 1 ] || [ "$SSH_PORT" -gt 65535 ]; then
        echo -e "${RED}Error: SSH port must be a number between 1 and 65535${NC}"
        exit 1
    fi

    echo ""
    echo -e "${BLUE}Target device: ${DEVICE_USER}@${DEVICE_IP}:${SSH_PORT}${NC}"
    echo ""
}

# Reuse one SSH connection for the whole script so password (or keyboard-interactive)
# is not prompted on every ssh/scp invocation. Requires OpenSSH client.
# macOS: $TMPDIR is often under /var/folders/...; ControlPath = dir + %C + ssh suffix
# can exceed AF_UNIX sun_path (~104 bytes). Prefer /tmp (short path); %C keeps names compact.
setup_ssh_mux() {
    if [ -d /tmp ] && [ -w /tmp ]; then
        SSH_MUX_DIR=$(mktemp -d /tmp/bd.XXXXXX)
    else
        SSH_MUX_DIR=$(mktemp -d "${TMPDIR:-/tmp}/bd.XXXXXX")
    fi
    chmod 700 "$SSH_MUX_DIR"
    SSH_MUX_OPTS=(
        -o "ControlMaster=auto"
        -o "ControlPath=$SSH_MUX_DIR/%C"
        -o "ControlPersist=300"
    )
}

cleanup_ssh_mux() {
    if [ -n "${SSH_MUX_DIR:-}" ] && [ -n "${DEVICE_USER:-}" ] && [ -n "${DEVICE_IP:-}" ]; then
        ssh "${SSH_MUX_OPTS[@]}" -p "$SSH_PORT" -o BatchMode=yes -O exit \
            "${DEVICE_USER}@${DEVICE_IP}" 2>/dev/null || true
    fi
    if [ -n "${SSH_MUX_DIR:-}" ] && [ -d "$SSH_MUX_DIR" ]; then
        rm -rf "$SSH_MUX_DIR"
    fi
}

# Test connection
test_connection() {
    echo "========== Testing Connection =========="
    echo ""

    if ssh "${SSH_MUX_OPTS[@]}" -p "$SSH_PORT" -o ConnectTimeout=5 -o BatchMode=yes \
        "${DEVICE_USER}@${DEVICE_IP}" "echo 'OK'" &>/dev/null; then
        echo -e "${GREEN}✓ SSH connection successful${NC}"
    else
        echo -e "${YELLOW}⚠ SSH connection failed, password may be required${NC}"
        echo "Testing connection..."
        if ! ssh "${SSH_MUX_OPTS[@]}" -p "$SSH_PORT" "${DEVICE_USER}@${DEVICE_IP}" "echo 'OK'"; then
            echo -e "${RED}Error: Cannot connect to device${NC}"
            exit 1
        fi
    fi

    save_deploy_defaults

    echo ""
}

# Detect device architecture
detect_device_arch() {
    echo "========== Detecting Device =========="
    echo ""

    # One remote shell (mapfile needs bash 4+; keep portable for macOS /bin/bash)
    _uname_out=$(
        ssh "${SSH_MUX_OPTS[@]}" -p "$SSH_PORT" "${DEVICE_USER}@${DEVICE_IP}" \
            "uname -m; uname -s" 2>/dev/null || printf '%s\n' unknown unknown
    )
    DEVICE_ARCH=$(printf '%s\n' "$_uname_out" | sed -n '1p')
    DEVICE_OS=$(printf '%s\n' "$_uname_out" | sed -n '2p')
    DEVICE_ARCH=${DEVICE_ARCH:-unknown}
    DEVICE_OS=${DEVICE_OS:-unknown}

    echo "Device architecture: $DEVICE_ARCH"
    echo "Operating system: $DEVICE_OS"

    # Architecture match check
    case "$DEVICE_ARCH" in
        x86_64)
            if [ "$SELECTED_ARCH" != "x86_64" ]; then
                echo -e "${YELLOW}⚠ Warning: Device is x86_64, but selected $SELECTED_ARCH${NC}"
            fi
            ;;
        armv7l)
            if [ "$SELECTED_ARCH" != "armv7" ]; then
                echo -e "${YELLOW}⚠ Warning: Device is armv7l, but selected $SELECTED_ARCH${NC}"
            fi
            ;;
        aarch64)
            if [ "$SELECTED_ARCH" != "aarch64" ]; then
                echo -e "${YELLOW}⚠ Warning: Device is aarch64, but selected $SELECTED_ARCH${NC}"
            fi
            ;;
    esac

    echo ""
}

# Select deployment mode
select_deploy_mode() {
    echo "========== Deployment Mode =========="
    echo ""
    echo "  1) Quick deploy (binary only)"
    echo "  2) Full deploy (binary + systemd service)"
    echo "  3) Update binary only"
    echo ""

    read -p "Select mode [1-3] (default 2): " mode
    DEPLOY_MODE=${mode:-2}
    echo ""
}

# True if packaging/linux/embed-deps/<arch>/ has at least one non-doc file.
local_embed_deps_nonempty() {
    local embed="$SCRIPT_DIR/packaging/linux/embed-deps/$SELECTED_ARCH"
    local f
    if [ ! -d "$embed" ]; then
        return 1
    fi
    for f in "$embed"/*; do
        [ -f "$f" ] || continue
        case "$(basename "$f")" in
            README*|*.md|*.txt) continue ;;
        esac
        return 0
    done
    return 1
}

# Remote: any of iw/hostapd/dnsmasq missing on PATH → sets REMOTE_WIFI_INCOMPLETE=1 else 0
probe_remote_wifi_tools() {
    REMOTE_WIFI_INCOMPLETE=0
    local out
    out=$(
        ssh "${SSH_MUX_OPTS[@]}" -p "$SSH_PORT" "${DEVICE_USER}@${DEVICE_IP}" \
            'for c in iw hostapd dnsmasq; do
                command -v "$c" >/dev/null 2>&1 || echo MISSING
            done'
    )
    case "$out" in
        *MISSING*) REMOTE_WIFI_INCOMPLETE=1 ;;
    esac
}

# Before upload: if device lacks tools and this PC has no embed-deps, ask to continue or abort.
prompt_wifi_helpers_or_continue() {
    echo "========== WiFi helper tools (preflight) =========="
    echo ""
    probe_remote_wifi_tools
    if [ "${REMOTE_WIFI_INCOMPLETE:-0}" -eq 0 ]; then
        echo -e "${GREEN}  Device already has iw, hostapd, and dnsmasq on PATH — nothing extra to bundle.${NC}"
        echo ""
        return 0
    fi
    if local_embed_deps_nonempty; then
        echo -e "${GREEN}  This PC has files under packaging/linux/embed-deps/$SELECTED_ARCH/ — they will be uploaded to /opt/beetle/bin/.${NC}"
        echo ""
        return 0
    fi
    echo -e "${YELLOW}  The device is missing one or more of: iw, hostapd, dnsmasq (on PATH).${NC}"
    echo -e "${YELLOW}  Beetle’s Linux WiFi needs them; this script does not download or pull from firmware.${NC}"
    echo ""
    echo "  To bundle helpers: put binaries named iw, hostapd, dnsmasq on **this computer** under:"
    echo "    $SCRIPT_DIR/packaging/linux/embed-deps/$SELECTED_ARCH/"
    echo ""
    read -p "  Deploy beetle binary only (no WiFi helpers this time)? [Y/n]: " wifi_ans
    wifi_ans=${wifi_ans:-Y}
    case "$wifi_ans" in
        [Nn]*)
            echo ""
            echo "Aborted. Add the three tools to embed-deps (or install them on the device), then run ./deploy-linux.sh again."
            exit 0
            ;;
    esac
    echo ""
}

# Optional bundled WiFi userland (iw, hostapd, dnsmasq) for distros without opkg/apk/apt.
# Place binaries in packaging/linux/embed-deps/<arch>/ (same arch as selected build).
EMBED_DEPS_UPLOADED=0
upload_embed_deps() {
    local embed="$SCRIPT_DIR/packaging/linux/embed-deps/$SELECTED_ARCH"
    EMBED_DEPS_UPLOADED=0
    if [ ! -d "$embed" ]; then
        return 0
    fi
    local has=""
    local f
    for f in "$embed"/*; do
        [ -f "$f" ] || continue
        case "$(basename "$f")" in
            README*|*.md|*.txt) continue ;;
        esac
        has=1
        break
    done
    if [ -z "$has" ]; then
        return 0
    fi
    echo "Uploading bundled WiFi tools → /opt/beetle/bin ..."
    ssh "${SSH_MUX_OPTS[@]}" -p "$SSH_PORT" "${DEVICE_USER}@${DEVICE_IP}" \
        "mkdir -p /opt/beetle/bin"
    for f in "$embed"/*; do
        [ -f "$f" ] || continue
        case "$(basename "$f")" in
            README*|*.md|*.txt) continue ;;
        esac
        scp "${SSH_MUX_OPTS[@]}" -P "$SSH_PORT" "$f" \
            "${DEVICE_USER}@${DEVICE_IP}:/opt/beetle/bin/"
    done
    ssh "${SSH_MUX_OPTS[@]}" -p "$SSH_PORT" "${DEVICE_USER}@${DEVICE_IP}" \
        'for f in /opt/beetle/bin/*; do [ -f "$f" ] && chmod a+x "$f"; done'
    EMBED_DEPS_UPLOADED=1
    echo -e "${GREEN}✓ Bundled tools uploaded (beetle prefers /opt/beetle/bin)${NC}"
}

# Upload files
upload_files() {
    echo "========== Uploading Files =========="
    echo ""

    echo "Creating directories..."
    ssh "${SSH_MUX_OPTS[@]}" -p "$SSH_PORT" "${DEVICE_USER}@${DEVICE_IP}" \
        "mkdir -p /opt/beetle /opt/beetle/bin /var/lib/beetle"

    upload_embed_deps

    echo "Uploading binary..."
    scp "${SSH_MUX_OPTS[@]}" -P "$SSH_PORT" "$BINARY_PATH" \
        "${DEVICE_USER}@${DEVICE_IP}:/opt/beetle/beetle"

    if [ "$DEPLOY_MODE" = "2" ]; then
        echo "Uploading systemd service..."
        scp "${SSH_MUX_OPTS[@]}" -P "$SSH_PORT" packaging/linux/beetle.service \
            "${DEVICE_USER}@${DEVICE_IP}:/tmp/"
    fi

    echo -e "${GREEN}✓ Upload complete${NC}"
    echo ""
    report_wifi_tools_on_device
}

# Shell-side check: script never "extracts from firmware"; it only uploads files you placed
# under packaging/linux/embed-deps/<arch>/ on this computer.
report_wifi_tools_on_device() {
    echo "========== WiFi tools on device (iw / hostapd / dnsmasq) =========="
    echo ""
    local miss=0
    local line
    while IFS= read -r line; do
        echo "  $line"
        case "$line" in
            *MISS*) miss=1 ;;
        esac
    done < <(
        ssh "${SSH_MUX_OPTS[@]}" -p "$SSH_PORT" "${DEVICE_USER}@${DEVICE_IP}" \
            'for c in iw hostapd dnsmasq; do
                if command -v "$c" >/dev/null 2>&1; then
                    p=$(command -v "$c")
                    echo "OK $c → $p"
                else
                    echo "MISS $c"
                fi
            done'
    )
    echo ""
    if [ "${EMBED_DEPS_UPLOADED:-0}" -eq 1 ]; then
        echo -e "${GREEN}  Also uploaded from this PC: packaging/linux/embed-deps/$SELECTED_ARCH/ → /opt/beetle/bin/${NC}"
        echo ""
        return 0
    fi
    if [ "$miss" -eq 1 ]; then
        echo -e "${YELLOW}  This deploy script does NOT auto-copy tools from device firmware.${NC}"
        echo -e "${YELLOW}  Put matching binaries on **this computer** under:${NC}"
        echo -e "${YELLOW}    packaging/linux/embed-deps/$SELECTED_ARCH/${NC}"
        echo -e "${YELLOW}  then run ./deploy-linux.sh again (or install those packages on the device if you can).${NC}"
        echo -e "${YELLOW}  See docs/zh-cn/deploy-linux.md${NC}"
        echo ""
    fi
}

# Install service
install_service() {
    echo "========== Installing Service =========="
    echo ""

    ssh "${SSH_MUX_OPTS[@]}" -p "$SSH_PORT" "${DEVICE_USER}@${DEVICE_IP}" << 'REMOTE_EOF'
chmod +x /opt/beetle/beetle

if [ -f /tmp/beetle.service ]; then
    cp /tmp/beetle.service /etc/systemd/system/
    systemctl daemon-reload
    echo "✓ systemd service installed"
fi
REMOTE_EOF

    echo ""
}

# Show next steps
show_next_steps() {
    echo "=========================================="
    echo "  Deployment Complete!"
    echo "=========================================="
    echo ""
    echo "Next steps:"
    echo ""
    if [ "${EMBED_DEPS_UPLOADED:-0}" -eq 1 ]; then
        echo "  (Bundled iw/hostapd/dnsmasq are under /opt/beetle/bin on the device.)"
        echo "  Beetle looks there first — you do not need to edit PATH on the device for these."
        echo ""
    fi
    echo "  1. Start beetle:"
    echo "     - With systemd: systemctl start beetle"
    echo "     - Without systemd: ssh -p $SSH_PORT ${DEVICE_USER}@${DEVICE_IP}"
    echo "       then: nohup /opt/beetle/beetle >> /var/log/beetle.log 2>&1 &"
    echo ""
    echo "  2. Configure WiFi (after WiFi stack works):"
    echo "     Hotspot SSID Beetle → http://DEVICE_IP/ (or http://192.168.1.4 on SoftAP)"
    echo ""
    echo "Configuration files:"
    echo "  - State directory: /var/lib/beetle"
    echo "  - Service config: /etc/systemd/system/beetle.service"
    echo ""
}

# 主流程
main() {
    select_arch
    fetch_embed_deps_from_url
    input_device_info
    setup_ssh_mux
    trap cleanup_ssh_mux EXIT INT TERM
    test_connection
    detect_device_arch
    select_deploy_mode
    prompt_wifi_helpers_or_continue
    upload_files

    if [ "$DEPLOY_MODE" = "2" ]; then
        install_service
    fi

    show_next_steps
}

main "$@"
