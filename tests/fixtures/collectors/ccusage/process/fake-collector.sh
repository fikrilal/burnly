#!/bin/sh
script_path="$(readlink -f -- "$0" 2>/dev/null || printf '%s' "$0")"
fixture_root="$(CDPATH= cd -- "$(dirname -- "$script_path")/.." && pwd)"
name="$(basename -- "$0")"

if [ "$1" = "claude" ] || [ "$1" = "codex" ] || [ "$1" = "opencode" ]; then
  fixture_dir="$fixture_root/$1-$2"
  if [ ! -d "$fixture_dir" ]; then
    exit 7
  fi
  case "$name" in
    *empty*) cat "$fixture_dir/empty.json" ;;
    *invalid-json*) cat "$fixture_dir/invalid-json.json" ;;
    *incompatible*) cat "$fixture_dir/incompatible-envelope.json" ;;
    *non-utf8*) printf '\377' ;;
    *nonzero*) exit 7 ;;
    *stdout-limit*) head -c 20000000 /dev/zero ;;
    *stderr-limit*) head -c 300000 /dev/zero >&2 ;;
    *timeout*) sleep 35 ;;
    *)
      if [ "$2" = "session" ] && [ -f "$fixture_dir/empty.json" ]; then
        cat "$fixture_dir/empty.json"
      else
        cat "$fixture_dir/valid.json"
      fi
      ;;
  esac
  exit
fi

case "$1" in
  --version) printf 'ccusage 20.0.14\n' ;;
  success)
    if IFS= read -r _; then exit 8; fi
    if [ -n "${CCUSAGE_MODEL_ALIASES:-}" ]; then exit 9; fi
    printf 'stdin-closed env-filtered\n'
    printf 'path=%s\n' "$PWD" >&2
    ;;
  stdout-limit) head -c 4096 /dev/zero ;;
  stderr-limit) head -c 4096 /dev/zero >&2 ;;
  sleep) sleep 5 ;;
  nonzero) exit 7 ;;
  non-utf8) printf '\377' ;;
esac
