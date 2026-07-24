import { parseKml, type KmlNetworkLinkRef, type ParseKmlResult } from "./parseKml.js";

export interface ResolveNetworkLinksOptions {
  /** Injectable for tests/non-browser runtimes; defaults to the global fetch. */
  fetch?: typeof fetch;
  /** Caps recursive NetworkLink chains (A links to B links to C, ...). */
  maxDepth?: number;
}

const DEFAULT_MAX_DEPTH = 5;

/**
 * Statically resolves every `<NetworkLink>` found by parseKml: fetches each
 * href once, parses it with the same parser, and inlines the result into the
 * folder that held the link. One-time resolve, no polling/refresh - matches
 * this app's static-import model. Failures (network error, bad XML, cycles)
 * are recorded as warnings, never thrown - consistent with the rest of the
 * importer's fail-soft contract.
 */
export async function resolveNetworkLinks(
  result: ParseKmlResult,
  options: ResolveNetworkLinksOptions = {},
): Promise<ParseKmlResult> {
  const fetchImpl = options.fetch ?? fetch;
  const maxDepth = options.maxDepth ?? DEFAULT_MAX_DEPTH;

  let folders = result.folders;
  let placemarks = result.placemarks;
  let styles = result.styles;
  let screenOverlays = result.screenOverlays;
  const warnings = [...result.warnings];
  const visited = new Set<string>();

  let pending: KmlNetworkLinkRef[] = result.networkLinks;
  let depth = 0;

  while (pending.length > 0 && depth < maxDepth) {
    const nextPending: KmlNetworkLinkRef[] = [];

    for (const link of pending) {
      if (visited.has(link.href)) {
        warnings.push(`Skipped NetworkLink "${link.href}": already resolved (cycle).`);
        continue;
      }
      visited.add(link.href);

      try {
        const response = await fetchImpl(link.href);
        if (!response.ok) {
          throw new Error(`HTTP ${response.status}`);
        }
        const xml = await response.text();
        const child = parseKml(xml);

        folders = [
          ...folders,
          ...child.folders.map((f) => (f.parentId === null ? { ...f, parentId: link.folderId } : f)),
        ];
        placemarks = [
          ...placemarks,
          ...child.placemarks.map((p) => (p.folderId === null ? { ...p, folderId: link.folderId } : p)),
        ];
        styles = [...styles, ...child.styles];
        screenOverlays = [
          ...screenOverlays,
          ...child.screenOverlays.map((o) =>
            o.folderId === null ? { ...o, folderId: link.folderId } : o,
          ),
        ];
        warnings.push(...child.warnings);
        nextPending.push(
          ...child.networkLinks.map((nl) => (nl.folderId === null ? { ...nl, folderId: link.folderId } : nl)),
        );
      } catch (err) {
        warnings.push(
          `Failed to resolve NetworkLink "${link.href}": ${err instanceof Error ? err.message : String(err)}`,
        );
      }
    }

    pending = nextPending;
    depth++;
  }

  if (pending.length > 0) {
    warnings.push(`Stopped resolving NetworkLinks after reaching max depth (${maxDepth}).`);
  }

  return { folders, placemarks, styles, screenOverlays, networkLinks: [], warnings };
}
