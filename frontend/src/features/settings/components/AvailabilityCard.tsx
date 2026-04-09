import { useEffect, useMemo, useState } from 'react';
import { CalendarRange, Check, Clock3 } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { updateAvailability } from '../api/settings';
import {
  hasExplicitAvailabilityWeek,
  isAvailabilityConfigured,
  type AvailabilityDay,
  type UserSettingsResponse,
} from '../types';

type AvailabilityCardProps = {
  settings: UserSettingsResponse;
  apiBaseUrl: string;
  onSave: (updatedSettings?: UserSettingsResponse) => void;
};

const DURATION_OPTIONS = [30, 60, 90, 120, 150, 180, 210, 240, 270, 300] as const;

const WEEKDAY_LABEL_KEYS: Record<AvailabilityDay['weekday'], string> = {
  mon: 'calendar.monday',
  tue: 'calendar.tuesday',
  wed: 'calendar.wednesday',
  thu: 'calendar.thursday',
  fri: 'calendar.friday',
  sat: 'calendar.saturday',
  sun: 'calendar.sunday',
};

const WEEKDAY_SHORT_KEYS: Record<AvailabilityDay['weekday'], string> = {
  mon: 'availability.shortMon',
  tue: 'availability.shortTue',
  wed: 'availability.shortWed',
  thu: 'availability.shortThu',
  fri: 'availability.shortFri',
  sat: 'availability.shortSat',
  sun: 'availability.shortSun',
};

function normalizeDays(days: AvailabilityDay[]): AvailabilityDay[] {
  return days.map((day) => ({
    ...day,
    maxDurationMinutes: day.available ? (day.maxDurationMinutes ?? 60) : null,
  }));
}

function formatDurationLabel(minutes: number, t: (key: string, options?: Record<string, unknown>) => string) {
  if (minutes % 60 === 0) {
    return t('availability.durationHours', { count: minutes / 60 });
  }

  if (minutes > 60) {
    const hours = Math.floor(minutes / 60);
    const restMinutes = minutes % 60;
    return t('availability.durationHoursMinutes', { hours, minutes: restMinutes });
  }

  return t('availability.durationMinutes', { count: minutes });
}

