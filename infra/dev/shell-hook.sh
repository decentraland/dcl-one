# nix develop --command and direnv get tools without launching services.
export FOUNDATION_DEV_ROOT="$(git rev-parse --show-toplevel)"
if [[ ! -f "$FOUNDATION_DEV_ROOT/tooling/dev.py" ]]; then
  echo "Enter nix develop from the Foundation checkout." >&2
  return 1
fi
if [[ $- == *i* && ${FOUNDATION_DEV_AUTOSTART:-1} != 0 ]]; then
  foundation-dev up || echo "Development startup failed. Run foundation-dev up to retry." >&2
fi
