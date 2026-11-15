#!/usr/bin/env bash
# 更新回帰テスト（release_update）の起点を、公開中の portable/latest.json から決める。
#
# 使い方: tools/resolve-update-origin.sh <候補版 X.Y.Z>
# 出力（GITHUB_OUTPUT があればそこへ、無ければ標準出力へ）:
#   skip=true|false
#   origin=<X.Y.Z>   skip=false のときだけ。R2 の portable/<X.Y.Z>/ から取得する起点の版
#
# 規則:
#   - latest.json が 404: 初回の移行版。1.1.8 以前は GitHub しか見ない旧クライアントのため、公開版を起点にできない。
#     skip=true にして ::notice を出す（初回だけの明示的なスキップ）。
#   - 公開中の版が候補以上: 同じ版の再実行（または後退）。起点にできないため skip=true。
#   - 公開中の版が候補より古い: その版を起点（skip=false）。
#   - 上のどれにも当たらない異常（HTTP エラー・版の形式不正）は失敗にする。黙ってスキップしない。
# curl と sort -V だけを使う（Windows の Git Bash・Mac のどちらでも動く）。
set -euo pipefail

candidate="${1:?candidate version (X.Y.Z) is required}"
[[ "$candidate" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "invalid candidate version: $candidate" >&2; exit 1; }
feed="${PORTABLE_FEED:-https://skilvella.kagayoi.com/portable}"

out() { if [[ -n "${GITHUB_OUTPUT:-}" ]]; then echo "$1" >> "$GITHUB_OUTPUT"; else echo "$1"; fi; }

body="$(mktemp)"
trap 'rm -f "$body"' EXIT
# リダイレクトは辿らない（固定ホストの応答だけを信頼する）。応答は版だけの小さな JSON。
# 404 の本文は大きい場合があるため、4096 バイトの上限は 200 のときだけ後で確かめる
# （curl の --max-filesize を 4096 にすると 404 まで失敗になる）。
status="$(curl --proto '=https' --silent --show-error --max-time 30 --max-filesize 1048576 \
  --output "$body" --write-out '%{http_code}' "$feed/latest.json")"

if [[ "$status" == "404" ]]; then
  echo "::notice title=更新回帰テストをスキップ::$feed/latest.json が未公開（初回の移行版）のため、公開版を起点にできません。"
  out "skip=true"
  exit 0
fi
[[ "$status" == "200" ]] || { echo "could not read $feed/latest.json (HTTP $status)" >&2; exit 1; }

[[ "$(wc -c < "$body")" -le 4096 ]] || { echo "the published latest.json is too large" >&2; exit 1; }
published="$(sed -n 's/.*"version"[[:space:]]*:[[:space:]]*"\([0-9][0-9.]*\)".*/\1/p' "$body" | head -n 1)"
[[ "$published" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "the published latest.json has no valid version" >&2; exit 1; }

if [[ "$published" == "$candidate" ]] || [[ "$(printf '%s\n%s\n' "$published" "$candidate" | sort -V | tail -n 1)" != "$candidate" ]]; then
  echo "::notice title=更新回帰テストをスキップ::公開中の版 $published が候補 $candidate 以上のため、起点にできません（再実行）。"
  out "skip=true"
  exit 0
fi
out "skip=false"
out "origin=$published"
