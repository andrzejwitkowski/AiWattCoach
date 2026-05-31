import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { useAdminPromptPreviewApi } from '../features/admin-prompt-preview/api';
import type { AdminPromptPreviewResponse } from '../features/admin-prompt-preview/types';

function todayIsoDate() {
  return new Date().toISOString().slice(0, 10);
}

export function AdminPromptPreviewPage() {
  const { t } = useTranslation();
  const { loadAdminPostWorkoutPromptPreview, loadAdminCalendarCoachPromptPreview } =
    useAdminPromptPreviewApi();
  const [userId, setUserId] = useState('');
  const [date, setDate] = useState(todayIsoDate());
  const [preview, setPreview] = useState<AdminPromptPreviewResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loadingSurface, setLoadingSurface] = useState<'post-workout' | 'calendar-coach' | null>(
    null,
  );

  const maxDate = useMemo(() => todayIsoDate(), []);
  const formattedPreview = useMemo(
    () => (preview ? JSON.stringify(preview, null, 2) : ''),
    [preview],
  );

  const runPreview = async (surface: 'post-workout' | 'calendar-coach') => {
    if (!userId.trim() || !date) {
      setError(t('adminPromptPreview.validation'));
      return;
    }
    if (date > maxDate) {
      setError(t('adminPromptPreview.futureDate'));
      return;
    }

    setLoadingSurface(surface);
    setError(null);
    try {
      const response =
        surface === 'post-workout'
          ? await loadAdminPostWorkoutPromptPreview(userId.trim(), date)
          : await loadAdminCalendarCoachPromptPreview(userId.trim(), date);
      setPreview(response);
    } catch {
      setPreview(null);
      setError(t('adminPromptPreview.loadError'));
    } finally {
      setLoadingSurface(null);
    }
  };

  return (
    <section className="space-y-6">
      <div>
        <h1 className="text-2xl font-semibold text-white">{t('adminPromptPreview.title')}</h1>
        <p className="mt-2 max-w-3xl text-sm text-slate-400">{t('adminPromptPreview.subtitle')}</p>
      </div>

      <div className="grid gap-4 rounded-2xl border border-white/10 bg-white/5 p-5 md:grid-cols-[minmax(0,1fr)_12rem_auto] md:items-end">
        <label className="grid gap-2 text-sm text-slate-300">
          <span>{t('adminPromptPreview.userIdLabel')}</span>
          <input
            value={userId}
            onChange={(event) => setUserId(event.target.value)}
            className="rounded-xl border border-white/10 bg-[#0a0f1a] px-4 py-3 text-sm text-white outline-none transition focus:border-[#f2c98e]/45"
            placeholder={t('adminPromptPreview.userIdPlaceholder')}
          />
        </label>

        <label className="grid gap-2 text-sm text-slate-300">
          <span>{t('adminPromptPreview.dateLabel')}</span>
          <input
            type="date"
            max={maxDate}
            value={date}
            onChange={(event) => setDate(event.target.value)}
            className="rounded-xl border border-white/10 bg-[#0a0f1a] px-4 py-3 text-sm text-white outline-none transition focus:border-[#f2c98e]/45"
          />
        </label>

        <div className="flex flex-col gap-2 sm:flex-row md:flex-col">
          <button
            type="button"
            disabled={loadingSurface !== null}
            onClick={() => void runPreview('post-workout')}
            className="rounded-xl bg-[#f2c98e] px-4 py-3 text-sm font-semibold text-[#0a0f1a] transition hover:bg-[#f7d9ad] disabled:cursor-not-allowed disabled:opacity-60"
          >
            {loadingSurface === 'post-workout'
              ? t('adminPromptPreview.loading')
              : t('adminPromptPreview.postWorkoutButton')}
          </button>
          <button
            type="button"
            disabled={loadingSurface !== null}
            onClick={() => void runPreview('calendar-coach')}
            className="rounded-xl border border-white/15 bg-white/5 px-4 py-3 text-sm font-semibold text-white transition hover:border-[#f2c98e]/45 hover:bg-white/10 disabled:cursor-not-allowed disabled:opacity-60"
          >
            {loadingSurface === 'calendar-coach'
              ? t('adminPromptPreview.loading')
              : t('adminPromptPreview.calendarCoachButton')}
          </button>
        </div>
      </div>

      {error ? (
        <p className="rounded-xl border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-200">
          {error}
        </p>
      ) : null}

      <div className="min-h-[60vh] rounded-2xl border border-white/10 bg-[#070b12] p-4">
        {preview ? (
          <div className="mb-3 text-xs uppercase tracking-[0.2em] text-slate-500">
            {preview.meta.surface}
            {preview.meta.selectedWorkoutId
              ? ` · ${preview.meta.selectedWorkoutId}`
              : ''}
          </div>
        ) : null}
        <pre className="max-h-[70vh] overflow-auto whitespace-pre-wrap break-words font-mono text-xs leading-5 text-slate-200">
          {formattedPreview || t('adminPromptPreview.empty')}
        </pre>
      </div>
    </section>
  );
}
