import { Trophy, X } from 'lucide-react';
import { useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';

import { useDialogFocusTrap } from '../../../lib/useDialogFocusTrap';
import { formatRaceSubtitle, mapRaceDisciplineLabel } from '../racePresentation';
import type { CalendarRaceLabel } from '../types';
import { formatRaceDate, formatRaceDistance } from '../../races/utils';

type RaceDayDetailModalProps = {
  selection: CalendarRaceLabel | null;
  onClose: () => void;
};

export function RaceDayDetailModal({ selection, onClose }: RaceDayDetailModalProps) {
  const { t, i18n } = useTranslation();
  const locale = i18n.resolvedLanguage ?? i18n.language ?? 'en';
  const dialogRef = useRef<HTMLDivElement>(null);
  const closeButtonRef = useRef<HTMLButtonElement>(null);

  useDialogFocusTrap(Boolean(selection), dialogRef, closeButtonRef);

  useEffect(() => {
    if (!selection) {
      return undefined;
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        onClose();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => {
      window.removeEventListener('keydown', handleKeyDown);
    };
  }, [selection, onClose]);

  if (!selection) {
    return null;
  }

  const race = selection.payload;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/65 px-4 py-6 backdrop-blur-sm" onClick={onClose}>
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby="race-day-title"
        tabIndex={-1}
        className="w-full max-w-3xl overflow-hidden rounded-[1.5rem] border border-white/8 bg-[#111417] shadow-[0_24px_80px_rgba(0,0,0,0.5)]"
        onClick={(event) => event.stopPropagation()}
      >
        <div className="flex items-center justify-between border-b border-white/6 px-6 py-4 md:px-8">
          <div>
            <p className="text-[10px] font-black uppercase tracking-[0.28em] text-[#f2c98e]">{t('calendar.raceDay')}</p>
            <h2 id="race-day-title" className="mt-2 text-2xl font-black uppercase tracking-tight text-[#f9f9fd] md:text-3xl">
              {race.name}
            </h2>
            <p className="mt-2 text-sm font-semibold text-slate-400">{formatRaceDate(race.date, locale)}</p>
          </div>
          <button
            ref={closeButtonRef}
            type="button"
            onClick={onClose}
            aria-label={t('calendar.closeRaceDetails')}
            className="rounded-full border border-white/10 bg-white/5 p-2 text-slate-300 transition hover:bg-white/10 hover:text-white"
          >
            <X size={18} />
          </button>
        </div>

        <div className="space-y-6 px-6 py-6 md:px-8">
          <div className="flex items-center gap-3 rounded-2xl border border-[#cda56b]/20 bg-[#201810]/70 px-4 py-4 text-[#f6deb1]">
            <span className="flex h-11 w-11 items-center justify-center rounded-2xl border border-[#cda56b]/20 bg-[#2d2115]">
              <Trophy size={18} />
            </span>
            <div>
              <p className="text-[10px] font-black uppercase tracking-[0.22em] text-[#d7b37b]">{mapRaceDisciplineLabel(race.discipline, t)}</p>
              <p className="mt-1 text-sm font-semibold text-[#f9f2e8]">
                {formatRaceSubtitle(race, t)}
              </p>
            </div>
          </div>

          <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
            <StatCard label={t('calendar.raceDistance')} value={t('races.distanceValue', { value: formatRaceDistance(race.distanceMeters, locale) })} />
            <StatCard label={t('calendar.raceDiscipline')} value={mapRaceDisciplineLabel(race.discipline, t)} />
            <StatCard label={t('calendar.racePriority')} value={t('calendar.priorityLabel', { priority: race.priority })} />
            <StatCard label={t('calendar.raceSyncStatus')} value={t(`races.syncStatus.${race.syncStatus}`)} />
          </div>
        </div>
      </div>
    </div>
  );
}

function StatCard({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-2xl border border-white/8 bg-white/[0.03] px-4 py-4">
      <p className="text-[10px] font-black uppercase tracking-[0.22em] text-slate-500">{label}</p>
      <p className="mt-2 text-sm font-semibold text-[#f9f9fd]">{value}</p>
    </div>
  );
}
