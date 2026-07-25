import { Flag, Landmark, MapPin, Navigation, Pin, Star, type LucideIcon } from "lucide-react";
import { MARKER_ICON_IDS, type MarkerIconId } from "@terra-globe/core";

const MARKER_ICON_COMPONENTS: Record<MarkerIconId, LucideIcon> = {
  "map-pin": MapPin,
  pin: Pin,
  flag: Flag,
  star: Star,
  landmark: Landmark,
  navigation: Navigation,
};

export const MARKER_ICON_OPTIONS = MARKER_ICON_IDS.map((id) => ({
  id,
  Icon: MARKER_ICON_COMPONENTS[id],
}));
