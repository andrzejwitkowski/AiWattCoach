import { useEffect, useMemo, useState } from 'react';
import { CalendarRange, Clock3 } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { updateAvailability } from '../api/settings';
import { hasExplicitAvailabilityWeek, isAvailabilityConfigured, type AvailabilityDay, type UserSettingsResponse } from '../types';

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

function normalizeDays(days: AvailabilityDay[]): AvailabilityDay[] {
  return days.map((day) => ({
    ...day,
    maxDurationMinutes: day.available ? (day.maxDurationMinutes ?? 60) : null,
  }));
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

  return (
    <div className="rounded-2xl border border-white/10 bg-white/5 p-6 backdrop-blur">
      <div className="flex flex-wrap items-start justify-between gap-4">
        <div>
          <div className="flex items-center gap-3">
            <CalendarRange size={20} className="text-emerald-300" />
            <h2 className="text-2xl font-bold text-white">{t('availability.title')}</h2>
          </div>
          <p className="mt-1 text-sm text-slate-400">
            {t('availability.description')}
          </p>
        </div>
        <div className="rounded-xl border border-emerald-400/20 bg-emerald-400/10 px-4 py-3 text-right">
          <p className="text-[10px] uppercase tracking-[0.18em] text-emerald-200/70">{t('availability.coachReadiness')}</p>
          <p className="mt-1 text-sm font-medium text-emerald-100">
            {configured ? t('availability.configured') : t('availability.unconfigured')}
          </p>
        </div>
      </div>

      <div className="mt-6 space-y-3">
        {days.map((day) => {
          const weekdayLabel = t(WEEKDAY_LABEL_KEYS[day.weekday]);

          return (
            <div
              key={day.weekday}
              className="grid gap-3 rounded-2xl border border-white/10 bg-black/20 px-4 py-4 md:grid-cols-[1.2fr_auto_12rem] md:items-center"
            >
              <div>
                <p className="text-sm font-semibold text-white">{weekdayLabel}</p>
                <p className="text-xs text-slate-400">
                  {day.available ? t('availability.dayAvailable') : t('availability.dayUnavailable')}
                </p>
            </div>

            <button
              type="button"
              role="switch"
              aria-checked={day.available}
              aria-label={`${weekdayLabel} availability`}
              className={[
                'relative h-7 w-14 rounded-full transition focus:outline-none focus:ring-2 focus:ring-emerald-300/40',
                day.available ? 'bg-emerald-300' : 'bg-slate-700',
              ].join(' ')}
              onClick={() => {
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
              <span
                className={[
                  'absolute top-1 h-5 w-5 rounded-full bg-white transition-transform',
                  day.available ? 'translate-x-8' : 'translate-x-1',
                ].join(' ')}
              />
            </button>

            <label className="flex items-center gap-2 text-sm text-slate-300">
              <Clock3 size={16} className="text-slate-500" />
              <span className="sr-only">{`${weekdayLabel} max duration`}</span>
              <select
                aria-label={`${weekdayLabel} max duration`}
                className="w-full rounded-xl border border-white/10 bg-white/5 px-3 py-2 text-sm text-white outline-none focus:border-emerald-300/40"
                disabled={!day.available}
                value={day.maxDurationMinutes ?? 60}
                onChange={(event) => {
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
                    {minutes} min
                  </option>
                ))}
              </select>
            </label>
            </div>
          );
        })}
      </div>

      {saveError ? (
        <div
          role="alert"
          className="mt-4 rounded-xl border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-300"
        >
          {saveError}
        </div>
      ) : null}

      <button
        type="button"
        className="mt-6 w-full rounded-xl border border-emerald-300/30 bg-emerald-300/15 px-4 py-3 text-sm font-semibold uppercase tracking-[0.16em] text-emerald-100 transition hover:bg-emerald-300/25 disabled:cursor-not-allowed disabled:opacity-60"
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
