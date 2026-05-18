import {useEffect, useState} from 'react';
import {useTranslation} from 'react-i18next';

import {useApiBaseUrl} from '../../../lib/apiBaseUrl';
import {AuthenticationError, HttpError} from '../../../lib/httpClient';
import {syncPlannedWorkoutToIntervals, syncPlannedWorkoutToWahoo} from '../../intervals/api/intervals';
import type {IntervalEvent} from '../../intervals/types';
import {
  buildPlannedWorkoutBars,
  buildPlannedWorkoutChartIntervals,
  buildPlannedWorkoutPowerSeries,
  buildPlannedWorkoutStructureSections,
  formatDurationLabel,
} from '../workoutDetails';
import {MetricCard, WorkoutBars} from './WorkoutDetailPanelPrimitives';
import {PowerChart} from './WorkoutDetailPowerChart';

type PlannedWorkoutDetailModalProps = {
  event: IntervalEvent;
  syncingToIntervals: boolean;
  syncingToWahoo: boolean;
  onSyncingToIntervalsChange: (syncing: boolean) => void;
  onSyncingToWahooChange: (syncing: boolean) => void;
  onEventSynced: (event: IntervalEvent) => void;
  onSyncError: (message: string | null) => void;
};

export function PlannedWorkoutDetailModal({
  event,
  syncingToIntervals,
  syncingToWahoo,
  onSyncingToIntervalsChange,
  onSyncingToWahooChange,
  onEventSynced,
  onSyncError,
}: PlannedWorkoutDetailModalProps) {
  const apiBaseUrl = useApiBaseUrl();
  const {t} = useTranslation();
  const bars = buildPlannedWorkoutBars(event);
  const structureSections = buildPlannedWorkoutStructureSections(event);
  const rawWorkoutNoteLines = buildRawWorkoutNoteLines(event.eventDefinition.rawWorkoutDoc);
  const summary = event.eventDefinition.summary;
  const powerSeries = buildPlannedWorkoutPowerSeries(event);
  const chartIntervals = buildPlannedWorkoutChartIntervals(event);
  const [hoveredIntervalKey, setHoveredIntervalKey] = useState<string | null>(null);
  const [selectedIntervalKey, setSelectedIntervalKey] = useState<string | null>(null);
  const highlightedIntervalKey = chartIntervals.some((interval) => interval.id === (hoveredIntervalKey ?? selectedIntervalKey))
    ? (hoveredIntervalKey ?? selectedIntervalKey)
    : null;
  const activeInterval = chartIntervals.find((interval) => interval.id === highlightedIntervalKey) ?? null;
  const syncStatus = event.plannedSource === 'predicted' ? (event.syncStatus ?? 'unsynced') : null;
  const supervisorStatus = event.projectedWorkout?.supervisorStatus ?? null;
  const canSync = Boolean(event.projectedWorkout) && !event.restDay && !event.projectedWorkout?.restDay && !event.indoor;
  const canSyncToWahoo = canSync;
  const isInWahooSyncWindow = event.projectedWorkout ? isDateWithinWahooSyncWindow(event.projectedWorkout.date) : false;
  const syncDisabledReason = canSyncToWahoo && !isInWahooSyncWindow ? t('calendar.syncToWahooWindowMessage') : null;
  const syncing = syncingToIntervals || syncingToWahoo;

  useEffect(() => {
    setHoveredIntervalKey(null);
    setSelectedIntervalKey(null);
  }, [event.id, event.projectedWorkout?.projectedWorkoutId]);

  const handleIntervalsSync = async () => {
    if (!event.projectedWorkout || syncing) {
      return;
    }

    try {
      onSyncingToIntervalsChange(true);
      onSyncError(null);
      const syncedEvent = await syncPlannedWorkoutToIntervals(
        apiBaseUrl,
        event.projectedWorkout.operationKey,
        event.projectedWorkout.date,
      );
      onEventSynced(syncedEvent);
    } catch (error: unknown) {
      if (error instanceof AuthenticationError) {
        window.location.href = '/';
        return;
      }

      if (error instanceof HttpError && error.status === 422) {
        onSyncError(mapIntervalsSyncError(error, t));
        return;
      }

      if (error instanceof HttpError && error.status === 400) {
        onSyncError(t('calendar.syncToIntervalsFailedMessage'));
        return;
      }

      onSyncError(t('calendar.syncToIntervalsFailedMessage'));
    } finally {
      onSyncingToIntervalsChange(false);
    }
  };

  const handleWahooSync = async () => {
    if (!event.projectedWorkout || syncing || !isInWahooSyncWindow) {
      return;
    }

    try {
      onSyncingToWahooChange(true);
      onSyncError(null);
      const syncedEvent = await syncPlannedWorkoutToWahoo(
        apiBaseUrl,
        event.projectedWorkout.operationKey,
        event.projectedWorkout.date,
      );
      onEventSynced(syncedEvent);
    } catch (error: unknown) {
      if (error instanceof AuthenticationError) {
        window.location.href = '/';
        return;
      }

      if (error instanceof HttpError && error.status === 422) {
        onSyncError(t('calendar.wahooConnectionRequired'));
        return;
      }

      if (error instanceof HttpError && error.status === 400) {
        onSyncError(mapWahooSyncValidationError(error, t));
        return;
      }

      onSyncError(t('calendar.syncFailedMessage'));
    } finally {
      onSyncingToWahooChange(false);
    }
  };

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-center gap-3">
        {syncStatus ? (
          <span className="rounded-full border border-white/10 bg-white/5 px-3 py-1 text-[10px] font-bold uppercase tracking-[0.18em] text-slate-300">
            {syncBadgeLabel(syncStatus, t)}
          </span>
        ) : null}
        {syncStatus === 'modified' ? (
          <span className="rounded-full border border-[#ffb86a]/25 bg-[#ffb86a]/10 px-3 py-1 text-[10px] font-bold uppercase tracking-[0.18em] text-[#ffd7a1]">
            {t('calendar.scheduleChanged')}
          </span>
        ) : null}
        {supervisorStatus ? (
          <span className="rounded-full border border-[#00e3fd]/20 bg-[#00e3fd]/10 px-3 py-1 text-[10px] font-bold uppercase tracking-[0.18em] text-[#8eeeff]">
            {supervisorBadgeLabel(supervisorStatus, t)}
          </span>
        ) : null}
        {canSync ? (
          <>
            <button
              type="button"
              onClick={() => void handleIntervalsSync()}
              disabled={syncing}
              className="rounded-full border border-white/10 bg-white/5 px-4 py-2 text-xs font-bold uppercase tracking-[0.2em] text-slate-200 transition hover:bg-white/10 hover:text-white disabled:cursor-not-allowed disabled:opacity-60"
            >
              {syncingToIntervals ? t('calendar.syncingToIntervals') : t('calendar.syncToIntervals')}
            </button>
            {canSyncToWahoo ? (
              <button
                type="button"
                onClick={() => void handleWahooSync()}
                disabled={syncing || !isInWahooSyncWindow}
                className="rounded-full border border-white/10 bg-white/5 px-4 py-2 text-xs font-bold uppercase tracking-[0.2em] text-slate-200 transition hover:bg-white/10 hover:text-white disabled:cursor-not-allowed disabled:opacity-60"
              >
                {syncingToWahoo ? t('calendar.syncingToWahoo') : t('calendar.syncToWahoo')}
              </button>
            ) : null}
            {syncDisabledReason ? (
              <div className="rounded-full border border-amber-300/20 bg-amber-300/10 px-3 py-1 text-[10px] font-bold uppercase tracking-[0.18em] text-amber-100">
                {syncDisabledReason}
              </div>
            ) : null}
          </>
        ) : null}
      </div>
      <WorkoutBars bars={bars} />
      {powerSeries.length > 0 ? (
        <PowerChart
          activeInterval={activeInterval}
          activeIntervalKey={highlightedIntervalKey}
          formatMaxValueLabel={(value) =>
            t('calendar.powerChartMaxTargetLabel', {
              defaultValue: '{{value}}% FTP max target',
              value,
            })
          }
          formatValueLabel={(value) => `${value}% FTP`}
          intervals={chartIntervals}
          onHoverIntervalChange={setHoveredIntervalKey}
          onSelectIntervalChange={setSelectedIntervalKey}
          selectedIntervalKey={selectedIntervalKey}
          sampleDurationSeconds={5}
          title={t('calendar.powerChart')}
          values={powerSeries}
        />
      ) : null}
      <div className="grid gap-4 md:grid-cols-4">
        <MetricCard label={t('calendar.duration')} value={formatDurationLabel(summary.totalDurationSeconds)} />
        <MetricCard
          label="IF"
          value={summary.estimatedIntensityFactor !== null ? `${summary.estimatedIntensityFactor.toFixed(2)} IF` : '--'}
        />
        <MetricCard
          label="TSS"
          value={summary.estimatedTrainingStressScore !== null ? `${Math.round(summary.estimatedTrainingStressScore)} TSS` : '--'}
        />
        <MetricCard
          label="NP"
          value={summary.estimatedNormalizedPowerWatts !== null ? `${summary.estimatedNormalizedPowerWatts} W` : '--'}
        />
      </div>
      {structureSections.length > 0 ? (
        <div className="rounded-2xl border border-white/6 bg-[#171a1d] p-4">
          <p className="text-[10px] font-black uppercase tracking-[0.24em] text-slate-500">{t('calendar.workoutStructure')}</p>
          <div className="mt-4 space-y-3">
            {structureSections.map((section) => (
              <div
                key={section.id}
                className="rounded-xl border border-white/6 bg-white/[0.03] px-4 py-3"
              >
                <div className="flex items-center justify-between gap-4">
                  <p className="text-sm font-bold text-[#f9f9fd]">{section.label}</p>
                  {section.durationSeconds ? (
                    <p className="text-xs font-bold uppercase tracking-[0.18em] text-[#d2ff9a]">
                      {formatDurationLabel(section.durationSeconds)}
                    </p>
                  ) : null}
                </div>
                {section.steps.length > 0 ? (
                  <div className="mt-3 space-y-2">
                    {section.steps.map((step) => (
                      <div key={step.id} className="rounded-lg border border-white/6 bg-[#1d2125] px-3 py-2">
                        <div className="flex items-center justify-between gap-3">
                          <p className="text-sm font-semibold text-slate-100">{step.label}</p>
                          {step.durationSeconds ? (
                            <p className="text-[11px] font-bold uppercase tracking-[0.18em] text-slate-300">
                              {formatDurationLabel(step.durationSeconds)}
                            </p>
                          ) : null}
                        </div>
                        {step.detail ? (
                          <p className="mt-1 text-xs text-slate-400">{step.detail}</p>
                        ) : null}
                      </div>
                    ))}
                  </div>
                ) : null}
              </div>
            ))}
          </div>
        </div>
      ) : null}
      {rawWorkoutNoteLines.length > 0 ? (
        <div className="rounded-2xl border border-white/6 bg-[#171a1d] p-4">
          <p className="text-[10px] font-black uppercase tracking-[0.24em] text-slate-500">{t('calendar.workoutNotes')}</p>
          <div className="mt-4 space-y-2">
            {rawWorkoutNoteLines.map((line, index) => (
              <p key={`${index}-${line}`} className="text-sm text-slate-300">{line}</p>
            ))}
          </div>
        </div>
      ) : null}
      {syncStatus === 'failed' ? (
        <div className="rounded-2xl border border-amber-300/20 bg-amber-300/10 p-4 text-sm text-amber-100">
          {t('calendar.syncFailedBannerMessage')}
        </div>
      ) : null}
    </div>
  );
}

