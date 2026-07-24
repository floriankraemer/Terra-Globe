import { strFromU8, strToU8, unzipSync, zipSync } from "fflate";
import { parseKml, type ParseKmlResult } from "./parseKml.js";
import { serializeKml, type SerializeKmlInput } from "./serializeKml.js";

const DOC_KML_ENTRY = "doc.kml";

export async function serializeKmz(input: SerializeKmlInput): Promise<Uint8Array> {
  const xml = serializeKml(input);
  return zipSync({ [DOC_KML_ENTRY]: strToU8(xml) });
}

export async function parseKmz(bytes: Uint8Array): Promise<ParseKmlResult> {
  const entries = unzipSync(bytes);
  const docKml = entries[DOC_KML_ENTRY];
  if (!docKml) {
    throw new Error(`KMZ archive is missing a ${DOC_KML_ENTRY} entry`);
  }
  return parseKml(strFromU8(docKml));
}
