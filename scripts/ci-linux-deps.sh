#!/usr/bin/env bash
# Apt build dependencies for GPUI on Ubuntu CI runners.
set -euo pipefail

apt-get update
apt-get install -y --no-install-recommends \
  gcc g++ clang libclang-dev libfontconfig-dev \
  libwayland-dev libxkbcommon-dev libxkbcommon-x11-dev libx11-xcb-dev \
  libssl-dev libzstd-dev libvulkan1 vulkan-validationlayers \
  libwebkit2gtk-4.1-dev