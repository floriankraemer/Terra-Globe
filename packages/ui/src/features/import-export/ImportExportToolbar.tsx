import { useRef } from "react";
import { useTranslation } from "react-i18next";

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
  const { t } = useTranslation();
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
        {t("importExport.importButton")}
      </button>
      <button type="button" className="btn" disabled={disabled} onClick={onExportKml}>
        {t("importExport.exportKml")}
      </button>
      <button type="button" className="btn" disabled={disabled} onClick={onExportKmz}>
        {t("importExport.exportKmz")}
      </button>
    </div>
  );
}
