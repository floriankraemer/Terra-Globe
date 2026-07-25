import * as Cesium from "cesium";
import { useTranslation } from "react-i18next";

export interface SceneModeToolbarProps {
  mode: Cesium.SceneMode;
  onChange: (mode: Cesium.SceneMode) => void;
}

const MODES: { mode: Cesium.SceneMode; labelKey: "scene3d" | "scene2d" | "columbus" }[] = [
  { mode: Cesium.SceneMode.SCENE3D, labelKey: "scene3d" },
  { mode: Cesium.SceneMode.SCENE2D, labelKey: "scene2d" },
  { mode: Cesium.SceneMode.COLUMBUS_VIEW, labelKey: "columbus" },
];

export function SceneModeToolbar({ mode, onChange }: SceneModeToolbarProps) {
  const { t } = useTranslation();
  return (
    <div role="toolbar" aria-label={t("sceneMode.ariaLabel")} className="toolbar-group">
      {MODES.map(({ mode: buttonMode, labelKey }) => (
        <button
          key={labelKey}
          type="button"
          className="btn"
          aria-pressed={mode === buttonMode}
          onClick={() => onChange(buttonMode)}
        >
          {t(`sceneMode.${labelKey}`)}
        </button>
      ))}
    </div>
  );
}
