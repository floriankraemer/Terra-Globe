import { useRef } from "react";
import { useTranslation } from "react-i18next";
import { MenuButton } from "../../components/MenuButton.js";

export interface FileMenuProps {
  disabled: boolean;
  onImportFile: (file: File) => void;
  onExportKml: () => void;
  onExportKmz: () => void;
}

export function FileMenu({ disabled, onImportFile, onExportKml, onExportKmz }: FileMenuProps) {
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
      <MenuButton label={t("fileMenu.trigger")} disabled={disabled}>
        {(close) => (
          <>
            <button
              type="button"
              role="menuitem"
              className="menu-item"
              onClick={() => {
                inputRef.current?.click();
                close();
              }}
            >
              {t("importExport.importButton")}
            </button>
            <button
              type="button"
              role="menuitem"
              className="menu-item"
              onClick={() => {
                onExportKml();
                close();
              }}
            >
              {t("importExport.exportKml")}
            </button>
            <button
              type="button"
              role="menuitem"
              className="menu-item"
              onClick={() => {
                onExportKmz();
                close();
              }}
            >
              {t("importExport.exportKmz")}
            </button>
          </>
        )}
      </MenuButton>
    </div>
  );
}
