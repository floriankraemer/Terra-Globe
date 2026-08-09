#!/bin/sh
# See docker/entrypoint-web.sh for why this reinstall is needed: node_modules
# named volumes nested under the bind-mounted source tree don't reliably
# inherit the image's already-installed deps.
set -e
for dir in node_modules packages/*/node_modules e2e/playwright/node_modules; do
  find "$dir" -mindepth 1 -delete 2>/dev/null || true
done
pnpm install --frozen-lockfile
exec "$@"
