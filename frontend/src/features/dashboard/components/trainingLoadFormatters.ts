type TimelineLabel = {
  key: string;
  label: string;
};

function formatNumber(value: number, language: string, fractionDigits: number) {
  return new Intl.NumberFormat(language, {
    minimumFractionDigits: fractionDigits,
    maximumFractionDigits: fractionDigits,
  }).format(value);
}

export function formatMetricValue(value: number | null, language: string) {
  return value === null ? '-' : formatNumber(value, language, 1);
}

export function formatSignedMetricValue(value: number | null, language: string) {
  if (value === null) {
    return '-';
  }

  const sign = value > 0 ? '+' : '';
  return `${sign}${formatNumber(value, language, 1)}`;
}

export function formatTwoDecimalValue(value: number | null, language: string) {
  return value === null ? '-' : formatNumber(value, language, 2);
}

export function formatWattsValue(value: number | null, language: string) {
  if (value === null) {
    return '-';
  }

  try {
    return new Intl.NumberFormat(language, {
      style: 'unit',
      unit: 'watt',
      unitDisplay: 'narrow',
      maximumFractionDigits: 0,
    }).format(value);
  } catch {
    return `${formatNumber(value, language, 0)} W`;
  }
}

export function formatShortDate(date: string, language: string) {
  const parsed = new Date(`${date}T00:00:00Z`);
  return Number.isNaN(parsed.getTime())
    ? date
    : new Intl.DateTimeFormat(language, { month: 'short', day: '2-digit', timeZone: 'UTC' }).format(parsed);
}

export function formatWindowDate(date: string, language: string) {
  const parsed = new Date(`${date}T00:00:00Z`);
  return Number.isNaN(parsed.getTime())
    ? date
    : new Intl.DateTimeFormat(language, { month: 'short', day: '2-digit', year: 'numeric', timeZone: 'UTC' }).format(parsed);
}

export function buildTimelineLabels(dates: string[], language: string): TimelineLabel[] {
  if (dates.length === 0) {
    return [];
  }

  const candidateIndexes = Array.from(new Set([
    0,
    Math.floor((dates.length - 1) * 0.33),
    Math.floor((dates.length - 1) * 0.66),
    dates.length - 1,
  ]));

  return candidateIndexes.map((index) => ({ key: `${dates[index]}-${index}`, label: formatShortDate(dates[index], language) }));
}
