#!/usr/bin/env bash
# Download integration-test assets, verify host tools, and run the full workspace
# test suite including ignored infra tests (OCI stack, QEMU VM stack, k3s+Helm stack).
#
# Usage:
#   ./scripts/run-infra-tests.sh              # setup + all tests
#   ./scripts/run-infra-tests.sh --setup-only
#   ./scripts/run-infra-tests.sh --test-only
#   ./scripts/run-infra-tests.sh --install-deps   # try to install missing host packages (Linux)
#
# Environment (optional):
#   INFRZEUG_CACHE_DIR          cache root (default: ~/.cache/infrazeug)
#   INFRZEUG_DEBIAN_CLOUD_IMAGE override qcow2 path
#   INFRZEUG_K3S_STACK_MEM_MB   k3s guest RAM (default: 8192)
#   INFRZEUG_VM_STACK_MEM_MB    per-VM RAM for 4-VM stack (default: 768)
#   CARGO_TEST_FLAGS            extra args to cargo test (e.g. -j 4)

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CACHE_DIR="${INFRZEUG_CACHE_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/infrazeug}"
INSTALL_DEPS=0
MODE=both

OCI_IMAGES=(
  docker.io/library/postgres:16-alpine
  quay.io/keycloak/keycloak:26.0
  ghcr.io/open-webui/open-webui:main
  docker.io/rustfs/rustfs:latest
)

usage() {
  sed -n '2,14p' "$0" | sed 's/^# \{0,1\}//'
  echo
  echo "Options:"
  echo "  --setup-only     download images and check tools; do not run cargo test"
  echo "  --test-only      run tests only (assumes setup already done)"
  echo "  --install-deps   attempt to install qemu/iso tooling/container runtime (Linux)"
  echo "  -h, --help       show this help"
}

log() { printf '==> %s\n' "$*"; }
warn() { printf 'warning: %s\n' "$*" >&2; }
die() { printf 'error: %s\n' "$*" >&2; exit 1; }

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || die "missing command: $1 (see --install-deps or your package manager)"
}

detect_arch() {
  case "$(uname -m)" in
    x86_64|amd64) echo amd64 ;;
    aarch64|arm64) echo arm64 ;;
    *) die "unsupported architecture: $(uname -m)" ;;
  esac
}

debian_image_url() {
  local arch="$1"
  case "$arch" in
    amd64) echo "https://cdimage.debian.org/cdimage/cloud/bookworm/latest/debian-12-generic-amd64.qcow2" ;;
    arm64) echo "https://cdimage.debian.org/cdimage/cloud/bookworm/latest/debian-12-generic-arm64.qcow2" ;;
  esac
}

default_debian_image_path() {
  local arch="$1"
  echo "${CACHE_DIR}/debian-12-generic-${arch}.qcow2"
}

iso_tool() {
  if command -v genisoimage >/dev/null 2>&1; then
    echo genisoimage
  elif command -v mkisofs >/dev/null 2>&1; then
    echo mkisofs
  else
    return 1
  fi
}

qemu_system_bin() {
  local arch="$1"
  case "$arch" in
    amd64)
      if command -v qemu-system-x86_64 >/dev/null 2>&1; then
        echo qemu-system-x86_64
        return 0
      fi
      ;;
    arm64)
      if command -v qemu-system-aarch64 >/dev/null 2>&1; then
        echo qemu-system-aarch64
        return 0
      fi
      ;;
  esac
  return 1
}

container_runtime() {
  if [[ -n "${INFRZEUG_CONTAINER_RUNTIME:-}" ]] && command -v "${INFRZEUG_CONTAINER_RUNTIME}" >/dev/null 2>&1; then
    echo "${INFRZEUG_CONTAINER_RUNTIME}"
    return 0
  fi
  if [[ -n "${INFRZEUG_PODMAN:-}" ]] && command -v "${INFRZEUG_PODMAN}" >/dev/null 2>&1; then
    echo "${INFRZEUG_PODMAN}"
    return 0
  fi
  if command -v podman >/dev/null 2>&1; then
    echo podman
    return 0
  fi
  if [[ -n "${INFRZEUG_DOCKER:-}" ]] && command -v "${INFRZEUG_DOCKER}" >/dev/null 2>&1; then
    echo "${INFRZEUG_DOCKER}"
    return 0
  fi
  if command -v docker >/dev/null 2>&1; then
    echo docker
    return 0
  fi
  return 1
}

install_deps_linux() {
  if [[ "$(uname -s)" != Linux ]]; then
    die "--install-deps is only supported on Linux"
  fi
  if command -v pacman >/dev/null 2>&1; then
    log "installing packages via pacman (sudo)"
    sudo pacman -S --needed --noconfirm \
      qemu-system-x86_64 qemu-system-aarch64 qemu-img cdrtools openssh podman curl
    return 0
  fi
  if command -v apt-get >/dev/null 2>&1; then
    log "installing packages via apt (sudo)"
    sudo apt-get update -qq
    sudo DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
      qemu-system-x86 qemu-system-arm qemu-utils genisoimage openssh-client podman curl ca-certificates
    return 0
  fi
  if command -v dnf >/dev/null 2>&1; then
    log "installing packages via dnf (sudo)"
    sudo dnf install -y \
      qemu-kvm qemu-img genisoimage openssh-clients podman curl
    return 0
  fi
  die "no supported package manager found for --install-deps"
}

