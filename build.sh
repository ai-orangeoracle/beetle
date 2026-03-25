#!/usr/bin/env bash
# One-shot build script with interactive platform selection.
set -e
SCRIPT_ROOT="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_ROOT"

# Colors (same palette as deploy-linux.sh)
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

show_help() {
  cat <<'EOF'
Usage:
  ./build.sh [--flash | --flash-update] [--no-monitor] [cargo build args...]

Flash:
  --flash          Build then flash; interactive menu (default: update only, keep NVS).
  --flash-update   Build then flash without erase (no menu; same as option 1).

Quick examples:
  ./build.sh
  TARGET=linux ./build.sh
  TARGET=linux-armv7 ./build.sh
  TARGET=esp ./build.sh --flash
  TARGET=esp ./build.sh --flash-update

Notes:
  - On macOS building Linux musl, prefer Docker (zero local linker setup).
  - For Linux builds, this script uses Rust stable toolchain.
EOF
}

# Fast-path help.
for arg in "$@"; do
  case "$arg" in
    -h|--help) show_help; exit 0 ;;
  esac
done

MSG_TITLE="Beetle Build Script"
MSG_SELECT_PLATFORM="Select build platform:"
MSG_PLATFORM_ESP="ESP32-S3 Firmware (default)"
MSG_PLATFORM_LINUX="Linux x86_64"
MSG_PLATFORM_LINUX_ARMV7="Linux armv7 (32-bit ARM hard-float)"
MSG_INPUT_OPTION="Enter option"
MSG_PRESS_ENTER="press Enter for"
MSG_INVALID_OPTION="Invalid option, enter"
MSG_LINUX_MODE="Linux Build Mode"
MSG_DETECTED_LINUX="Detected Linux system, using native build"
MSG_DETECTED_MACOS="Detected macOS system, cross-compiling to Linux"
MSG_SELECT_METHOD="Select build method:"
MSG_METHOD_DOCKER="Docker build (recommended, no setup needed)"
MSG_METHOD_MUSL="musl-cross toolchain (requires installation)"
MSG_ERROR_NO_DOCKER="Error: Docker not found"
MSG_INSTALL_DOCKER="Please install Docker Desktop"
MSG_UNKNOWN_OS="Warning: Unknown system"
MSG_TRY_NATIVE="trying native build"
MSG_ESP_MODE="ESP32 Build Mode"
MSG_USING_DOCKER="Using Docker for build"
MSG_BUILD_IN_DOCKER="Building in Docker"
MSG_BUILD_COMPLETE="Build complete"
MSG_BINARY="Binary"
# 固定 target 到本仓库，避免环境/IDE 将 CARGO_TARGET_DIR 指到临时目录导致 esp-idf-sys bindings 与 esp-idf-svc cfg 不一致。
export CARGO_TARGET_DIR="${SCRIPT_ROOT}/target"
export PATH="${HOME}/.cargo/bin:${PATH}"
command -v cargo &>/dev/null || { echo "Error: cargo not found. Install Rust: https://rustup.rs" >&2; exit 1; }

# --- Parse args (same as build.ps1) ---
DO_FLASH=""
NO_MONITOR=""
FLASH_NO_ERASE=""
BUILD_METHOD="${BUILD_METHOD:-auto}" # auto | docker | local
BUILD_ARGS=()
for arg in "$@"; do
  case "$arg" in
    -h|--help)       show_help; exit 0 ;;
    --flash)         DO_FLASH=1 ;;
    --flash-update)  DO_FLASH=1; FLASH_NO_ERASE=1 ;;
    --no-monitor)    NO_MONITOR=1 ;;
    *)               BUILD_ARGS+=("$arg") ;;
  esac
done

run_linux_docker_build() {
  local target="$1"
  echo "  $MSG_USING_DOCKER"
  echo ""
  echo "========== $MSG_BUILD_IN_DOCKER =========="
  if [[ "$target" == "x86_64-unknown-linux-musl" ]]; then
    docker run --rm -v "$SCRIPT_ROOT":/workspace -w /workspace \
      rust:latest \
      bash -c "rustup target add x86_64-unknown-linux-musl && cargo build --release --target x86_64-unknown-linux-musl"
  elif [[ "$target" == "armv7-unknown-linux-musleabihf" ]]; then
    docker run --rm -v "$SCRIPT_ROOT":/home/rust/src -w /home/rust/src \
      messense/rust-musl-cross:armv7-musleabihf \
      cargo build --release --target armv7-unknown-linux-musleabihf
  else
    echo "Error: Docker build not supported for target: $target" >&2
    exit 1
  fi
}

