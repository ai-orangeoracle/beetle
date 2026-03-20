#!/usr/bin/env bash
# One-shot: env check, install espup/ldproxy/toolchain, then release build.
# Usage: ./build.sh  or  ./build.sh --target xtensa-esp32s3-espidf
#        ./build.sh clean            clean project target dir
#        ./build.sh --flash          build then flash (prompt y/N erase; if no port, scan and select)
#        ESPFLASH_PORT=/dev/ttyUSB0 ./build.sh --flash   skip port selection
# Logic aligned with build.ps1 (Windows); only platform-specific parts differ.
set -e
SCRIPT_ROOT="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_ROOT"
# 固定 target 到本仓库，避免环境/IDE 将 CARGO_TARGET_DIR 指到临时目录导致 esp-idf-sys bindings 与 esp-idf-svc cfg 不一致。
export CARGO_TARGET_DIR="${SCRIPT_ROOT}/target"
export PATH="${HOME}/.cargo/bin:${PATH}"
command -v cargo &>/dev/null || { echo "Error: cargo not found. Install Rust: https://rustup.rs" >&2; exit 1; }

# --- Parse args (same as build.ps1) ---
DO_FLASH=""
NO_MONITOR=""
BUILD_ARGS=()
for arg in "$@"; do
  case "$arg" in
    --flash)      DO_FLASH=1 ;;
    --no-monitor) NO_MONITOR=1 ;;
    *)            BUILD_ARGS+=("$arg") ;;
  esac
done

# --- BOARD => target/features from board_presets.toml ---
BUILD_TARGET="xtensa-esp32s3-espidf"
BUILD_FEATURES=""
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

# --- Confirm erase (same as build.ps1 Confirm-EraseBeforeFlash) ---
confirm_erase_before_flash() {
  local port="$1" triple="$2"
  read -r -p "Erase entire flash before flashing? (y/n): " r
  r=$(echo "$r" | tr '[:upper:]' '[:lower:]')
  if [[ "$r" != "y" && "$r" != "yes" ]]; then
    echo "Skipping flash (no erase)."
    exit 0
  fi
  echo "WARNING: Entire flash will be erased on $port; firmware target: $triple" >&2
  read -r -p "Type 'yes' to confirm erase and flash: " confirm
  if [[ "$confirm" != "yes" ]]; then
    echo "Aborted."
    exit 0
  fi
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

# --- Write sdkconfig board overlay so esp-idf-sys uses correct partition table and flash size ---
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

# --- Build args: if BOARD or --flash, use resolved target/features + buildArgs (same as build.ps1) ---
RELEASE_ARGS=()
if [[ -n "$BOARD" ]] || [[ -n "$DO_FLASH" ]]; then
  HAS_TARGET=0
  for a in "${BUILD_ARGS[@]}"; do [[ "$a" == "--target" ]] && HAS_TARGET=1; done
  [[ $HAS_TARGET -eq 0 ]] && RELEASE_ARGS+=(--target "$BUILD_TARGET")
  [[ -n "$BUILD_FEATURES" ]] && RELEASE_ARGS+=($BUILD_FEATURES)
  RELEASE_ARGS+=("${BUILD_ARGS[@]}")
else
  RELEASE_ARGS=("${BUILD_ARGS[@]}")
fi

# --- Build (same as build.ps1) ---
echo ""
echo "========== Step: Building release =========="
echo "  Target: $BUILD_TARGET  |  Root: $SCRIPT_ROOT"
cargo build --release "${RELEASE_ARGS[@]}"

# --- Flash path (only when --flash) ---
if [[ -z "$DO_FLASH" ]]; then
  echo "  Build done. Use ./build.sh --flash to flash."
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
echo "========== Flash: detected hardware and paths =========="
echo "  Project root:      $SCRIPT_ROOT"
echo "  Build target:      $BUILD_TARGET"
echo "  BOARD (optional):  ${BOARD:-(not set)}"
echo "  Chip (for flash):  ${FLASH_CHIP:-(N/A)}"
echo "  Features:          ${BUILD_FEATURES:-(none)}"
echo "  Serial port:       $CHOSEN_PORT"
echo "  Partition table:  $PARTITION_FOR_FLASH"
echo "  Bootloader:        $BOOTLOADER_BIN"
echo "  Firmware binary:   $BIN"
echo ""

# Pre-flash connection check: try board-info; on failure print diagnostic hints.
echo "========== Step: Checking connection =========="
if ! espflash board-info --port "$CHOSEN_PORT" --chip "$FLASH_CHIP" 2>/dev/null; then
  echo "  Warning: could not read board-info from $CHOSEN_PORT (connection or chip mismatch)." >&2
  echo "  Proceeding anyway; if erase/flash fails, try:" >&2
  echo "    1) Replug USB or use the other port (e.g. CH340: ESPFLASH_PORT=/dev/cu.usbmodem5B... ./build.sh --flash)" >&2
  echo "    2) Put board in download mode: hold BOOT, tap RESET, then run flash within a few seconds" >&2
  echo "    3) On macOS, avoid permission issues: ensure port is not in use by another process" >&2
fi
echo ""

confirm_erase_before_flash "$CHOSEN_PORT" "$BUILD_TARGET"

echo "========== Step: Erasing entire flash =========="
echo "  Port: $CHOSEN_PORT  |  Chip: $FLASH_CHIP"
if ! espflash erase-flash --port "$CHOSEN_PORT" --chip "$FLASH_CHIP"; then
  echo "" >&2
  echo "Erase failed. Common causes:" >&2
  echo "  - Port in use or disconnected: unplug and replug; set ESPFLASH_PORT=/dev/cu.xxx if multiple ports" >&2
  echo "  - Board not in download mode: hold BOOT, tap RESET, run this command again within a few seconds" >&2
  echo "  - Wrong chip: build target is $BUILD_TARGET (chip $FLASH_CHIP); use the matching board" >&2
  exit 1
fi
echo "  Erase completed. Waiting 2s before flash."
sleep 2

echo ""
echo "========== Step: Flashing firmware =========="
echo "  Binary: $BIN  |  Partition table: $PARTITION_FOR_FLASH"
if [[ -n "$NO_MONITOR" ]]; then
  if ! espflash flash --port "$CHOSEN_PORT" --chip "$FLASH_CHIP" "${FLASH_EXTRA[@]}" "$BIN"; then
    echo "Flash failed. See erase-step hints (port, download mode, chip)." >&2
    exit 1
  fi
else
  if ! espflash flash --port "$CHOSEN_PORT" --chip "$FLASH_CHIP" "${FLASH_EXTRA[@]}" --monitor "$BIN"; then
    echo "Flash failed. See erase-step hints (port, download mode, chip)." >&2
    exit 1
  fi
fi
