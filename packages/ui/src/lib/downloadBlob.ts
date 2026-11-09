/** Triggers a browser download of in-memory bytes via a throwaway object URL and `<a download>` link. */
export function downloadBlob(bytes: BlobPart, filename: string, mimeType: string): void {
  const blob = new Blob([bytes], { type: mimeType });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = filename;
  link.click();
  URL.revokeObjectURL(url);
}