function isDateWithinWahooSyncWindow(dateKey: string): boolean {
  const today = new Date();
  const earliestAllowedDate = toUtcDateKey(today);
  const latestAllowedDate = new Date(Date.UTC(today.getUTCFullYear(), today.getUTCMonth(), today.getUTCDate()));
  latestAllowedDate.setUTCDate(latestAllowedDate.getUTCDate() + 6);

  return dateKey >= earliestAllowedDate && dateKey <= toUtcDateKey(latestAllowedDate);
}

function toUtcDateKey(date: Date): string {
  const year = date.getUTCFullYear();
  const month = String(date.getUTCMonth() + 1).padStart(2, '0');
  const day = String(date.getUTCDate()).padStart(2, '0');
  return `${year}-${month}-${day}`;
}

function mapWahooSyncValidationError(
  error: HttpError,
  t: ReturnType<typeof useTranslation>['t'],
): string {
  const errorCode = typeof error.body === 'object' && error.body !== null && 'code' in error.body
    ? (error.body as { code?: unknown }).code
    : undefined;

  if (errorCode === 'wahoo_window_out_of_range') {
    return t('calendar.syncToWahooWindowMessage');
  }

  if (errorCode === 'wahoo_ftp_required') {
    return t('calendar.syncToWahooFtpRequired');
  }

  if (errorCode === 'invalid_date_format') {
    return t('calendar.syncToWahooInvalidDate');
  }

  if (error.message === 'Only planned workouts scheduled between today and the next 6 days can sync to Wahoo') {
    return t('calendar.syncToWahooWindowMessage');
  }

  if (error.message === 'Set your cycling FTP in Settings before syncing to Wahoo') {
    return t('calendar.syncToWahooFtpRequired');
  }

  if (error.message === 'planned workout date must be in YYYY-MM-DD format') {
    return t('calendar.syncToWahooInvalidDate');
  }

  return t('calendar.syncFailedMessage');
}

