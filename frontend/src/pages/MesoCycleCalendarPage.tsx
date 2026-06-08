import { Link } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { ArrowLeft, Loader2, RefreshCw, Sparkles } from 'lucide-react';

import { useSettings } from '../features/settings/context/SettingsContext';
import { useMesoCycleCalendar } from '../features/meso-cycle/hooks/useMesoCycleCalendar';

export function MesoCycleCalendarPage() {
  const { t } = useTranslation();
  const settingsCtx = useSettings();
  const {
    status,
    orderedDates,
    daysByDate,
    isLoading,
    isGenerating,
    error,
    windowRange,
    canGenerate,
    refresh,
    generate,
  } = useMesoCycleCalendar({
    settings: settingsCtx.settings,
  });

  return (
    <div className="mx-auto flex w-full max-w-5xl flex-col gap-6 px-4 py-6 md:px-8">
        <div className="flex flex-col gap-4 md:flex-row md:items-center md:justify-between">
          <div className="space-y-2">
            <Link
              className="inline-flex items-center gap-2 text-sm text-slate-400 transition hover:text-white"
              to="/calendar"
            >
              <ArrowLeft className="h-4 w-4" />
              {t('mesoCycle.backToCalendar')}
            </Link>
            <div>
              <p className="text-xs uppercase tracking-[0.25em] text-slate-500">{t('mesoCycle.kicker')}</p>
              <h2 className="text-2xl font-semibold text-white">{t('mesoCycle.title')}</h2>
              <p className="mt-1 max-w-2xl text-sm text-slate-400">{t('mesoCycle.description')}</p>
            </div>
          </div>

          <div className="flex flex-wrap items-center gap-3">
            <button
              className="inline-flex items-center gap-2 rounded-lg border border-white/10 px-4 py-2 text-sm text-slate-200 transition hover:border-white/20 hover:bg-white/5 disabled:opacity-50"
              disabled={isGenerating}
              onClick={() => {
                void refresh();
              }}
              type="button"
            >
              <RefreshCw
                className={`h-4 w-4 ${isLoading ? 'animate-spin motion-reduce:animate-none' : ''}`}
              />
              {t('mesoCycle.refresh')}
            </button>
            <button
              aria-busy={isGenerating}
              className="inline-flex items-center gap-2 rounded-lg bg-cyan-500 px-4 py-2 text-sm font-medium text-slate-950 transition hover:bg-cyan-400 disabled:opacity-50"
              disabled={!canGenerate || isGenerating}
              onClick={() => {
                void generate();
              }}
              type="button"
            >
              {isGenerating ? (
                <Loader2 className="h-4 w-4 animate-spin motion-reduce:animate-none" />
              ) : (
                <Sparkles className="h-4 w-4" />
              )}
              {isGenerating ? t('mesoCycle.generating') : t('mesoCycle.generate')}
            </button>
          </div>
        </div>

        {!canGenerate ? (
          <div
            className="rounded-lg border border-amber-500/30 bg-amber-500/10 px-4 py-3 text-sm text-amber-100"
            role="alert"
          >
            {t('mesoCycle.configureAiFirst')}
          </div>
        ) : null}

        {windowRange ? (
          <p className="text-sm text-slate-300">
            {t('mesoCycle.windowLabel', { from: windowRange.from, to: windowRange.to })}
          </p>
        ) : (
          <p className="text-sm text-slate-400">{t('mesoCycle.noWindow')}</p>
        )}

        {status?.latestOperation?.failureMessage ? (
          <div
            className="rounded-lg border border-rose-500/30 bg-rose-500/10 px-4 py-3 text-sm text-rose-100"
            role="alert"
          >
            {status.latestOperation.failureMessage}
          </div>
        ) : null}

        {error ? (
          <div
            className="rounded-lg border border-rose-500/30 bg-rose-500/10 px-4 py-3 text-sm text-rose-100"
            role="alert"
          >
            {error}
          </div>
        ) : null}

        {isLoading && orderedDates.length === 0 ? (
          <div className="flex items-center gap-2 text-sm text-slate-400">
            <Loader2 className="h-4 w-4 animate-spin motion-reduce:animate-none" />
            {t('mesoCycle.loading')}
          </div>
        ) : (
          <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
            {orderedDates.map((date) => {
              const day = daysByDate.get(date);
              const isOutdated = day?.overlapStatus === 'outdated';
              const title = day?.restDay
                ? day.restDayReason ?? t('mesoCycle.restDay')
                : day?.name ?? t('mesoCycle.noPlanYet');

              return (
                <article
                  className={`rounded-xl border px-4 py-3 ${
                    isOutdated
                      ? 'border-amber-500/30 bg-amber-500/5'
                      : 'border-white/10 bg-white/[0.03]'
                  }`}
                  key={date}
                >
                  <div className="flex items-center justify-between gap-3">
                    <p className="text-sm font-medium text-white">{date}</p>
                    {isOutdated ? (
                      <span className="rounded-full bg-amber-500/15 px-2 py-0.5 text-[10px] uppercase tracking-wide text-amber-200">
                        {t('mesoCycle.outdated')}
                      </span>
                    ) : null}
                  </div>
                  <p className="mt-2 text-sm text-slate-300">{title}</p>
                </article>
              );
            })}
          </div>
        )}
      </div>
  );
}
