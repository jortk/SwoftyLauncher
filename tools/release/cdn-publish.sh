#!/usr/bin/env bash
# Usage: cdn-publish.sh <tag> <artefacts-dir> [installer-dir]
# Needs R2_ACCESS_KEY_ID, R2_SECRET_ACCESS_KEY, R2_ENDPOINT, R2_BUCKET; aws, curl, jq, sha256sum.
# Optional: CDN_BASE (default https://cdn.swofty.net), PUBLISHED_AT, NOTES_URL.
set -euo pipefail

tag=${1:?tag}
dir=${2:?artefacts directory}
installers=${3:-}
version=${tag#v}
cdn=${CDN_BASE:-https://cdn.swofty.net}
notes=${NOTES_URL:-https://github.com/Swofty-Developments/SwoftyLauncher/releases/tag/$tag}
published=${PUBLISHED_AT:-$(date -u +%Y-%m-%dT%H:%M:%SZ)}
prefix="launcher/$tag"

export AWS_ACCESS_KEY_ID=$R2_ACCESS_KEY_ID
export AWS_SECRET_ACCESS_KEY=$R2_SECRET_ACCESS_KEY
export AWS_DEFAULT_REGION=auto
export AWS_EC2_METADATA_DISABLED=true
export AWS_REQUEST_CHECKSUM_CALCULATION=when_required
export AWS_RESPONSE_CHECKSUM_VALIDATION=when_required

put() {
  aws s3 cp "$1" "s3://$R2_BUCKET/$2" --endpoint-url "$R2_ENDPOINT" --only-show-errors \
    --cache-control "$3" ${4:+--content-type "$4"}
}

fetch_sha() {
  local url=$1 sha
  for attempt in 1 2 3 4 5; do
    if sha=$(curl -fsSL --retry 3 "$url" | sha256sum | cut -d' ' -f1) && [ -n "$sha" ]; then
      echo "$sha"
      return 0
    fi
    sleep $((attempt * 3))
  done
  return 1
}

cd "$dir"
sha256sum -c SHA256SUMS
files=(swofty-launcher-* SHA256SUMS)

assets='[]'
for f in "${files[@]}"; do
  put "$f" "$prefix/$f" "public, max-age=31536000, immutable"
  want=$(sha256sum "$f" | cut -d' ' -f1)
  got=$(fetch_sha "$cdn/$prefix/$f")
  if [ "$want" != "$got" ]; then
    echo "::error::$cdn/$prefix/$f has SHA-256 $got, expected $want"
    exit 1
  fi
  echo "verified $prefix/$f $want"
  assets=$(jq -c --arg name "$f" --arg url "$cdn/$prefix/$f" --argjson size "$(stat -c %s "$f")" --arg sha "$want" \
    '. + [{name: $name, url: $url, size: $size, sha256: $sha}]' <<<"$assets")
done

cd - >/dev/null
if [ -n "$installers" ] && [ -d "$installers" ]; then
  for f in "$installers"/SwoftyLauncherInstaller*; do
    [ -f "$f" ] || continue
    put "$f" "launcher/latest/$(basename "$f")" "public, max-age=300"
    echo "uploaded launcher/latest/$(basename "$f")"
  done
fi

case "$version" in
  *-*)
    echo "$tag is a pre-release; launcher/latest.json is left as it is"
    exit 0
    ;;
esac

manifest=$(mktemp)
jq -n --arg version "$version" --arg tag "$tag" --arg published "$published" --arg notes "$notes" \
  --argjson assets "$assets" \
  '{schema: 1, version: $version, tag: $tag, published_at: $published, notes_url: $notes, assets: $assets}' >"$manifest"
put "$manifest" "launcher/latest.json" "public, max-age=60" "application/json"
echo "wrote launcher/latest.json for $tag"
cat "$manifest"
