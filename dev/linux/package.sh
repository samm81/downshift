#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
repo_root="$(cd -- "$script_dir/../.." && pwd -P)"
version="${1:-}"
binary="${2:-$repo_root/target/release/downshift}"
dist_dir="${3:-$repo_root/dist/linux}"

if [[ -z "$version" ]]; then
  printf 'usage: %s <version> [binary] [output-directory]\n' "$(basename "$0")" >&2
  exit 2
fi

version="${version#v}"
if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+([.-][0-9A-Za-z.-]+)?$ ]]; then
  printf 'error: invalid release version: %s\n' "$version" >&2
  exit 2
fi
if [[ ! -f "$binary" ]]; then
  printf 'error: release binary not found: %s\n' "$binary" >&2
  exit 1
fi

package_name="Downshift-linux-x86_64-v$version"
artifact="$dist_dir/$package_name.tar.gz"
staging_parent="$(mktemp -d "${TMPDIR:-/tmp}/downshift-linux-package.XXXXXX")"
staging="$staging_parent/$package_name"

cleanup() {
  rm -rf -- "$staging_parent"
}
trap cleanup EXIT

mkdir -p -- "$staging" "$dist_dir"
install -m 755 "$binary" "$staging/downshift"
install -m 755 "$script_dir/install.sh" "$staging/install.sh"
install -m 644 "$repo_root/README-linux.md" "$staging/README-linux.md"
install -m 644 "$repo_root/docs/assets/icon.png" "$staging/icon.png"

{
  printf '%s\n' '[Desktop Entry]'
  printf '%s\n' 'Type=Application'
  printf '%s\n' 'Name=Downshift'
  printf '%s\n' 'Comment=A quiet desktop breathing cue'
  printf '%s\n' 'Exec=__DOWNSHIFT_INSTALL_DIR__/downshift'
  printf '%s\n' 'Icon=__DOWNSHIFT_ICON_FILE__'
  printf '%s\n' 'Terminal=false'
  printf '%s\n' 'Categories=Utility;'
  printf '%s\n' 'StartupNotify=false'
} >"$staging/com.samm81.downshift.desktop"

tar -czf "$artifact" -C "$staging_parent" "$package_name"
test -s "$artifact"
tar -tzf "$artifact" | grep -Fx "$package_name/downshift" >/dev/null
tar -tzf "$artifact" | grep -Fx "$package_name/install.sh" >/dev/null
tar -tzf "$artifact" | grep -Fx "$package_name/README-linux.md" >/dev/null
tar -tzf "$artifact" | grep -Fx "$package_name/icon.png" >/dev/null
tar -tzf "$artifact" | grep -Fx "$package_name/com.samm81.downshift.desktop" >/dev/null
tar -xOf "$artifact" "$package_name/com.samm81.downshift.desktop" | grep -Fx 'Type=Application' >/dev/null
tar -xOf "$artifact" "$package_name/com.samm81.downshift.desktop" | grep -Fx 'Exec=__DOWNSHIFT_INSTALL_DIR__/downshift' >/dev/null
tar -tvzf "$artifact" | grep -E '^-rwx.* /?Downshift-linux-x86_64-v[^/]+/downshift$' >/dev/null

checksum_path="$dist_dir/SHA256SUMS.txt"
printf '%s  %s\n' "$(sha256sum "$artifact" | awk '{print $1}')" "$(basename "$artifact")" >"$checksum_path"
(cd "$dist_dir" && sha256sum --check --status "$(basename "$checksum_path")")

printf 'created %s\n' "$artifact"
printf 'checksums: %s\n' "$checksum_path"
