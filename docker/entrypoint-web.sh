#!/bin/sh
# node_modules for /workspace and each package lives in a named volume
# (docker-compose.yml), mounted under the bind-mounted source tree. Docker's
# copy-image-content-into-empty-volume step is unreliable for volumes nested
# under an existing bind mount, so a fresh/stale volume can end up missing
# deps the image itself has installed correctly. Reinstalling here, against
# the frozen lockfile, is a no-op when the volume already matches and
# self-heals it otherwise.
set -e
for dir in node_modules packages/*/node_modules e2e/playwright/node_modules; do
  find "$dir" -mindepth 1 -delete 2>/dev/null || true
done
pnpm install --frozen-lockfile
exec "$@"
