#!/usr/bin/env bash
# Download the sidecar binaries Tauri bundles into the app.
#
# Tauri requires the files to be named `<name>-<target-triple>` and referenced
# from `tauri.conf.json` under `bundle.externalBin`. They are large and
# platform-specific, so they are gitignored and fetched on demand (and in CI).
#
# The `tsuki-` prefix is deliberate: Linux packages install externalBin files
# into /usr/bin, and plain `yt-dlp`/`ffmpeg` would conflict with distro packages.
#
# Usage:
#   scripts/fetch-sidecars.sh                # host target triple
#   scripts/fetch-sidecars.sh <triple>       # e.g. aarch64-apple-darwin
#   scripts/fetch-sidecars.sh --force        # re-download even if present
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
out_dir="$root/src-tauri/binaries"

force=0
triple=""
for arg in "$@"; do
  case "$arg" in
    --force) force=1 ;;
    -*) echo "unknown option: $arg" >&2; exit 2 ;;
    *) triple="$arg" ;;
  esac
done

if [[ -z "$triple" ]]; then
  triple="$(rustc -vV | sed -n 's/^host: //p')"
fi
if [[ -z "$triple" ]]; then
  echo "could not determine the target triple; pass it as an argument" >&2
  exit 1
fi

mkdir -p "$out_dir"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

say() { printf '\033[1;36m==>\033[0m %s\n' "$*"; }
die() { printf '\033[1;31merror:\033[0m %s\n' "$*" >&2; exit 1; }

have() { command -v "$1" >/dev/null 2>&1; }

fetch() {
  local url="$1" dest="$2"
  if [[ -f "$dest" && $force -eq 0 ]]; then
    say "$(basename "$dest") already present (use --force to refresh)"
    return
  fi
  say "downloading $(basename "$dest")"
  if have curl; then
    curl -fL --retry 3 --connect-timeout 20 -o "$dest" "$url"
  elif have wget; then
    wget -q --tries=3 -O "$dest" "$url"
  else
    die "curl or wget is required"
  fi
  # A proxy or a rate-limited mirror can answer 200 with an HTML error page.
  # A real yt-dlp binary is never this small; catch it before it is bundled.
  if [[ $(wc -c < "$dest") -lt 100000 ]]; then
    die "$(basename "$dest") is too small to be a real binary; check $url"
  fi
  chmod +x "$dest"
}

# Download an archive, verifying its format, and fall back to another mirror.
# `curl -f` alone is not enough: some mirrors answer 200 with an HTML page.
fetch_archive() {
  local kind="$1" dest="$2"
  shift 2
  local url valid
  for url in "$@"; do
    say "downloading $(basename "$dest") from $url"
    rm -f "$dest"
    if ! curl -fL --retry 2 --connect-timeout 20 -o "$dest" "$url"; then
      continue
    fi
    valid=0
    if [[ "$kind" == xz ]] && have xz && xz -t "$dest" >/dev/null 2>&1; then
      valid=1
    elif [[ "$kind" == zip ]] && have unzip && unzip -tq "$dest" >/dev/null 2>&1; then
      valid=1
    fi
    if [[ $valid -eq 1 ]]; then
      return 0
    fi
    say "$(basename "$dest") from $url was not a valid $kind archive; trying the next mirror"
  done
  rm -f "$dest"
  die "could not download a valid $kind archive for $triple"
}

ytdlp_url=""
case "$triple" in
  *-unknown-linux-gnu|*-unknown-linux-musl)
    ytdlp_url="https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux"
    [[ "$triple" == aarch64* ]] && ytdlp_url="https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux_aarch64"
    ;;
  *-apple-darwin)
    ytdlp_url="https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_macos"
    ;;
  *-pc-windows-msvc|*-pc-windows-gnu)
    ytdlp_url="https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe"
    ;;
  *)
    die "no yt-dlp build known for $triple"
    ;;
esac

suffix=""
case "$triple" in
  *-pc-windows-*) suffix=".exe" ;;
esac

fetch "$ytdlp_url" "$out_dir/tsuki-yt-dlp-$triple$suffix"

# ffmpeg: prefer a static build that needs no system libraries. yt-dlp only
# needs it to merge separate video+audio streams and remux containers.
ffmpeg_dest="$out_dir/tsuki-ffmpeg-$triple$suffix"
if [[ -f "$ffmpeg_dest" && $force -eq 0 ]]; then
  say "tsuki-ffmpeg-$triple already present (use --force to refresh)"
elif [[ -n "$suffix" ]]; then
  # Windows: gyan.dev builds are the de-facto standard.
  fetch "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip" "$tmp/ffmpeg.zip"
  if have unzip; then
    unzip -q -j "$tmp/ffmpeg.zip" '*/bin/ffmpeg.exe' -d "$tmp/bin" || die "ffmpeg.exe not found in the archive"
    mv "$tmp/bin/ffmpeg.exe" "$ffmpeg_dest"
  else
    die "unzip is required to extract ffmpeg on Windows"
  fi
elif [[ "$triple" == *-apple-darwin ]]; then
  # macOS has no redistributable static build we can fetch blindly; fall back
  # to a Homebrew install so local builds and CI still work.
  if have ffmpeg; then
    say "using the ffmpeg on PATH for $triple"
    cp "$(command -v ffmpeg)" "$ffmpeg_dest"
  else
    die "install ffmpeg (brew install ffmpeg) or copy it to $ffmpeg_dest"
  fi
else
  # Linux: prefer BtbN's static builds, fall back to johnvansickle (GPL;
  # see THIRD_PARTY.md). Both mirrors are validated before they are used:
  # a captive proxy can answer an archive request with a small HTML page.
  if [[ "$triple" == aarch64* ]]; then
    primary="https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-linuxarm64-gpl.tar.xz"
    fallback="https://johnvansickle.com/ffmpeg/releases/ffmpeg-release-arm64-static.tar.xz"
  else
    primary="https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-linux64-gpl.tar.xz"
    fallback="https://johnvansickle.com/ffmpeg/releases/ffmpeg-release-amd64-static.tar.xz"
  fi
  fetch_archive xz "$tmp/ffmpeg.tar.xz" "$primary" "$fallback"
  tar -xJf "$tmp/ffmpeg.tar.xz" -C "$tmp"
  src="$(find "$tmp" -type f -name ffmpeg -path '*/ffmpeg-*' | head -n1)"
  [[ -n "$src" ]] || die "ffmpeg not found in the archive"
  mv "$src" "$ffmpeg_dest"
fi

chmod +x "$out_dir/tsuki-yt-dlp-$triple$suffix" "$ffmpeg_dest"
say "sidecars ready for $triple:"
ls -lh "$out_dir" | sed 's/^/    /'
