#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
sudo apt-get update
sudo apt-get install -y build-essential pkg-config curl ca-certificates cmake \
    libx11-dev libxi-dev libxrandr-dev libxcursor-dev libxinerama-dev \
    libxkbcommon-dev libwayland-dev libgl1-mesa-dev libegl1-mesa-dev \
    libudev-dev libssl-dev
if [[ ! -x "$HOME/.cargo/bin/rustup" ]]; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile default
fi
source "$HOME/.cargo/env"
rustup toolchain install 1.99.0 --profile default --component rust-analyzer
bash scripts/check.sh
