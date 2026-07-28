#!/usr/bin/env bash
# 预下载 Lindera 嵌入词典到 .lindera-cache（供 cargo build / Docker 使用）
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VER="${LINDERA_VERSION:-1.5.1}"
CACHE="${LINDERA_CACHE:-$ROOT/.lindera-cache}/$VER"
mkdir -p "$CACHE"

IPADIC_URL="${LINDERA_IPADIC_URL:-https://Lindera.dev/mecab-ipadic-2.7.0-20250920.tar.gz}"
KODIC_URL="${LINDERA_KODIC_URL:-https://Lindera.dev/mecab-ko-dic-2.1.1-20180720.tar.gz}"
IPADIC_MD5=a95c409f12f1023fce8ef91f991ef042
KODIC_MD5=b996764e91c96bc89dc32ea208514a96

download() {
  local url="$1" out="$2" expect="$3"
  if [[ -f "$out" ]]; then
    local actual
    if command -v md5sum >/dev/null; then
      actual=$(md5sum "$out" | awk '{print $1}')
    else
      actual=$(md5 -q "$out")
    fi
    if [[ "$actual" == "$expect" ]]; then
      echo "ok: $(basename "$out")"
      return 0
    fi
    echo "md5 mismatch, re-downloading $(basename "$out")"
    rm -f "$out"
  fi
  echo "downloading $(basename "$out")…"
  curl --http1.1 -L --retry 10 --retry-all-errors --retry-delay 3 \
    --connect-timeout 30 --max-time 900 \
    -C - -o "$out" "$url"
  local actual
  if command -v md5sum >/dev/null; then
    actual=$(md5sum "$out" | awk '{print $1}')
  else
    actual=$(md5 -q "$out")
  fi
  if [[ "$actual" != "$expect" ]]; then
    echo "ERROR: md5 for $(basename "$out"): got $actual want $expect" >&2
    exit 1
  fi
  echo "ok: $(basename "$out")"
}

download "$IPADIC_URL" "$CACHE/mecab-ipadic-2.7.0-20250920.tar.gz" "$IPADIC_MD5"
download "$KODIC_URL" "$CACHE/mecab-ko-dic-2.1.1-20180720.tar.gz" "$KODIC_MD5"
echo "LINDERA_CACHE=${LINDERA_CACHE:-$ROOT/.lindera-cache}"
