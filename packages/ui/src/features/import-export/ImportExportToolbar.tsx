import { useRef } from "react";

export interface ImportExportToolbarProps {
  disabled: boolean;
  onImportFile: (file: File) => void;
  onExportKml: () => void;
  onExportKmz: () => void;
}

export function ImportExportToolbar({
  disabled,
  onImportFile,
  onExportKml,
  onExportKmz,
}: ImportExportToolbarProps) {
  const inputRef = useRef<HTMLInputElement>(null);

  return (
    <div className="toolbar-group">
      <input
        ref={inputRef}
        type="file"
        accept=".kml,.kmz"
        style={{ display: "none" }}
        onChange={(e) => {
          const file = e.target.files?.[0];
          if (file) onImportFile(file);
          e.target.value = "";
        }}
        aria-hidden="true"
        tabIndex={-1}
      />
      <button
        type="button"
        className="btn"
        disabled={disabled}
        onClick={() => inputRef.current?.click()}
      >
        Import KML/KMZ
      </button>
      <button type="button" className="btn" disabled={disabled} onClick={onExportKml}>
        Export KML
      </button>
      <button type="button" className="btn" disabled={disabled} onClick={onExportKmz}>
        Export KMZ
      </button>
    </div>
  );
}
