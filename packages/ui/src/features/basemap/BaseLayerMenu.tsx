import { useTranslation } from "react-i18next";
import { MenuButton } from "../../components/MenuButton.js";

export interface TileSource {
  id: string;
  name: string;
}

export interface BaseLayerMenuProps {
  tileSources: TileSource[];
  baseLayerId: string;
  onChange: (id: string) => void;
}

export function BaseLayerMenu({ tileSources, baseLayerId, onChange }: BaseLayerMenuProps) {
  const { t } = useTranslation();
  const activeName = tileSources.find((source) => source.id === baseLayerId)?.name ?? "";
  return (
    <div className="toolbar-group">
      <MenuButton label={`${t("app.basemap")}: ${activeName}`}>
        {(close) =>
          tileSources.map((source) => (
            <button
              key={source.id}
              type="button"
              role="menuitemradio"
              className="menu-item"
              aria-checked={source.id === baseLayerId}
              onClick={() => {
                onChange(source.id);
                close();
              }}
            >
              {source.name}
            </button>
          ))
        }
      </MenuButton>
    </div>
  );
}
