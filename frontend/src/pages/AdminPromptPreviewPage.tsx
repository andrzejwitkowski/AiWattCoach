import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { useAuth } from '../features/auth/context/AuthProvider';
import { useAdminPromptPreviewApi } from '../features/admin-prompt-preview/api';
import { MetaBar } from '../features/admin-prompt-preview/components/MetaBar';
import { SystemPromptSection } from '../features/admin-prompt-preview/components/SystemPromptSection';
import { StableContextSection } from '../features/admin-prompt-preview/components/StableContextSection';
import { VolatileContextSection } from '../features/admin-prompt-preview/components/VolatileContextSection';
import { ConversationSection } from '../features/admin-prompt-preview/components/ConversationSection';
import { ToolsSection } from '../features/admin-prompt-preview/components/ToolsSection';
import { ProviderMessagesSection } from '../features/admin-prompt-preview/components/ProviderMessagesSection';
import type { AdminPromptPreviewResponse } from '../features/admin-prompt-preview/types';

function todayIsoDate() {
  return new Date().toISOString().slice(0, 10);
}

export function AdminPromptPreviewPage() {
  const { t } = useTranslation();
  const { user } = useAuth();
  const {
    loadAdminPostWorkoutPromptPreview,
    loadAdminCalendarCoachPromptPreview,
    loadAdminMesoCyclePromptPreview,
    loadAdminTrainingPlanPromptPreview,
  } = useAdminPromptPreviewApi();
  const [userId, setUserId] = useState(user?.id ?? '');
  const [date, setDate] = useState(todayIsoDate());
  const [preview, setPreview] = useState<AdminPromptPreviewResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loadingSurface, setLoadingSurface] = useState<
    'post-workout' | 'calendar-coach' | 'meso-cycle' | 'training-plan' | null
  >(null);

  const maxDate = useMemo(() => todayIsoDate(), []);
  const formattedPreview = useMemo(
    () => (preview ? JSON.stringify(preview, null, 2) : ''),
    [preview],
  );
  const [showRawJson, setShowRawJson] = useState(false);

  const runPreview = async (
    surface: 'post-workout' | 'calendar-coach' | 'meso-cycle' | 'training-plan',
  ) => {
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
          : surface === 'calendar-coach'
            ? await loadAdminCalendarCoachPromptPreview(userId.trim(), date)
            : surface === 'meso-cycle'
              ? await loadAdminMesoCyclePromptPreview(userId.trim(), date)
              : await loadAdminTrainingPlanPromptPreview(userId.trim(), date);
      setPreview(response);
    } catch {
      setPreview(null);
      setError(t('adminPromptPreview.loadError'));
    } finally {
      setLoadingSurface(null);
    }
  };

  return (
    <section className="min-w-0 max-w-full space-y-6">
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
          <button
            type="button"
            disabled={loadingSurface !== null}
            onClick={() => void runPreview('meso-cycle')}
            className="rounded-xl border border-white/15 bg-white/5 px-4 py-3 text-sm font-semibold text-white transition hover:border-[#f2c98e]/45 hover:bg-white/10 disabled:cursor-not-allowed disabled:opacity-60"
          >
            {loadingSurface === 'meso-cycle'
              ? t('adminPromptPreview.loading')
              : t('adminPromptPreview.mesoCycleButton')}
          </button>
          <button
            type="button"
            disabled={loadingSurface !== null}
            onClick={() => void runPreview('training-plan')}
            className="rounded-xl border border-white/15 bg-white/5 px-4 py-3 text-sm font-semibold text-white transition hover:border-[#f2c98e]/45 hover:bg-white/10 disabled:cursor-not-allowed disabled:opacity-60"
          >
            {loadingSurface === 'training-plan'
              ? t('adminPromptPreview.loading')
              : t('adminPromptPreview.trainingPlanButton')}
          </button>
        </div>
      </div>

      {error ? (
        <p className="rounded-xl border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-200">
          {error}
        </p>
      ) : null}

      {preview && (
        <div className="min-w-0 max-w-full space-y-4">
          <div className="flex min-w-0 flex-wrap items-center justify-between gap-3">
            <div className="min-w-0 flex-1">
              <MetaBar meta={preview.meta} />
            </div>
            <button
              type="button"
              onClick={() => setShowRawJson(!showRawJson)}
              className="ml-auto rounded-lg border border-white/10 bg-white/5 px-3 py-1.5 text-xs font-semibold uppercase tracking-wider text-slate-400 hover:text-slate-200"
            >
              {showRawJson ? 'Structured view' : 'Raw JSON'}
            </button>
          </div>

          {showRawJson ? (
            <pre className="prompt-preview-text max-h-[80vh] overflow-auto rounded-2xl border border-white/10 bg-[#070b12] p-5 font-mono text-xs leading-5 text-slate-300">
              {formattedPreview}
            </pre>
          ) : (
            <div className="space-y-4">
              <SystemPromptSection systemPrompt={preview.request.systemPrompt} />
              <StableContextSection rawText={preview.request.stableContext} />
              <VolatileContextSection rawText={preview.request.volatileContext} />
              <ConversationSection conversation={preview.request.conversation} />
              <ToolsSection tools={preview.request.tools} toolChoice={preview.request.toolChoice} />
              <ProviderMessagesSection messages={preview.providerMessages} />
            </div>
          )}
        </div>
      )}

      {!preview && !error && (
        <div className="flex min-h-[40vh] items-center justify-center rounded-2xl border border-dashed border-white/10">
          <p className="text-sm text-slate-500">{t('adminPromptPreview.empty')}</p>
        </div>
      )}
    </section>
  );
}
