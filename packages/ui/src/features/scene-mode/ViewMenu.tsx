import * as Cesium from "cesium";
import { useTranslation } from "react-i18next";
import { MenuButton } from "../../components/MenuButton.js";

export interface ViewMenuProps {
  mode: Cesium.SceneMode;
  onChange: (mode: Cesium.SceneMode) => void;
}

const MODES: { mode: Cesium.SceneMode; labelKey: "scene3d" | "scene2d" | "columbus" }[] = [
  { mode: Cesium.SceneMode.SCENE3D, labelKey: "scene3d" },
  { mode: Cesium.SceneMode.SCENE2D, labelKey: "scene2d" },
  { mode: Cesium.SceneMode.COLUMBUS_VIEW, labelKey: "columbus" },
];

export function ViewMenu({ mode, onChange }: ViewMenuProps) {
  const { t } = useTranslation();
  const activeLabelKey = MODES.find((m) => m.mode === mode)?.labelKey ?? "scene3d";
  return (
    <div className="toolbar-group">
      <MenuButton label={`${t("viewMenu.trigger")}: ${t(`sceneMode.${activeLabelKey}`)}`}>
        {(close) =>
          MODES.map(({ mode: buttonMode, labelKey }) => (
            <button
              key={labelKey}
              type="button"
              role="menuitemradio"
              className="menu-item"
              aria-checked={mode === buttonMode}
              onClick={() => {
                onChange(buttonMode);
                close();
              }}
            >
              {t(`sceneMode.${labelKey}`)}
            </button>
          ))
        }
      </MenuButton>
    </div>
  );
}