# --- Interactive platform selection ---
select_build_platform() {
  # If TARGET env is set, skip interactive prompt.
  if [[ -n "${TARGET:-}" ]]; then
    case "${TARGET}" in
      esp|esp32) PLATFORM_CHOICE=1; return 0 ;;  # ESP32
      linux) PLATFORM_CHOICE=2; return 0 ;;      # Linux
      linux-armv7|armv7) PLATFORM_CHOICE=3; return 0 ;;
      *) echo "Error: Unknown TARGET=$TARGET. Use 'esp', 'linux', or 'linux-armv7'" >&2; exit 1 ;;
    esac
  fi

  # If --flash / --flash-update is set, default to ESP32.
  if [[ -n "$DO_FLASH" ]]; then
    PLATFORM_CHOICE=1
    return 0
  fi

  echo ""
  echo "=========================================="
  echo "  $MSG_TITLE"
  echo "=========================================="
  echo ""
  echo "$MSG_SELECT_PLATFORM"
  echo "  1) $MSG_PLATFORM_ESP"
  echo "  2) $MSG_PLATFORM_LINUX"
  echo "  3) $MSG_PLATFORM_LINUX_ARMV7"
  echo ""

  while true; do
    read -r -p "$MSG_INPUT_OPTION [1-3] ($MSG_PRESS_ENTER 1): " choice
    choice=${choice:-1}
    case "$choice" in
      1) PLATFORM_CHOICE=1; return 0 ;;
      2) PLATFORM_CHOICE=2; return 0 ;;
      3) PLATFORM_CHOICE=3; return 0 ;;
      *) echo "$MSG_INVALID_OPTION 1, 2, or 3" ;;
    esac
  done
}

# Execute platform selection.
PLATFORM_CHOICE=1
select_build_platform

if [[ $PLATFORM_CHOICE -eq 2 || $PLATFORM_CHOICE -eq 3 ]]; then
  # Linux 构建
  echo ""
  echo "========== $MSG_LINUX_MODE =========="
  if [[ $PLATFORM_CHOICE -eq 3 ]]; then
    BUILD_TARGET="armv7-unknown-linux-musleabihf"
  fi

  # 检测当前系统
  CURRENT_OS="$(uname -s)"
  if [[ "$CURRENT_OS" == "Linux" ]]; then
    # 在 Linux 上，直接用原生构建
    if [[ $PLATFORM_CHOICE -eq 2 ]]; then
      BUILD_TARGET="x86_64-unknown-linux-gnu"
    fi
    echo "  $MSG_DETECTED_LINUX"
  elif [[ "$CURRENT_OS" == "Darwin" ]]; then
    # On macOS, cross-compile to Linux.
    echo "  $MSG_DETECTED_MACOS"
    if [[ $PLATFORM_CHOICE -eq 2 ]]; then
      BUILD_TARGET="x86_64-unknown-linux-musl"
      LOCAL_LINKER_CMD="x86_64-linux-musl-gcc"
    else
      BUILD_TARGET="armv7-unknown-linux-musleabihf"
      LOCAL_LINKER_CMD="arm-linux-musleabihf-gcc"
    fi

    HAS_DOCKER=0
    command -v docker &>/dev/null && HAS_DOCKER=1
    HAS_LOCAL_LINKER=0
    command -v "$LOCAL_LINKER_CMD" &>/dev/null && HAS_LOCAL_LINKER=1

    case "$BUILD_METHOD" in
      docker)
        [[ $HAS_DOCKER -eq 1 ]] || {
          echo "$MSG_ERROR_NO_DOCKER"
          echo "$MSG_INSTALL_DOCKER: https://www.docker.com/products/docker-desktop"
          exit 1
        }
        USE_DOCKER=1
        ;;
      local)
        USE_DOCKER=""
        ;;
      auto)
        # Novice-friendly default: if Docker exists, use Docker first.
        if [[ $HAS_DOCKER -eq 1 ]]; then
          USE_DOCKER=1
          echo "  Auto-selected Docker build (detected Docker)."
        else
          USE_DOCKER=""
          echo "  Auto-selected local musl-cross build (Docker not found)."
        fi
        ;;
      *)
        echo "Error: BUILD_METHOD must be one of: auto, docker, local" >&2
        exit 1
        ;;
    esac
  else
    echo "$MSG_UNKNOWN_OS $CURRENT_OS, $MSG_TRY_NATIVE"
    BUILD_TARGET="x86_64-unknown-linux-gnu"
  fi

  BUILD_FEATURES=""
  SKIP_ESP_TOOLCHAIN=1
