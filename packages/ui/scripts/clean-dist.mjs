import { existsSync, rmSync } from "node:fs";
import { fileURLToPath } from "node:url";

// vite's own build-time emptyDir() fails with a confusing mid-build EACCES
// stack trace if dist/ contains files from a previous run that used sudo
// (owned by another user) - clean it up-front with a clear, actionable error
// instead.
const dist = fileURLToPath(new URL("../dist", import.meta.url));

if (existsSync(dist)) {
  try {
    rmSync(dist, { recursive: true, force: true });
  } catch (err) {
    console.error(
      `\nCannot remove ${dist} - it contains files owned by another user ` +
        `(likely a previous build ran under sudo).\nFix: sudo rm -rf "${dist}"\n`,
    );
    console.error(err instanceof Error ? err.message : err);
    process.exit(1);
  }
}
