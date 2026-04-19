import type { MouseEvent } from 'react';

export type SeriesPoint = {
  index: number;
  value: number;
  x: number;
  y: number;
};

type PositionedAxisLabel = {
  key: string;
  label: string;
  top: number;
};

const CHART_X_INSET = 2;

export function clamp(value: number, min: number, max: number) {
  return Math.min(Math.max(value, min), max);
}

function buildPointX(index: number, total: number, width: number) {
  return total === 1 ? width / 2 : CHART_X_INSET + (index / (total - 1)) * (width - (CHART_X_INSET * 2));
}

export function buildSeriesPoints(values: Array<number | null>, min: number, max: number, height: number, width: number): SeriesPoint[] {
  const denominator = Math.max(max - min, 1);

  return values.flatMap((value, index) => {
    if (value === null) {
      return [];
    }

    const x = buildPointX(index, values.length, width);
    const y = height - ((value - min) / denominator) * height;
    return [{ index, value, x, y }];
  });
}

export function buildLinePath(values: Array<number | null>, min: number, max: number, height: number, width: number) {
  const denominator = Math.max(max - min, 1);
  let segmentOpen = false;

  return values.reduce((path, value, index) => {
    if (value === null) {
      segmentOpen = false;
      return path;
    }

    const x = buildPointX(index, values.length, width);
    const y = height - ((value - min) / denominator) * height;
    const command = `${segmentOpen ? 'L' : 'M'}${x.toFixed(2)},${y.toFixed(2)}`;
    segmentOpen = true;
    return path ? `${path} ${command}` : command;
  }, '');
}

export function buildAreaPath(points: SeriesPoint[], height: number) {
  if (points.length < 2) {
    return '';
  }

  const line = points.map((point, index) => `${index === 0 ? 'M' : 'L'}${point.x.toFixed(2)},${point.y.toFixed(2)}`).join(' ');
  return `${line} L${points[points.length - 1].x.toFixed(2)},${height.toFixed(2)} L${points[0].x.toFixed(2)},${height.toFixed(2)} Z`;
}

function formatAxisLabel(value: number) {
  return Number.isInteger(value) ? value.toString() : value.toFixed(0);
}

export function buildPositionedAxisLabels(values: number[], min: number, max: number): PositionedAxisLabel[] {
  const range = Math.max(max - min, 1);
  const seen = new Set<string>();

  return values.flatMap((value) => {
    if (value < min || value > max) {
      return [];
    }

    const key = value.toFixed(3);
    if (seen.has(key)) {
      return [];
    }
    seen.add(key);

    return [{
      key,
      label: formatAxisLabel(value),
      top: clamp(((max - value) / range) * 100, 0, 100),
    }];
  });
}

export function latestSeriesPoint(...seriesCollections: SeriesPoint[][]) {
  return seriesCollections.flat().reduce<SeriesPoint | null>((latest, point) => {
    if (!latest || point.index > latest.index) {
      return point;
    }

    return latest;
  }, null);
}

export function seriesPointAtIndex(points: SeriesPoint[], index: number | null) {
  if (index === null) {
    return null;
  }

  return points.find((point) => point.index === index) ?? null;
}

export function resolveHoveredIndex(event: MouseEvent<SVGSVGElement>, totalPoints: number) {
  if (totalPoints === 0) {
    return null;
  }

  if (totalPoints === 1) {
    return 0;
  }

  const bounds = event.currentTarget.getBoundingClientRect();
  if (bounds.width <= 0) {
    return totalPoints - 1;
  }

  const ratio = clamp((event.clientX - bounds.left) / bounds.width, 0, 1);
  return Math.round(ratio * (totalPoints - 1));
}