else
  # ESP32 构建
  echo ""
  echo "========== $MSG_ESP_MODE =========="

  # --- BOARD => target/features from board_presets.toml ---
  BUILD_TARGET="xtensa-esp32s3-espidf"
  BUILD_FEATURES=""
fi
if [[ -n "${BOARD:-}" ]]; then
  if [[ ! "$BOARD" =~ ^[a-z0-9-]+$ ]]; then
    echo "Error: BOARD must contain only [a-z0-9-]. Got: $BOARD" >&2
    exit 1
  fi
  PRESETS_FILE="$SCRIPT_ROOT/board_presets.toml"
  if [[ ! -f "$PRESETS_FILE" ]]; then
    echo "Error: BOARD=$BOARD set but board_presets.toml not found" >&2
    exit 1
  fi
  section="[boards.$BOARD]"
  block=$(awk -v section="$section" '
    $0 == section { found=1; next }
    found { print }
    /^\[/ && found { exit }
  ' "$PRESETS_FILE")
  if [[ -z "$block" ]]; then
    echo "Error: Unknown board: $BOARD" >&2
    echo "Known boards: $(grep -E '^\[boards\.' "$PRESETS_FILE" 2>/dev/null | sed 's/\[boards\.\(.*\)\]/\1/' | tr '\n' ' ')" >&2
    exit 1
  fi
  BUILD_TARGET=$(echo "$block" | grep -E '^target\s*=' | head -1 | sed 's/.*"\([^"]*\)".*/\1/')
  [[ -z "$BUILD_TARGET" ]] && { echo "Error: board $BOARD has no 'target' in board_presets.toml" >&2; exit 1; }
  PARTITION_TABLE=$(echo "$block" | grep -E '^partition_table\s*=' | head -1 | sed 's/.*"\([^"]*\)".*/\1/')
  if [[ -z "$PARTITION_TABLE" ]]; then
    case "$BOARD" in
      esp32-s3-8mb)  PARTITION_TABLE=partitions_8mb.csv ;;
      esp32-s3-32mb) PARTITION_TABLE=partitions_32mb.csv ;;
      *)             PARTITION_TABLE=partitions.csv ;;
    esac
  fi
else
  PARTITION_TABLE=partitions.csv