download_debian_cloud_image() {
  local arch="$1"
  local dest="${INFRZEUG_DEBIAN_CLOUD_IMAGE:-$(default_debian_image_path "$arch")}"
  local url
  url="$(debian_image_url "$arch")"

  mkdir -p "$(dirname "$dest")"
  if [[ -f "$dest" ]]; then
    log "debian cloud image present: $dest"
    export INFRZEUG_DEBIAN_CLOUD_IMAGE="$dest"
    return 0
  fi

  log "downloading debian 12 cloud image ($arch)"
  log "  url:  $url"
  log "  dest: $dest"
  need_cmd curl
  curl -fL --retry 3 --continue-at - -o "$dest" "$url"
  export INFRZEUG_DEBIAN_CLOUD_IMAGE="$dest"
}

pull_oci_images() {
  local rt
  if ! rt="$(container_runtime)"; then
    warn "no podman/docker found; OCI stack test will fail unless runtime is installed"
    return 0
  fi
  log "pre-pulling OCI images with $rt"
  local img
  for img in "${OCI_IMAGES[@]}"; do
    log "  pull $img"
    "$rt" pull "$img"
  done
}

check_host_tools() {
  local arch="$1"
  local missing=0

  need_cmd curl
  need_cmd ssh
  need_cmd ssh-keygen
  need_cmd cargo

  if ! qemu_system_bin "$arch" >/dev/null; then
    warn "missing qemu-system for $arch"
    missing=1
  else
    log "qemu: $(qemu_system_bin "$arch")"
  fi

  if ! command -v qemu-img >/dev/null 2>&1; then
    warn "missing qemu-img"
    missing=1
  else
    log "qemu-img: $(command -v qemu-img)"
  fi

  if ! iso_tool >/dev/null; then
    warn "missing genisoimage or mkisofs (cloud-init seed ISO)"
    missing=1
  else
    log "iso tool: $(iso_tool)"
  fi

  if ! container_runtime >/dev/null; then
    warn "missing podman or docker (OCI example stack)"
    missing=1
  else
    log "container runtime: $(container_runtime)"
  fi

  if [[ "$missing" -eq 1 ]]; then
    if [[ "$INSTALL_DEPS" -eq 1 ]]; then
      install_deps_linux
    else
      die "missing host tools (re-run with --install-deps or install manually)"
    fi
    check_host_tools "$arch"
  fi
}

warn_memory() {
  local kb
  if [[ -r /proc/meminfo ]]; then
    kb="$(awk '/^MemAvailable:/ { print $2 }' /proc/meminfo)"
    if [[ -n "$kb" && "$kb" -lt 12000000 ]]; then
      warn "MemAvailable < 12 GiB — k3s stack test wants ~8 GiB for the guest; close other workloads if runs fail"
    fi
  fi
}

setup() {
  local arch
  arch="$(detect_arch)"
  mkdir -p "$CACHE_DIR"
  log "cache directory: $CACHE_DIR"
  log "host architecture: $arch"

  if [[ "$INSTALL_DEPS" -eq 1 ]]; then
    install_deps_linux
  fi

  check_host_tools "$arch"
  download_debian_cloud_image "$arch"
  pull_oci_images
  warn_memory

  log "setup complete"
  log "  INFRZEUG_DEBIAN_CLOUD_IMAGE=${INFRZEUG_DEBIAN_CLOUD_IMAGE}"
}

run_tests() {
  local arch
  arch="$(detect_arch)"

  export INFRZEUG_DEBIAN_CLOUD_IMAGE="${INFRZEUG_DEBIAN_CLOUD_IMAGE:-$(default_debian_image_path "$arch")}"
  export INFRZEUG_STACK_TEST=1
  export INFRZEUG_VM_STACK_TEST=1
  export INFRZEUG_K3S_STACK_TEST=1

  if [[ ! -f "$INFRZEUG_DEBIAN_CLOUD_IMAGE" ]]; then
    die "debian image missing at $INFRZEUG_DEBIAN_CLOUD_IMAGE (run without --test-only first)"
  fi

  # shellcheck disable=SC2206
  local extra=(${CARGO_TEST_FLAGS:-})

  log "running workspace tests (including ignored infra tests)"
  log "  QEMU stacks need serial execution and ~8+ GiB RAM — using --test-threads=1"
  log "  full run may take 30–60 minutes"

  cd "$ROOT"
  cargo test --workspace -- "${extra[@]}" --include-ignored --nocapture --test-threads=1
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --setup-only) MODE=setup ;;
    --test-only) MODE=test ;;
    --install-deps) INSTALL_DEPS=1 ;;
    -h|--help) usage; exit 0 ;;
    *) die "unknown option: $1 (try --help)" ;;
  esac
  shift
done

case "$MODE" in
  both) setup; run_tests ;;
  setup) setup ;;
  test) run_tests ;;
esac
