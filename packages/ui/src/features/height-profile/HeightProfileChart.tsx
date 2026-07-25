import { useState } from "react";
import { useTranslation } from "react-i18next";
import {
  formatDistance,
  pickGridStepMeters,
  type TrackProfilePoint,
  type UnitSystem,
} from "@terra-globe/core";

export interface HeightProfileChartProps {
  profile: TrackProfilePoint[];
  unitSystem: UnitSystem;
  width: number;
  height: number;
}

const PADDING_LEFT = 48;
const PADDING_RIGHT = 12;
const PADDING_TOP = 12;
const PADDING_BOTTOM = 28;

function range(start: number, stop: number, step: number): number[] {
  const values: number[] = [];
  for (let v = start; v <= stop + 1e-6; v += step) values.push(v);
  return values;
}

function altitudeAt(profile: TrackProfilePoint[], distanceMeters: number): number {
  let prev = profile[0]!;
  for (const point of profile) {
    if (point.distanceMeters >= distanceMeters) {
      const span = point.distanceMeters - prev.distanceMeters;
      const t = span > 0 ? (distanceMeters - prev.distanceMeters) / span : 0;
      return prev.altitudeMeters + (point.altitudeMeters - prev.altitudeMeters) * t;
    }
    prev = point;
  }
  return prev.altitudeMeters;
}

export function HeightProfileChart({
  profile,
  unitSystem,
  width,
  height,
}: HeightProfileChartProps) {
  const { t } = useTranslation();
  const [hoverX, setHoverX] = useState<number | null>(null);
  const plotWidth = Math.max(1, width - PADDING_LEFT - PADDING_RIGHT);
  const plotHeight = Math.max(1, height - PADDING_TOP - PADDING_BOTTOM);

  const totalDistance = profile.at(-1)?.distanceMeters ?? 0;
  const altitudes = profile.map((p) => p.altitudeMeters);
  const minAltitude = altitudes.length > 0 ? Math.min(...altitudes) : 0;
  const maxAltitudeRaw = altitudes.length > 0 ? Math.max(...altitudes) : 0;
  const isFlat = maxAltitudeRaw <= minAltitude;
  // Guard against a zero-height range (flat/no-altitude track) so the scale doesn't divide by zero.
  const maxAltitude = isFlat ? minAltitude + 1 : maxAltitudeRaw;

  function xFor(distanceMeters: number): number {
    return totalDistance > 0 ? (distanceMeters / totalDistance) * plotWidth : 0;
  }
  function yFor(altitudeMeters: number): number {
    return plotHeight - ((altitudeMeters - minAltitude) / (maxAltitude - minAltitude)) * plotHeight;
  }

  const distanceStep = pickGridStepMeters(totalDistance);
  const distanceGridlines = totalDistance > 0 ? range(0, totalDistance, distanceStep) : [0];
  // A flat/no-altitude track has no real span to subdivide - show a single gridline
  // instead of 4 steps that round to the same label.
  const altitudeGridlines = isFlat
    ? [minAltitude]
    : range(minAltitude, maxAltitude, (maxAltitude - minAltitude) / 4);

  const points = profile
    .map((p) => `${xFor(p.distanceMeters)},${yFor(p.altitudeMeters)}`)
    .join(" ");

  const canHover = profile.length > 1 && totalDistance > 0;
  const hoverDistance = hoverX !== null && canHover ? (hoverX / plotWidth) * totalDistance : null;
  const hoverAltitude = hoverDistance !== null ? altitudeAt(profile, hoverDistance) : null;

  function handleHoverMove(e: React.MouseEvent<SVGRectElement>) {
    const bounds = e.currentTarget.getBoundingClientRect();
    const x = Math.min(plotWidth, Math.max(0, e.clientX - bounds.left));
    setHoverX(x);
  }

  return (
    <svg
      className="height-profile-chart"
      width={width}
      height={height}
      role="img"
      aria-label={t("heightProfile.ariaLabel")}
    >
      <g transform={`translate(${PADDING_LEFT}, ${PADDING_TOP})`}>
        {altitudeGridlines.map((altitude) => (
          <g key={`alt-${altitude}`}>
            <line
              className="height-profile-gridline"
              x1={0}
              x2={plotWidth}
              y1={yFor(altitude)}
              y2={yFor(altitude)}
            />
            <text
              className="height-profile-axis-label"
              x={-6}
              y={yFor(altitude)}
              textAnchor="end"
              dy="0.32em"
            >
              {Math.round(altitude)}m
            </text>
          </g>
        ))}
        {distanceGridlines.map((distance) => (
          <g key={`dist-${distance}`}>
            <line
              className="height-profile-gridline"
              x1={xFor(distance)}
              x2={xFor(distance)}
              y1={0}
              y2={plotHeight}
            />
            <text
              className="height-profile-axis-label"
              x={xFor(distance)}
              y={plotHeight + 14}
              textAnchor="middle"
            >
              {formatDistance(distance, unitSystem)}
            </text>
          </g>
        ))}
        {profile.length > 1 && <polyline className="height-profile-line" points={points} />}
        {profile.length === 1 && (
          <circle
            className="height-profile-line"
            cx={xFor(0)}
            cy={yFor(profile[0]!.altitudeMeters)}
            r={2}
          />
        )}
        {hoverX !== null && hoverAltitude !== null && (
          <g>
            <line
              className="height-profile-hover-line"
              x1={hoverX}
              x2={hoverX}
              y1={0}
              y2={plotHeight}
            />
            <circle
              className="height-profile-hover-dot"
              cx={hoverX}
              cy={yFor(hoverAltitude)}
              r={3}
            />
            <text
              className="height-profile-hover-label"
              x={Math.min(plotWidth - 4, Math.max(4, hoverX))}
              y={Math.max(10, yFor(hoverAltitude) - 10)}
              textAnchor={hoverX > plotWidth - 40 ? "end" : hoverX < 40 ? "start" : "middle"}
            >
              {Math.round(hoverAltitude)}m
            </text>
          </g>
        )}
        {canHover && (
          <rect
            className="height-profile-hover-target"
            x={0}
            y={0}
            width={plotWidth}
            height={plotHeight}
            fill="transparent"
            onMouseMove={handleHoverMove}
            onMouseLeave={() => setHoverX(null)}
          />
        )}
      </g>
    </svg>
  );
}
