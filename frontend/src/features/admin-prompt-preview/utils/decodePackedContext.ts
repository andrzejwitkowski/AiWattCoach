const KEY_LABEL: Record<string, string> = {
  v: 'Schema Version',
  i: 'Intervals Status',
  g: 'Generated At (epoch)',
  fx: 'Focus',
  k: 'Kind',
  p: 'Athlete Profile',
  rc: 'Race Calendar',
  fe: 'Future Events',
  prd: 'Planned Rest Days',
  h: 'Historical Training',
  rd: 'Recent Days',
  wr: 'Workout Recaps',
  ud: 'Upcoming Days',
  pd: 'Projected Days',
  a: 'Activities Status',
  e: 'Events Status',
  fnm: 'Full Name',
  age: 'Age',
  hcm: 'Height (cm)',
  wkg: 'Weight (kg)',
  ftp: 'FTP (W)',
  hrm: 'Max HR',
  vo2: 'VO₂max',
  ap: 'Athlete Prompt',
  meds: 'Medications',
  notes: 'Athlete Notes',
  acfg: 'Availability Configured',
  av: 'Weekly Availability',
  wd: 'Weekday',
  available: 'Available',
  mdm: 'Max Duration (min)',
  ws: 'Window Start',
  we: 'Window End',
  ac: 'Activity Count',
  ttss: 'Total TSS',
  ctl: 'CTL',
  atl: 'ATL',
  tsb: 'TSB',
  ftpd: 'FTP Change',
  t7: 'Avg TSS 7d',
  t28: 'Avg TSS 28d',
  if28: 'Avg IF 28d',
  lt: 'Load Trend',
  w: 'Workouts',
  sa: 'Aligned Intervals',
  pw: 'Planned Workouts',
  days: 'Days',
  tss: 'TSS',
  dur: 'Duration (s)',
  ifv: 'IF',
  ef: 'EF',
  np: 'NP',
  vi: 'VI',
  rpe: 'RPE',
  recap: 'Recap',
  bl: 'Interval Blocks',
  ps: 'Power segments [minW,maxW,durSec]',
  cs: 'Cadence segments [minRPM,maxRPM,durSec]',
  minp: 'Min %FTP',
  maxp: 'Max %FTP',
  minw: 'Min Watts',
  maxw: 'Max Watts',
  sd: 'Start Date',
  ed: 'End Date',
  nt: 'Note',
  n: 'Name',
  ty: 'Activity Type',
  c: 'Category',
  desc: 'Description',
  id: 'ID',
  d: 'Date',
  fr: 'Calendar Empty',
  sick: 'Sick',
  sickn: 'Sick Note',
  doc: 'Raw Doc',
  done: 'Completed',
  km: 'Distance (km)',
  disc: 'Discipline',
  pri: 'Priority',
  n7d: 'Avg NP 7d',
  n28d: 'Avg NP 28d',
};

export function decodeShortKey(key: string): string {
  return KEY_LABEL[key] ?? key;
}

export function parseHeaderTable(table: unknown): Record<string, unknown>[] {
  if (typeof table !== 'object' || table === null || Array.isArray(table)) return [];
  const { h, r } = table as { h?: unknown; r?: unknown };
  if (!Array.isArray(h) || !Array.isArray(r)) return [];

  const headers = h.map(String);
  return r.flatMap((row) => {
    if (!Array.isArray(row)) return [];
    const record: Record<string, unknown> = {};
    headers.forEach((header, index) => {
      record[header] = row[index] ?? null;
    });
    return [record];
  });
}
export function formatSeconds(seconds: number): string {
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
}

export function formatSegmentTriplets(segments: unknown, unit: 'W' | 'RPM'): string[] {
  if (!Array.isArray(segments)) return [];

  return segments.flatMap((segment) => {
    if (!Array.isArray(segment) || segment.length !== 3) return [];

    const [min, max, duration] = segment;
    const minNum = Number(min);
    const maxNum = Number(max);
    const durationNum = Number(duration);
    if (
      !Number.isFinite(minNum) ||
      !Number.isFinite(maxNum) ||
      !Number.isFinite(durationNum)
    ) {
      return [];
    }

    const range = minNum === maxNum ? `${minNum}` : `${minNum}–${maxNum}`;
    return [`${range} ${unit} · ${formatSeconds(durationNum)}`];
  });
}
