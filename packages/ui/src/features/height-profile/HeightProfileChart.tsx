import {
  formatDistance,
  pickGridStepMeters,
  type TrackProfilePoint,
  type UnitSystem,
} from "@webglobe/core";

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

export function HeightProfileChart({
  profile,
  unitSystem,
  width,
  height,
}: HeightProfileChartProps) {
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

  return (
    <svg
      className="height-profile-chart"
      width={width}
      height={height}
      role="img"
      aria-label="Elevation profile"
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
      </g>
    </svg>
  );
}