export function AvailabilityCard({ settings, apiBaseUrl, onSave }: AvailabilityCardProps) {
  const { t } = useTranslation();
  const [days, setDays] = useState<AvailabilityDay[]>(() => normalizeDays(settings.availability.days));
  const [isSaving, setIsSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [hasUnsavedChanges, setHasUnsavedChanges] = useState(false);

  useEffect(() => {
    if (hasUnsavedChanges) {
      return;
    }

    setDays(normalizeDays(settings.availability.days));
  }, [hasUnsavedChanges, settings.availability.days]);

  const configured = useMemo(
    () => isAvailabilityConfigured({ configured: days.some((day) => day.available), days }),
    [days],
  );

  async function handleSave() {
    setIsSaving(true);
    setSaveError(null);

    try {
      const updatedSettings = await updateAvailability(apiBaseUrl, {
        days: days.map((day) => ({
          weekday: day.weekday,
          available: day.available,
          maxDurationMinutes: day.available ? day.maxDurationMinutes : null,
        })),
      });
      setDays(normalizeDays(updatedSettings.availability.days));
      setHasUnsavedChanges(false);
      onSave(updatedSettings);
    } catch (error) {
      setSaveError(error instanceof Error ? error.message : t('availability.saveError'));
    } finally {
      setIsSaving(false);
    }
  }

  function weekdayAvailabilityAriaLabel(weekdayLabel: string) {
    return t('availability.dayAvailabilityAriaLabel', { weekday: weekdayLabel });
  }

  function weekdayMaxDurationAriaLabel(weekdayLabel: string) {
    return t('availability.maxDurationAriaLabel', { weekday: weekdayLabel });
  }

  function blockDraftChangesWhileSaving(): boolean {
    return isSaving;
  }

  return (
    <div className="rounded-[1.8rem] border border-white/10 bg-[linear-gradient(180deg,rgba(19,24,34,0.96),rgba(14,18,27,0.96))] p-4 shadow-[0_24px_80px_rgba(0,0,0,0.32)] backdrop-blur">
      <div className="flex flex-wrap items-start justify-between gap-3 border-b border-white/8 pb-4">
        <div>
          <div className="flex items-center gap-3">
            <div className="flex h-10 w-10 items-center justify-center rounded-2xl border border-emerald-300/20 bg-emerald-300/10 text-emerald-200">
              <CalendarRange size={17} />
            </div>
            <div>
              <h2 className="text-2xl font-bold text-white">{t('availability.title')}</h2>
              <p className="mt-1 max-w-2xl text-sm text-slate-400">{t('availability.description')}</p>
            </div>
          </div>
        </div>
        <div className="min-w-[14rem] rounded-2xl border border-emerald-400/20 bg-emerald-400/10 px-3.5 py-2.5 text-right">
          <p className="text-[10px] uppercase tracking-[0.28em] text-emerald-200/70">{t('availability.coachReadiness')}</p>
          <p className="mt-1 text-sm font-medium text-emerald-100">
            {configured ? t('availability.configured') : t('availability.unconfigured')}
          </p>
        </div>
      </div>

      <div className="mt-4 grid grid-cols-2 gap-2.5 md:grid-cols-4 xl:grid-cols-7">
        {days.map((day) => {
          const weekdayLabel = t(WEEKDAY_LABEL_KEYS[day.weekday]);
          const shortWeekdayLabel = t(WEEKDAY_SHORT_KEYS[day.weekday]);
          const durationLabel = formatDurationLabel(day.maxDurationMinutes ?? 60, t);

          return (
            <article
              key={day.weekday}
              className={[
                'flex min-h-[12rem] flex-col rounded-[1.2rem] border px-3.5 py-3.5 transition',
                day.available
                  ? 'border-emerald-300/25 bg-[linear-gradient(180deg,rgba(14,18,24,0.96),rgba(11,15,20,0.92))] shadow-[0_18px_45px_rgba(16,185,129,0.08)]'
                  : 'border-white/8 bg-[linear-gradient(180deg,rgba(11,14,20,0.94),rgba(9,12,18,0.9))]',
              ].join(' ')}
            >
              <div className="flex items-start justify-between gap-3">
                <div>
                  <p className="text-[1rem] font-black uppercase tracking-[0.08em] text-white/88">{shortWeekdayLabel}</p>
                  <p className="mt-1 text-[10px] uppercase tracking-[0.22em] text-slate-500">{weekdayLabel}</p>
                </div>
                <button
                  type="button"
                  role="switch"
                  aria-checked={day.available}
                  aria-label={weekdayAvailabilityAriaLabel(weekdayLabel)}
                  disabled={isSaving}
                  className={[
                    'group relative flex h-6.5 w-6.5 items-center justify-center rounded-[0.75rem] border transition focus:outline-none focus:ring-2 focus:ring-lime-300/40',
                    day.available
                      ? 'border-lime-300/40 bg-lime-300 text-slate-950 shadow-[0_0_0_3px_rgba(190,242,100,0.12)]'
                      : 'border-white/8 bg-white/5 text-transparent hover:border-white/16 hover:bg-white/8',
                  ].join(' ')}
                  onClick={() => {
                    if (blockDraftChangesWhileSaving()) {
                      return;
                    }

                    setDays((current) =>
                      current.map((entry) =>
                        entry.weekday === day.weekday
                          ? {
                              ...entry,
                              available: !entry.available,
                              maxDurationMinutes: !entry.available ? (entry.maxDurationMinutes ?? 60) : null,
                            }
                          : entry,
                      ),
                    );
                    setHasUnsavedChanges(true);
                    setSaveError(null);
                  }}
                >
                  <Check size={13} className={day.available ? 'opacity-100' : 'opacity-0 transition-opacity group-hover:opacity-40'} />
                </button>
              </div>

              <div className="mt-4 flex flex-1 flex-col justify-between gap-4">
                <div>
                  <p className="text-[10px] font-semibold uppercase tracking-[0.22em] text-slate-500">
                    {t('availability.availableLabel')}
                  </p>
                  <p className="mt-1.5 max-w-[8ch] text-[0.94rem] font-extrabold uppercase leading-[1.05] text-slate-100">
                    {t('availability.availableValue')}
                  </p>
                </div>

                <div>
                  <p className="text-[10px] font-semibold uppercase tracking-[0.22em] text-slate-500">
                    {t('availability.maxDurationLabel')}
                  </p>
                  <label className="mt-2 flex items-center gap-1.5 text-sm text-slate-300">
                    <Clock3 size={13} className="text-slate-500" />
                    <span className="sr-only">{weekdayMaxDurationAriaLabel(weekdayLabel)}</span>
                    <select
                      aria-label={weekdayMaxDurationAriaLabel(weekdayLabel)}
                      className="w-full rounded-xl border border-white/8 bg-white/6 px-2.5 py-2 text-sm font-semibold text-white outline-none transition focus:border-emerald-300/40 disabled:cursor-not-allowed disabled:opacity-55"
                      disabled={isSaving || !day.available}
                      value={day.maxDurationMinutes ?? 60}
                      onChange={(event) => {
                        if (blockDraftChangesWhileSaving()) {
                          return;
                        }

                        const value = Number(event.target.value);
                        setDays((current) =>
                          current.map((entry) =>
                            entry.weekday === day.weekday
                              ? { ...entry, maxDurationMinutes: value }
                              : entry,
                          ),
                        );
                        setHasUnsavedChanges(true);
                        setSaveError(null);
                      }}
                    >
                      {DURATION_OPTIONS.map((minutes) => (
                        <option key={minutes} value={minutes}>
                          {formatDurationLabel(minutes, t)}
                        </option>
                      ))}
                    </select>
                  </label>
                  <p className="mt-1 text-[10px] leading-4 text-slate-500">{day.available ? durationLabel : t('availability.durationDisabled')}</p>
                </div>
              </div>
            </article>
          );
        })}
      </div>

      {saveError ? (
        <div
          role="alert"
          className="mt-4 rounded-2xl border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-300"
        >
          {saveError}
        </div>
      ) : null}

      <button
        type="button"
        className="mt-4 w-full rounded-2xl border border-emerald-300/25 bg-[linear-gradient(180deg,rgba(64,92,88,0.56),rgba(35,56,54,0.78))] px-4 py-3 text-sm font-semibold uppercase tracking-[0.24em] text-emerald-50 transition hover:bg-[linear-gradient(180deg,rgba(83,120,113,0.72),rgba(44,72,69,0.92))] disabled:cursor-not-allowed disabled:opacity-60"
        onClick={() => {
          void handleSave();
        }}
        disabled={isSaving || !hasExplicitAvailabilityWeek(days)}
      >
        {isSaving ? t('availability.saving') : t('availability.save')}
      </button>
    </div>
  );
}
