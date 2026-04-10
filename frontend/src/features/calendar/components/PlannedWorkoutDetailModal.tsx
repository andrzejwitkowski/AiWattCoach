import {useEffect, useState} from 'react';
import {useTranslation} from 'react-i18next';

import {AuthenticationError, HttpError} from '../../../lib/httpClient';
import {syncPlannedWorkout} from '../../intervals/api/intervals';
import type {IntervalEvent} from '../../intervals/types';
import {
  buildPlannedWorkoutBars,
  buildPlannedWorkoutChartIntervals,
  buildPlannedWorkoutPowerSeries,
  buildPlannedWorkoutStructureItems,
  formatDurationLabel,
} from '../workoutDetails';
import {MetricCard, WorkoutBars} from './WorkoutDetailPanelPrimitives';
import {PowerChart} from './WorkoutDetailPowerChart';

type PlannedWorkoutDetailModalProps = {
  apiBaseUrl: string;
  event: IntervalEvent;
  syncing: boolean;
  onSyncingChange: (syncing: boolean) => void;
  onEventSynced: (event: IntervalEvent) => void;
  onSyncError: (message: string | null) => void;
};

export function PlannedWorkoutDetailModal({
  apiBaseUrl,
  event,
  syncing,
  onSyncingChange,
  onEventSynced,
  onSyncError,
}: PlannedWorkoutDetailModalProps) {
  const {t} = useTranslation();
  const bars = buildPlannedWorkoutBars(event);
  const structureItems = buildPlannedWorkoutStructureItems(event);
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
  const canSync = Boolean(event.projectedWorkout);

  useEffect(() => {
    setHoveredIntervalKey(null);
    setSelectedIntervalKey(null);
  }, [event.id, event.projectedWorkout?.projectedWorkoutId]);

  const handleSync = async () => {
    if (!event.projectedWorkout || syncing) {
      return;
    }

    try {
      onSyncingChange(true);
      onSyncError(null);
      const syncedEvent = await syncPlannedWorkout(
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
        onSyncError(t('calendar.connectionRequired'));
        return;
      }

      onSyncError(t('calendar.syncFailedMessage'));
    } finally {
      onSyncingChange(false);
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
        {canSync ? (
          <button
            type="button"
            onClick={() => void handleSync()}
            disabled={syncing}
            className="rounded-full border border-white/10 bg-white/5 px-4 py-2 text-xs font-bold uppercase tracking-[0.2em] text-slate-200 transition hover:bg-white/10 hover:text-white disabled:cursor-wait disabled:opacity-60"
          >
            {syncing ? t('calendar.syncingToIntervals') : t('calendar.syncToIntervals')}
          </button>
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
      {structureItems.length > 0 ? (
        <div className="rounded-2xl border border-white/6 bg-[#171a1d] p-4">
          <p className="text-[10px] font-black uppercase tracking-[0.24em] text-slate-500">{t('calendar.workoutStructure')}</p>
          <div className="mt-4 space-y-3">
            {structureItems.map((item) => (
              <div
                key={item.id}
                className="rounded-xl border border-white/6 bg-white/[0.03] px-4 py-3"
              >
                <div className="flex items-center justify-between gap-4">
                  <p className="text-sm font-bold text-[#f9f9fd]">{item.label}</p>
                  {item.durationSeconds ? (
                    <p className="text-xs font-bold uppercase tracking-[0.18em] text-[#d2ff9a]">
                      {formatDurationLabel(item.durationSeconds)}
                    </p>
                  ) : null}
                </div>
                {item.detail ? (
                  <p className="mt-1 text-xs text-slate-400">{item.detail}</p>
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
          {t('calendar.syncFailedMessage')}
        </div>
      ) : null}
    </div>
  );
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
