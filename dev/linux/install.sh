#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
data_home="${XDG_DATA_HOME:-$HOME/.local/share}"
config_home="${XDG_CONFIG_HOME:-$HOME/.config}"
app_dir="${DOWNSHIFT_INSTALL_DIR:-$data_home/downshift}"
applications_dir="${DOWNSHIFT_APPLICATION_DIR:-$data_home/applications}"
icons_dir="${DOWNSHIFT_ICON_DIR:-$data_home/icons/hicolor/512x512/apps}"
desktop_file="$applications_dir/com.samm81.downshift.desktop"
icon_file="$icons_dir/com.samm81.downshift.png"
autostart_file="$config_home/autostart/com.samm81.downshift.desktop"

usage() {
  printf 'usage: %s [--uninstall]\n' "$(basename "$0")"
}

escape_exec_value() {
  local value="$1"
  value="${value//\\/\\\\}"
  value="${value//\"/\\\"}"
  value="${value//\`/\\\`}"
  value="${value//\$/\\\$}"
  value="${value//%/%%}"
  printf '%s' "$value"
}

escape_icon_value() {
  local value="$1"
  value="${value//\\/\\\\}"
  value="${value//$'\n'/\\n}"
  value="${value//$'\t'/\\t}"
  value="${value//$'\r'/\\r}"
  value="${value//;/\\;}"
  printf '%s' "$value"
}

remove_installation() {
  rm -f -- "$app_dir/downshift" "$app_dir/README-linux.md" "$app_dir/install.sh"
  rmdir --ignore-fail-on-non-empty "$app_dir" 2>/dev/null || true
  rm -f -- "$desktop_file" "$icon_file"
  rmdir --ignore-fail-on-non-empty "$applications_dir" 2>/dev/null || true
  rmdir --ignore-fail-on-non-empty "$icons_dir" 2>/dev/null || true
  rm -f -- "$autostart_file"
  rmdir --ignore-fail-on-non-empty "${autostart_file%/*}" 2>/dev/null || true
  printf 'removed Downshift from %s\n' "$app_dir"
}

install_application() {
  if [[ ! -f "$script_dir/downshift" ]]; then
    printf 'error: expected executable at %s/downshift\n' "$script_dir" >&2
    exit 1
  fi

  mkdir -p -- "$app_dir" "$applications_dir" "$icons_dir"
  install -m 755 "$script_dir/downshift" "$app_dir/downshift"
  if [[ -f "$script_dir/README-linux.md" ]]; then
    install -m 644 "$script_dir/README-linux.md" "$app_dir/README-linux.md"
  fi
  if [[ -f "$script_dir/icon.png" ]]; then
    install -m 644 "$script_dir/icon.png" "$icon_file"
  fi

  local executable icon
  executable="$(escape_exec_value "$app_dir/downshift")"
  icon="$(escape_icon_value "$icon_file")"
  {
    printf '%s\n' '[Desktop Entry]'
    printf '%s\n' 'Type=Application'
    printf '%s\n' 'Name=Downshift'
    printf 'Comment=%s\n' 'A quiet desktop breathing cue'
    printf 'Exec="%s"\n' "$executable"
    printf 'Icon=%s\n' "$icon"
    printf '%s\n' 'Terminal=false'
    printf '%s\n' 'Categories=Utility;'
    printf '%s\n' 'StartupNotify=false'
  } >"$desktop_file"

  printf 'installed Downshift in %s\n' "$app_dir"
  printf 'desktop entry: %s\n' "$desktop_file"
}

main() {
  case "${1:-}" in
    "")
      install_application
      ;;
    --uninstall)
      [[ $# -eq 1 ]] || {
        usage >&2
        exit 2
      }
      remove_installation
      ;;
    --help | -h)
      usage
      ;;
    *)
      usage >&2
      exit 2
      ;;
  esac
}

main "$@"
