import { useRef } from "react";
import { useTranslation } from "react-i18next";
import { Save } from "lucide-react";
import { MenuButton } from "../../components/MenuButton.js";
import { isTauri } from "../../platform/isTauri.js";

export interface FileMenuProps {
  disabled: boolean;
  onImportFile: (file: File) => void;
  onExportKml: () => void;
  onExportKmz: () => void;
  /** Tauri only: opens the native file picker and replaces the library with its contents. */
  onLoad: () => void;
  /** Tauri only: writes the library back to the loaded file (or opens a Save As dialog if nothing was loaded yet). */
  onSave: () => void;
  saveDisabled?: boolean;
}

export function FileMenu({
  disabled,
  onImportFile,
  onExportKml,
  onExportKmz,
  onLoad,
  onSave,
  saveDisabled,
}: FileMenuProps) {
  const { t } = useTranslation();
  const inputRef = useRef<HTMLInputElement>(null);
  const tauri = isTauri();

  return (
    <div className="toolbar-group">
      {!tauri && (
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
      )}
      <MenuButton
        label={
          <>
            <Save size={16} aria-hidden="true" />
            {t("fileMenu.trigger")}
          </>
        }
        disabled={disabled}
      >
        {(close) =>
          tauri ? (
            <>
              <button
                type="button"
                role="menuitem"
                className="menu-item"
                onClick={() => {
                  onLoad();
                  close();
                }}
              >
                {t("importExport.loadButton")}
              </button>
              <button
                type="button"
                role="menuitem"
                className="menu-item"
                disabled={saveDisabled}
                onClick={() => {
                  onSave();
                  close();
                }}
              >
                {t("importExport.saveButton")}
              </button>
            </>
          ) : (
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
          )
        }
      </MenuButton>
    </div>
  );
}
