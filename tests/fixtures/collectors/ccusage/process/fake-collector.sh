#!/bin/sh
case "$1" in
  --version) printf 'ccusage 20.0.11\n' ;;
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