fi
# Command-line --target overrides BOARD (same as build.ps1)
for (( i=0; i < ${#BUILD_ARGS[@]}; i++ )); do
  if [[ "${BUILD_ARGS[$i]}" == "--target" ]] && (( i+1 < ${#BUILD_ARGS[@]} )); then
    BUILD_TARGET="${BUILD_ARGS[$i+1]}"
    break
  fi
done
# Sanitize target (no path chars)
if [[ ! "$BUILD_TARGET" =~ ^[a-zA-Z0-9_-]+$ ]]; then
  echo "Error: Invalid --target (no path chars): $BUILD_TARGET" >&2
  exit 1
fi

# Derive chip from target for flash (same as build.ps1)
FLASH_CHIP=""
if [[ "$BUILD_TARGET" =~ (esp32[a-z0-9]+) ]]; then
  FLASH_CHIP="${BASH_REMATCH[1]}"
fi

# Print detected hardware / build config (same as build.ps1 Write-BuildStatus)
echo ""
echo "========== Detected hardware / build config =========="
echo "  Project root:      $SCRIPT_ROOT"
echo "  Build target:      $BUILD_TARGET"
echo "  BOARD (optional):  ${BOARD:-(not set)}"
echo "  Partition table:   $PARTITION_TABLE"
echo "  Chip (for flash):  ${FLASH_CHIP:-(N/A)}"
echo "  Features:          ${BUILD_FEATURES:-(none)}"
echo ""

# --- clean: cargo clean then exit (no short-path on Mac/Linux) ---
if printf '%s\n' "${BUILD_ARGS[@]}" | grep -qx "clean"; then
  echo "========== Step: Cleaning build artifacts =========="
  echo "  Running: cargo clean (project root)..."
  CLEAN_ARGS=()
  for a in "${BUILD_ARGS[@]}"; do [[ "$a" != "clean" ]] && CLEAN_ARGS+=("$a"); done
  cargo clean "${CLEAN_ARGS[@]}"
  exit $?
fi

# effectiveTargetDir (same as build.ps1)
EFFECTIVE_TARGET_DIR="${CARGO_TARGET_DIR:-$SCRIPT_ROOT/target}"
RELEASE_DIR="$EFFECTIVE_TARGET_DIR/$BUILD_TARGET/release"
BIN="$RELEASE_DIR/beetle"
BOOTLOADER_BIN="$RELEASE_DIR/bootloader.bin"
PARTITION_TABLE_BIN="$RELEASE_DIR/partition-table.bin"
PARTITION_CSV="$SCRIPT_ROOT/$PARTITION_TABLE"
if [[ -n "$DO_FLASH" ]] && [[ -z "$FLASH_CHIP" ]]; then
  echo "Error: Cannot derive chip from target for flash: $BUILD_TARGET" >&2
  exit 1
fi

# --- Flash mode: numbered menu like deploy-linux.sh "Deployment Mode" (sets ERASE_BEFORE_FLASH; may exit) ---
# FLASH_NO_ERASE=1 (--flash-update): skip menu, never erase.
select_flash_mode() {
  local port="$1" triple="$2"
  ERASE_BEFORE_FLASH=0
  if [[ -n "$FLASH_NO_ERASE" ]]; then
    echo -e "${GREEN}✓ Flash mode: update only — entire flash will NOT be erased (NVS / config preserved).${NC}"
    echo ""
    return 0
  fi
  echo "========== Flash mode =========="
  echo ""
  echo "  1) Update flash — keep NVS, WiFi credentials, SPIFFS (typical dev / OTA-style)"
  echo "  2) Full chip erase then flash — wipes entire flash (factory reset / partition change)"
  echo "  3) Cancel"
  echo ""
  while true; do
    read -r -p "Select [1-3] (default 1): " flash_choice
    flash_choice=${flash_choice:-1}
    case "$flash_choice" in
      1)
        ERASE_BEFORE_FLASH=0
        echo -e "${GREEN}✓ Update flash: no full erase.${NC}"
        echo ""
        return 0
        ;;
      2)
        echo -e "${YELLOW}⚠ Entire flash will be erased on ${port}; firmware target: ${triple}${NC}"
        read -r -p "Type 'yes' to confirm full erase and flash: " confirm
        if [[ "$confirm" != "yes" ]]; then
          echo "Aborted."
          exit 0
        fi
        ERASE_BEFORE_FLASH=1
        echo ""
        return 0
        ;;
      3)
        echo "Cancelled."
        exit 0
        ;;
      *)
        echo -e "${YELLOW}Invalid option — enter 1, 2, or 3${NC}"
        ;;
    esac
  done
}

# Port validation: /dev/ path only (Mac/Linux)
valid_flash_port() {
  [[ -n "$1" ]] && [[ "$1" =~ ^/dev/[a-zA-Z0-9/_.-]+$ ]] && [[ "$1" != *".."* ]]
}
# Ensure espflash installed (same as build.ps1 Ensure-Espflash)
ensure_espflash() {
  if command -v espflash &>/dev/null; then return; fi
  echo ""
  echo "========== Step: Ensuring espflash is installed =========="
  echo "  espflash not found. Running: cargo install espflash"
  RUSTUP_TOOLCHAIN=stable cargo install espflash
  export PATH="${HOME}/.cargo/bin:${PATH}"
  command -v espflash &>/dev/null || { echo "Error: espflash install failed." >&2; exit 1; }
}
# Interactive port selection when ESPFLASH_PORT not set (same as build.ps1 Get-FlashPort)
# Only the chosen port is printed to stdout; messages go to stderr.
get_flash_port() {
  if [[ -n "${ESPFLASH_PORT:-}" ]]; then
    valid_flash_port "$ESPFLASH_PORT" || { echo "Error: ESPFLASH_PORT must be a valid device path (e.g. /dev/ttyUSB0). Got: $ESPFLASH_PORT" >&2; exit 1; }
    echo "$ESPFLASH_PORT"
    return
  fi
  PORTS=()
  if [[ "$(uname -s)" = "Linux" ]]; then
    for f in /dev/ttyUSB* /dev/ttyACM*; do [[ -e "$f" ]] && PORTS+=("$f"); done
  else
    for f in /dev/cu.usbmodem* /dev/cu.usbserial* /dev/cu.SLAB* /dev/cu.wchusbserial* /dev/cu.UART*; do [[ -e "$f" ]] && PORTS+=("$f"); done
  fi
  if [[ ${#PORTS[@]} -eq 0 ]]; then
    echo "No serial ports found. Plug in the board or set ESPFLASH_PORT=/dev/..." >&2
    exit 1
  fi
  if [[ ${#PORTS[@]} -eq 1 ]]; then
    echo "  Detected 1 serial port: ${PORTS[0]}" >&2
    echo "${PORTS[0]}"
    return
  fi
  echo "  Detected ${#PORTS[@]} serial ports. Select port to flash (ESP board):" >&2
  for i in "${!PORTS[@]}"; do echo "  $((i+1)). ${PORTS[i]}" >&2; done
  while true; do
    read -r -p "Enter number (1-${#PORTS[@]}): " sel
    if [[ "$sel" =~ ^[0-9]+$ ]] && (( sel >= 1 && sel <= ${#PORTS[@]} )); then
      echo "${PORTS[$((sel-1))]}"
      return
    fi
    echo "Invalid, enter 1-${#PORTS[@]}" >&2
  done
}

# --- Linux 构建：跳过 ESP 工具链 ---
if [[ "$BUILD_TARGET" =~ -unknown-linux ]]; then
  echo ""
  echo "========== $MSG_LINUX_MODE =========="
  echo "  Target: $BUILD_TARGET"

  # 检查是否在 macOS 上构建 Linux musl
  if [[ "$BUILD_TARGET" =~ -unknown-linux-musl ]] && [[ "$(uname -s)" == "Darwin" ]]; then
    echo "  $MSG_DETECTED_MACOS"

    # 使用 Docker
    if [[ -n "${USE_DOCKER:-}" ]]; then
      run_linux_docker_build "$BUILD_TARGET"

      echo ""
      echo "========== $MSG_BUILD_COMPLETE =========="
      echo "  $MSG_BINARY: $BIN"
      ls -lh "$BIN" 2>/dev/null || echo "  (check target/$BUILD_TARGET/release/beetle)"
      exit 0
    fi

    # Check and install local musl-cross toolchain when needed.
    if [[ "$BUILD_TARGET" == "x86_64-unknown-linux-musl" ]] && ! command -v x86_64-linux-musl-gcc &>/dev/null; then
      echo ""
      echo "========== Installing musl-cross toolchain =========="
      echo "  x86_64-linux-musl-gcc not found."
      if command -v brew &>/dev/null; then
        echo "  Installing via Homebrew..."
        brew install filosottile/musl-cross/musl-cross
      else
        echo "Error: Neither Docker nor musl-cross found." >&2
        echo "Install one of:" >&2
        echo "  - Docker: https://www.docker.com/products/docker-desktop" >&2
        echo "  - musl-cross: brew install filosottile/musl-cross/musl-cross" >&2
        exit 1
      fi
    fi
    if [[ "$BUILD_TARGET" == "armv7-unknown-linux-musleabihf" ]] && ! command -v arm-linux-musleabihf-gcc &>/dev/null; then
      echo ""
      echo "========== Installing musl-cross toolchain =========="
      echo "  arm-linux-musleabihf-gcc not found."
      if command -v brew &>/dev/null; then
        echo "  Installing via Homebrew..."
        brew install filosottile/musl-cross/musl-cross
      else
        echo "Error: Neither Docker nor musl-cross found." >&2
        echo "Install one of:" >&2
        echo "  - Docker: https://www.docker.com/products/docker-desktop" >&2
        echo "  - musl-cross: brew install filosottile/musl-cross/musl-cross" >&2
        exit 1
      fi
    fi

    if [[ "$BUILD_TARGET" == "x86_64-unknown-linux-musl" ]] && ! command -v x86_64-linux-musl-gcc &>/dev/null; then
      echo "Error: x86_64-linux-musl-gcc is still not available after installation." >&2
      echo "Hint: restart your shell and verify with: x86_64-linux-musl-gcc --version" >&2
      echo "Or choose Docker build mode to avoid local linker setup." >&2
      exit 1
    fi
    if [[ "$BUILD_TARGET" == "armv7-unknown-linux-musleabihf" ]] && ! command -v arm-linux-musleabihf-gcc &>/dev/null; then
      echo "Error: arm-linux-musleabihf-gcc is still not available after installation." >&2
      echo "Hint: restart your shell and verify with: arm-linux-musleabihf-gcc --version" >&2
      echo "Or choose Docker build mode to avoid local linker setup." >&2
      exit 1
    fi

    # 配置 musl 链接器（x86_64 本地模式）。
    if [[ "$BUILD_TARGET" == "x86_64-unknown-linux-musl" ]]; then
      mkdir -p .cargo
      if ! grep -q "x86_64-unknown-linux-musl" .cargo/config.toml 2>/dev/null; then
        cat >> .cargo/config.toml << 'EOF'

[target.x86_64-unknown-linux-musl]
linker = "x86_64-linux-musl-gcc"
EOF
        echo "  Configured musl linker in .cargo/config.toml"
      fi

      if ! grep -q 'linker = "x86_64-linux-musl-gcc"' .cargo/config.toml 2>/dev/null; then
        cat >> .cargo/config.toml << 'EOF'
linker = "x86_64-linux-musl-gcc"
EOF
        echo "  Added linker for musl target in .cargo/config.toml"
      fi
    fi
    if [[ "$BUILD_TARGET" == "armv7-unknown-linux-musleabihf" ]]; then
      mkdir -p .cargo
      if ! grep -q "armv7-unknown-linux-musleabihf" .cargo/config.toml 2>/dev/null; then
        cat >> .cargo/config.toml << 'EOF'

[target.armv7-unknown-linux-musleabihf]
linker = "arm-linux-musleabihf-gcc"
EOF
        echo "  Configured armv7 musl linker in .cargo/config.toml"
      fi
      if ! grep -q 'linker = "arm-linux-musleabihf-gcc"' .cargo/config.toml 2>/dev/null; then
        cat >> .cargo/config.toml << 'EOF'
linker = "arm-linux-musleabihf-gcc"
EOF
        echo "  Added linker for armv7 musl target in .cargo/config.toml"
      fi
      # Also export linker env to avoid any stale/global Cargo config precedence issues.
      export CARGO_TARGET_ARMV7_UNKNOWN_LINUX_MUSLEABIHF_LINKER="arm-linux-musleabihf-gcc"
    fi
  fi

  # 添加 target
  if ! rustup +stable target list --installed | grep -q "$BUILD_TARGET"; then
    echo "  Adding target: $BUILD_TARGET"
    rustup +stable target add "$BUILD_TARGET"
  fi

  # 跳过 ESP 工具链检查
  SKIP_ESP_TOOLCHAIN=1
else
  # --- ESP toolchain PATH (platform-specific, same role as build.ps1 Set-EspPath) ---
  set_esp_path() {
    for f in "$HOME/export-esp.sh" "$HOME/.espup/export-esp.sh" "$HOME/.local/share/esp-rs/export-esp.sh"; do
      [[ -f "$f" ]] && { source "$f"; return; }
    done
    for d in "$HOME/.rustup/toolchains/esp/xtensa-esp-elf/"*/xtensa-esp-elf/bin; do
      [[ -x "${d}/xtensa-esp32s3-elf-gcc" ]] 2>/dev/null && { export PATH="$d:$PATH"; return; }
    done
  }
  set_esp_path

  # --- Install ESP toolchain if missing (same as build.ps1) ---
  if ! command -v xtensa-esp32s3-elf-gcc &>/dev/null && ! command -v riscv32-esp-elf-gcc &>/dev/null; then
    echo ""
    echo "========== Step: Installing ESP Rust toolchain (espup) =========="
    echo "  xtensa-esp32s3-elf-gcc not found. Running espup install."
    if ! command -v espup &>/dev/null; then
      echo ">>> Installing espup (using stable)..."
      RUSTUP_TOOLCHAIN=stable cargo install espup
      export PATH="${HOME}/.cargo/bin:${PATH}"
    fi
    espup install
    set_esp_path
    command -v xtensa-esp32s3-elf-gcc &>/dev/null || command -v riscv32-esp-elf-gcc &>/dev/null || {
      echo "Error: xtensa-esp32s3-elf-gcc still not found after espup install" >&2
      exit 1
    }
  fi

  # --- Install ldproxy if missing (same as build.ps1; no Windows prebuilt on Mac/Linux) ---
  if ! command -v ldproxy &>/dev/null; then
    echo ""
    echo "========== Step: Installing ldproxy (linker wrapper) =========="
    echo ">>> Installing ldproxy (using stable)..."
    RUSTUP_TOOLCHAIN=stable cargo install ldproxy
    export PATH="${HOME}/.cargo/bin:${PATH}"
  fi
fi

# --- Write sdkconfig board overlay (仅 ESP 目标) ---
if [[ -z "${SKIP_ESP_TOOLCHAIN:-}" ]]; then
  BOARD_SDKCONFIG="$SCRIPT_ROOT/sdkconfig.defaults.esp32s3.board"
  # 与 sdkconfig.defaults.esp32s3 一致：CMake 工程根为 esp-idf-sys 的 out/，分区表路径需相对 out/ 指向仓库根 CSV。
  case "$PARTITION_TABLE" in
    partitions_8mb.csv)
      printf '%s\n' 'CONFIG_ESPTOOLPY_FLASHSIZE_8MB=y' '# CONFIG_ESPTOOLPY_FLASHSIZE_16MB is not set' 'CONFIG_PARTITION_TABLE_CUSTOM_FILENAME="../../../../../../partitions_8mb.csv"' > "$BOARD_SDKCONFIG"
      ;;
    partitions_32mb.csv)
      printf '%s\n' 'CONFIG_ESPTOOLPY_FLASHSIZE_32MB=y' '# CONFIG_ESPTOOLPY_FLASHSIZE_16MB is not set' 'CONFIG_PARTITION_TABLE_CUSTOM_FILENAME="../../../../../../partitions_32mb.csv"' > "$BOARD_SDKCONFIG"
      ;;
    *)
      printf '%s\n' 'CONFIG_ESPTOOLPY_FLASHSIZE_16MB=y' 'CONFIG_PARTITION_TABLE_CUSTOM_FILENAME="../../../../../../partitions.csv"' > "$BOARD_SDKCONFIG"
      ;;
  esac
fi

# --- Build args: inject default ESP --target when missing (same as build.ps1); no longer rely on .cargo [build] target ---
RELEASE_ARGS=()
HAS_TARGET=0
for a in "${BUILD_ARGS[@]}"; do [[ "$a" == "--target" ]] && HAS_TARGET=1; done
[[ $HAS_TARGET -eq 0 ]] && RELEASE_ARGS+=(--target "$BUILD_TARGET")
[[ -n "$BUILD_FEATURES" ]] && RELEASE_ARGS+=($BUILD_FEATURES)
RELEASE_ARGS+=("${BUILD_ARGS[@]}")

# --- Build (same as build.ps1) ---
echo ""
echo "========== Step: Building release =========="
echo "  Target: $BUILD_TARGET  |  Root: $SCRIPT_ROOT"

# Linux 构建用 stable 工具链
if [[ "$BUILD_TARGET" =~ -unknown-linux ]]; then
  if ! cargo +stable build --release "${RELEASE_ARGS[@]}"; then
    # Auto fallback: local musl build failed on macOS, retry with Docker if available.
    if [[ "$(uname -s)" == "Darwin" ]] && [[ "$BUILD_TARGET" =~ -unknown-linux-musl ]] && [[ -z "${USE_DOCKER:-}" ]] && command -v docker &>/dev/null; then
      echo ""
      echo "Local toolchain build failed. Auto-fallback to Docker build..."
      run_linux_docker_build "$BUILD_TARGET"
      echo ""
      echo "========== $MSG_BUILD_COMPLETE =========="
      echo "  $MSG_BINARY: $BIN"
      ls -lh "$BIN" 2>/dev/null || echo "  (check target/$BUILD_TARGET/release/beetle)"
      exit 0
    fi

    echo "" >&2
    echo "Build failed for target: $BUILD_TARGET" >&2
    if [[ "$BUILD_TARGET" == "x86_64-unknown-linux-musl" ]] && [[ "$(uname -s)" == "Darwin" ]]; then
      echo "Common fixes on macOS:" >&2
      echo "  1) Ensure target is installed on stable toolchain:" >&2
      echo "     rustup +stable target add x86_64-unknown-linux-musl" >&2
      echo "  2) Ensure musl linker is available:" >&2
      echo "     x86_64-linux-musl-gcc --version" >&2
      echo "  3) Ensure .cargo/config.toml contains:" >&2
      echo "     [target.x86_64-unknown-linux-musl]" >&2
      echo "     linker = \"x86_64-linux-musl-gcc\"" >&2
      echo "  4) If your default toolchain is esp, always use +stable for rustup target commands." >&2
      echo "  5) Prefer Docker mode if linker errors persist (recommended)." >&2
    elif [[ "$BUILD_TARGET" == "armv7-unknown-linux-musleabihf" ]] && [[ "$(uname -s)" == "Darwin" ]]; then
      echo "Common fixes on macOS (armv7):" >&2
      echo "  1) Prefer Docker mode for armv7 (recommended)." >&2
      echo "  2) Ensure target is installed on stable toolchain:" >&2
      echo "     rustup +stable target add armv7-unknown-linux-musleabihf" >&2
    fi
    exit 1
  fi
else
  cargo build --release "${RELEASE_ARGS[@]}"
fi

# --- Flash path (only when --flash) ---
if [[ -z "$DO_FLASH" ]]; then
  echo "  Build done. Use ./build.sh --flash or --flash-update to flash."
  exit 0
fi

if [[ ! -f "$BIN" ]]; then
  echo "Error: Binary not found: $BIN" >&2
  exit 1
fi
# Prefer built bootloader/partition-table (same as build.ps1)
FLASH_EXTRA=()
PARTITION_FOR_FLASH="$PARTITION_CSV"
if [[ -f "$BOOTLOADER_BIN" ]]; then
  [[ -f "$PARTITION_TABLE_BIN" ]] && PARTITION_FOR_FLASH="$PARTITION_TABLE_BIN"
  if [[ -f "$PARTITION_FOR_FLASH" ]]; then
    FLASH_EXTRA=(--bootloader "$BOOTLOADER_BIN" --partition-table "$PARTITION_FOR_FLASH")
  fi
fi

ensure_espflash
CHOSEN_PORT=$(get_flash_port)
echo ""
echo "=========================================="
echo "  Beetle — Flash to device"
echo "=========================================="
echo ""
echo "========== Flash: hardware and paths =========="
echo ""
echo "  Project root:      $SCRIPT_ROOT"
echo "  Build target:      $BUILD_TARGET"
echo "  BOARD (optional):  ${BOARD:-(not set)}"
echo "  Chip (for flash):  ${FLASH_CHIP:-(N/A)}"
echo "  Features:          ${BUILD_FEATURES:-(none)}"
echo -e "  ${BLUE}Serial port:${NC}       $CHOSEN_PORT"
echo "  Partition table:   $PARTITION_FOR_FLASH"
echo "  Bootloader:        $BOOTLOADER_BIN"
echo "  Firmware binary:   $BIN"
echo ""

# Pre-flash connection check: try board-info; on failure print diagnostic hints.
echo "========== Checking connection =========="
echo ""
if espflash board-info --port "$CHOSEN_PORT" --chip "$FLASH_CHIP" 2>/dev/null; then
  echo -e "${GREEN}✓ board-info OK${NC}"
else
  echo -e "${YELLOW}⚠ Could not read board-info from $CHOSEN_PORT (connection or chip mismatch).${NC}"
  echo "  Proceeding anyway; if flash fails, try:"
  echo "    1) Replug USB or set ESPFLASH_PORT=… (see ./build.sh --help)"
  echo "    2) Download mode: hold BOOT, tap RESET, run flash within a few seconds"
  echo "    3) macOS: ensure another app is not using the serial port"
fi
echo ""

select_flash_mode "$CHOSEN_PORT" "$BUILD_TARGET"

if [[ "$ERASE_BEFORE_FLASH" -eq 1 ]]; then
  echo "========== Erasing entire flash =========="
  echo ""
  echo "  Port: $CHOSEN_PORT  |  Chip: $FLASH_CHIP"
  if ! espflash erase-flash --port "$CHOSEN_PORT" --chip "$FLASH_CHIP"; then
    echo "" >&2
    echo -e "${RED}Erase failed.${NC} Common causes:" >&2
    echo "  - Port in use or disconnected: unplug and replug; set ESPFLASH_PORT=/dev/cu.xxx if multiple ports" >&2
    echo "  - Board not in download mode: hold BOOT, tap RESET, run this command again within a few seconds" >&2
    echo "  - Wrong chip: build target is $BUILD_TARGET (chip $FLASH_CHIP); use the matching board" >&2
    exit 1
  fi
  echo -e "${GREEN}✓ Erase completed. Waiting 2s before flash.${NC}"
  sleep 2
else
  echo "========== Skipping full erase (update flash) =========="
  echo ""
  echo -e "  ${GREEN}✓ NVS and other flash regions are left unchanged.${NC}"
  echo "  Port: $CHOSEN_PORT  |  Chip: $FLASH_CHIP"
fi

echo ""
echo "========== Flashing firmware =========="
echo ""
echo "  Binary: $BIN"
echo "  Partition table: $PARTITION_FOR_FLASH"
if [[ -n "$NO_MONITOR" ]]; then
  if ! espflash flash --port "$CHOSEN_PORT" --chip "$FLASH_CHIP" "${FLASH_EXTRA[@]}" "$BIN"; then
    echo "Flash failed. Check port, download mode, and chip." >&2
    exit 1
  fi
  echo ""
  echo -e "${GREEN}✓ Flash complete.${NC}"
  echo ""
else
  if ! espflash flash --port "$CHOSEN_PORT" --chip "$FLASH_CHIP" "${FLASH_EXTRA[@]}" --monitor "$BIN"; then
    echo "Flash failed. Check port, download mode, and chip." >&2
    exit 1
  fi
fi
