import type { ReactNode } from 'react';
import { Bike, Flag, Mountain, Timer, Trophy } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import type { Race } from '../types';
import { formatRaceDate, formatRaceDistance, getPriorityTone, mapRaceDisciplineLabel } from '../utils';

type RaceCardProps = {
  race: Race;
  onEdit?: (race: Race) => void;
};

export function RaceCard({ race, onEdit }: RaceCardProps) {
  const { i18n, t } = useTranslation();
  const locale = i18n.resolvedLanguage ?? i18n.language ?? 'en';
  const DisciplineIcon = getDisciplineIcon(race.discipline);
  const syncTone = race.syncStatus === 'synced'
    ? 'text-[#8fe8a4]'
    : race.syncStatus === 'failed'
      ? 'text-[#ff9a82]'
      : 'text-[#f2c98e]';

  return (
    <article className="rounded-[1.6rem] border border-white/8 bg-[radial-gradient(circle_at_top_left,rgba(242,201,142,0.08),transparent_24%),linear-gradient(180deg,rgba(17,20,23,0.96),rgba(12,14,17,0.92))] p-5 shadow-[0_18px_60px_rgba(0,0,0,0.28)]">
      <div className="flex items-start justify-between gap-4">
        <div>
          <p className="text-[10px] font-black uppercase tracking-[0.32em] text-slate-500">{formatRaceDate(race.date, locale)}</p>
          <h3 className="mt-3 text-xl font-black uppercase tracking-tight text-white">{race.name}</h3>
        </div>
        <span className={`rounded-full border px-3 py-1 text-[10px] font-bold uppercase tracking-[0.18em] ${getPriorityTone(race.priority)}`}>
          {t('races.priorityBadge', { priority: race.priority })}
        </span>
      </div>

      <div className="mt-5 grid gap-3 sm:grid-cols-3">
        <StatChip icon={<DisciplineIcon size={14} />} label={mapRaceDisciplineLabel(race.discipline)} />
        <StatChip icon={<Bike size={14} />} label={t('races.distanceValue', { value: formatRaceDistance(race.distanceMeters, locale) })} />
        <StatChip icon={<Timer size={14} />} label={t(`races.syncStatus.${race.syncStatus}`)} valueClassName={syncTone} />
      </div>

      <div className="mt-5 flex items-center justify-between gap-3 text-sm">
        <p className="text-slate-400">
          {race.result ? t(`races.result.${race.result}`) : t('races.result.pending')}
        </p>
        {onEdit ? (
          <button
            type="button"
            onClick={() => onEdit(race)}
            className="rounded-full border border-white/10 bg-white/5 px-4 py-2 text-xs font-bold uppercase tracking-[0.18em] text-slate-200 transition hover:bg-white/10 hover:text-white"
          >
            {t('races.editRace')}
          </button>
        ) : null}
      </div>
    </article>
  );
}

function StatChip({ icon, label, valueClassName }: { icon: ReactNode; label: string; valueClassName?: string }) {
  return (
    <div className="flex items-center gap-2 rounded-2xl border border-white/6 bg-white/[0.03] px-3.5 py-3 text-sm text-slate-200">
      <span className="text-[#f2c98e]">{icon}</span>
      <span className={valueClassName}>{label}</span>
    </div>
  );
}

function getDisciplineIcon(discipline: Race['discipline']) {
  switch (discipline) {
    case 'mtb':
      return Mountain;
    case 'timetrial':
      return Flag;
    case 'gravel':
    case 'cyclocross':
    case 'road':
    default:
      return Trophy;
  }
}