function mapIntervalsSyncError(
  error: HttpError,
  t: ReturnType<typeof useTranslation>['t'],
): string {
  const errorCode = typeof error.body === 'object' && error.body !== null && 'code' in error.body
    ? (error.body as { code?: unknown }).code
    : undefined;

  if (errorCode === 'intervals_not_connected' || errorCode === 'credentials_not_configured') {
    return t('calendar.intervalsConnectionRequired');
  }

  return t('calendar.syncToIntervalsFailedMessage');
}

function buildRawWorkoutNoteLines(rawWorkoutDoc: string | null): string[] {
  const lines = (rawWorkoutDoc ?? '')
    .split('\n')
    .map((line) => line.replace(/^[-*]\s*/, '').trim())
    .filter(Boolean)
    .filter((line) => !/^\d+\s*x\b/i.test(line) && !/%\s*ftp\b/i.test(line));

  return lines.length > 1 ? lines : [];
}

function syncBadgeLabel(syncStatus: NonNullable<IntervalEvent['syncStatus']>, t: ReturnType<typeof useTranslation>['t']) {
  switch (syncStatus) {
    case 'synced':
      return t('calendar.synced');
    case 'modified':
      return t('calendar.modified');
    case 'failed':
      return t('calendar.syncFailed');
    case 'pending':
      return t('calendar.syncPending');
    default:
      return t('calendar.notSynced');
  }
}

function supervisorBadgeLabel(
  status: NonNullable<NonNullable<IntervalEvent['projectedWorkout']>['supervisorStatus']>,
  t: ReturnType<typeof useTranslation>['t'],
) {
  switch (status) {
    case 'accepted':
      return t('calendar.supervisorAccepted');
    case 'replaced':
      return t('calendar.supervisorReplaced');
    case 'failed':
      return t('calendar.supervisorFailed');
    case 'pending':
    default:
      return t('calendar.supervisorPending');
  }
}
