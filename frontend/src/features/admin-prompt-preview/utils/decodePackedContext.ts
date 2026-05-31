const KEY_LABEL: Record<string, string> = {
  v: 'Schema Version',
  i: 'Intervals Status',
  g: 'Generated At (epoch)',
  fx: 'Focus',
  k: 'Kind',
  p: 'Athlete Profile',
  rc: 'Race Calendar',
  fe: 'Future Events',
  h: 'Historical Training',
  rd: 'Recent Days',
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
  pc: 'Power Curve',
  c5: 'Cadence 5s',
  minp: 'Min %FTP',
  maxp: 'Max %FTP',
  minw: 'Min Watts',
  maxw: 'Max Watts',
  sd: 'Start Date',
  n: 'Name',
  ty: 'Activity Type',
  c: 'Category',
  desc: 'Description',
  id: 'ID',
  d: 'Date',
  fr: 'Free Day',
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

const SKIP_KEYS = new Set(['pc', 'c5', 'bl', 'doc']);

export function isSkippableBulkKey(key: string): boolean {
  return SKIP_KEYS.has(key);
}

export function formatSeconds(seconds: number): string {
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
}

export function formatEpochSeconds(epoch: number): string {
  const d = new Date(epoch * 1000);
  return d.toISOString().replace('T', ' ').slice(0, 19) + 'Z';
}
