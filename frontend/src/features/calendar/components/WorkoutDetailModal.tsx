import { X } from 'lucide-react';
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { downloadFit, loadActivity, loadEvent } from '../../intervals/api/intervals';
import type { IntervalActivity, IntervalEvent } from '../../intervals/types';
import { AuthenticationError } from '../../../lib/httpClient';
import type { WorkoutDetailSelection } from '../workoutDetails';
import { buildCompletedWorkoutBars, buildPlannedWorkoutBars, formatDurationLabel } from '../workoutDetails';

type WorkoutDetailModalProps = {
  apiBaseUrl: string;
  selection: WorkoutDetailSelection | null;
  onClose: () => void;
};

type ModalState = {
  event: IntervalEvent | null;
  activity: IntervalActivity | null;
  loading: boolean;
};

export function WorkoutDetailModal({ apiBaseUrl, selection, onClose }: WorkoutDetailModalProps) {
  const { t } = useTranslation();
  const [state, setState] = useState<ModalState>({ event: null, activity: null, loading: false });
  const [downloadingFit, setDownloadingFit] = useState(false);

  useEffect(() => {
    if (!selection) {
      setState({ event: null, activity: null, loading: false });
      return;
    }

    let cancelled = false;
    setState({ event: null, activity: null, loading: true });

    void Promise.allSettled([
      selection.event ? loadEvent(apiBaseUrl, selection.event.id) : Promise.resolve(null),
      selection.activity ? loadActivity(apiBaseUrl, selection.activity.id) : Promise.resolve(null),
    ]).then(([eventResult, activityResult]) => {
      if (cancelled) {
        return;
      }

      const authError = [eventResult, activityResult]
        .filter((result): result is PromiseRejectedResult => result.status === 'rejected')
        .find((result) => result.reason instanceof AuthenticationError);
      if (authError) {
        window.location.href = '/';
        return;
      }

      setState({
        event: eventResult.status === 'fulfilled' ? eventResult.value : selection.event,
        activity: activityResult.status === 'fulfilled' ? activityResult.value : selection.activity,
        loading: false,
      });
    });

    return () => {
      cancelled = true;
    };
  }, [apiBaseUrl, selection]);

  if (!selection) {
    return null;
  }

  const event = state.event;
  const activity = state.activity;
  const actualWorkout = event?.actualWorkout ?? null;
  const isCompletedActivityOnly = Boolean(!event && activity);
  const isPlannedVsActual = Boolean(event && actualWorkout);
  const isCompleted = Boolean(actualWorkout || isCompletedActivityOnly);
  const title = isCompletedActivityOnly
    ? activity?.name ?? activity?.activityType ?? t('calendar.workout')
    : isPlannedVsActual
      ? actualWorkout?.activityName ?? event?.name ?? t('calendar.workout')
      : event?.name ?? t('calendar.workout');
  const showFitDownload = Boolean(event && !isCompleted);

  const handleDownloadFit = async () => {
    if (!event || downloadingFit) {
      return;
    }

    try {
      setDownloadingFit(true);
      const fitBytes = await downloadFit(apiBaseUrl, event.id);
      const fitFileBytes = Uint8Array.from(fitBytes);
      const blob = new Blob([fitFileBytes], { type: 'application/octet-stream' });
      const objectUrl = URL.createObjectURL(blob);
      const link = document.createElement('a');
      link.href = objectUrl;
      link.download = `event-${event.id}.fit`;
      link.click();
      URL.revokeObjectURL(objectUrl);
    } catch (error: unknown) {
      if (error instanceof AuthenticationError) {
        window.location.href = '/';
      }
    } finally {
      setDownloadingFit(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/65 px-4 py-6 backdrop-blur-sm">
      <div className="w-full max-w-5xl overflow-hidden rounded-[1.5rem] border border-white/8 bg-[#111417] shadow-[0_24px_80px_rgba(0,0,0,0.5)]">
        <div className="flex items-center justify-between border-b border-white/6 px-6 py-4 md:px-8">
          <div>
            <p className="text-[10px] font-black uppercase tracking-[0.28em] text-slate-500">
              {isCompleted ? t('calendar.completedWorkout') : t('calendar.plannedWorkout')}
            </p>
            <h2 className="mt-2 text-2xl font-black uppercase tracking-tight text-[#f9f9fd] md:text-3xl">{title}</h2>
          </div>
          <div className="flex items-center gap-3">
            {showFitDownload ? (
              <button
                type="button"
                onClick={() => void handleDownloadFit()}
                disabled={downloadingFit}
                className="rounded-full border border-white/10 bg-white/5 px-4 py-2 text-xs font-bold uppercase tracking-[0.2em] text-slate-200 transition hover:bg-white/10 hover:text-white disabled:cursor-wait disabled:opacity-60"
              >
                {downloadingFit ? t('calendar.downloadingFit') : t('calendar.downloadFit')}
              </button>
            ) : null}
            <button
              type="button"
              onClick={onClose}
              aria-label={t('calendar.closeWorkoutDetails')}
              className="rounded-full border border-white/10 bg-white/5 p-2 text-slate-300 transition hover:bg-white/10 hover:text-white"
            >
              <X size={18} />
            </button>
          </div>
        </div>

        <div className="max-h-[80vh] overflow-y-auto px-6 py-6 md:px-8">
          {state.loading ? (
            <p className="text-sm text-slate-400">{t('calendar.loadingWorkoutDetails')}</p>
          ) : isCompleted ? (
            <CompletedWorkoutPanel event={event} activity={activity} />
          ) : event ? (
            <PlannedWorkoutPanel event={event} />
          ) : (
            <p className="text-sm text-slate-400">{t('calendar.workoutDetailsUnavailable')}</p>
          )}
        </div>
      </div>
    </div>
  );
}

function PlannedWorkoutPanel({ event }: { event: IntervalEvent }) {
  const { t } = useTranslation();
  const bars = buildPlannedWorkoutBars(event);
  const summary = event.eventDefinition.summary;

  return (
    <div className="space-y-6">
      <WorkoutBars bars={bars} />
      <div className="grid gap-4 md:grid-cols-4">
        <MetricCard label={t('calendar.duration')} value={formatDurationLabel(summary.totalDurationSeconds)} />
        <MetricCard label="IF" value={summary.estimatedIntensityFactor !== null ? `${summary.estimatedIntensityFactor.toFixed(2)} IF` : '--'} />
        <MetricCard label="TSS" value={summary.estimatedTrainingStressScore !== null ? `${Math.round(summary.estimatedTrainingStressScore)} TSS` : '--'} />
        <MetricCard label="NP" value={summary.estimatedNormalizedPowerWatts !== null ? `${summary.estimatedNormalizedPowerWatts} W` : '--'} />
      </div>
    </div>
  );
}

function CompletedWorkoutPanel({ event, activity }: { event: IntervalEvent | null; activity: IntervalActivity | null }) {
  const { t } = useTranslation();
  const actualWorkout = event?.actualWorkout ?? null;
  const isCompletedActivityOnly = Boolean(!event && activity);
  const isPlannedVsActual = Boolean(event && actualWorkout);

  const bars = isCompletedActivityOnly && activity
    ? buildCompletedWorkoutBars(activity)
    : isPlannedVsActual
      ? buildActualWorkoutBars(actualWorkout)
      : [];
  const compliance = actualWorkout ? `${Math.round(actualWorkout.complianceScore * 100)}% ${t('calendar.compliance')}` : null;
  const completedIntervals = !actualWorkout
    ? activity?.details.intervals.filter((interval) => (
      interval.label !== null
      || interval.movingTimeSeconds !== null
      || interval.elapsedTimeSeconds !== null
      || interval.averagePowerWatts !== null
      || interval.averageHeartRateBpm !== null
    )) ?? []
    : [];
  const durationSeconds = isCompletedActivityOnly
    ? firstPositiveValue(activity?.movingTimeSeconds, activity?.elapsedTimeSeconds)
    : isPlannedVsActual
      ? firstPositiveValue(event?.eventDefinition.summary.totalDurationSeconds)
      : 0;
  const normalizedPowerLabel = isCompletedActivityOnly
    ? activity?.metrics.normalizedPowerWatts !== null && activity?.metrics.normalizedPowerWatts !== undefined
      ? `${activity.metrics.normalizedPowerWatts} W`
      : '--'
    : actualWorkout?.normalizedPowerWatts !== null && actualWorkout?.normalizedPowerWatts !== undefined
      ? `${actualWorkout.normalizedPowerWatts} W`
      : '--';
  const trainingStressLabel = isCompletedActivityOnly
    ? activity?.metrics.trainingStressScore !== null && activity?.metrics.trainingStressScore !== undefined
      ? `${activity.metrics.trainingStressScore} TSS`
      : '--'
    : actualWorkout?.trainingStressScore !== null && actualWorkout?.trainingStressScore !== undefined
      ? `${actualWorkout.trainingStressScore} TSS`
      : '--';

  return (
    <div className="space-y-6">
      <WorkoutBars bars={bars} />
      <div className="grid gap-4 md:grid-cols-4">
        <MetricCard label={t('calendar.duration')} value={formatDurationLabel(durationSeconds)} />
        <MetricCard label="NP" value={normalizedPowerLabel} />
        <MetricCard label="TSS" value={trainingStressLabel} />
        <MetricCard label={t('calendar.compliance')} value={compliance ?? '--'} />
      </div>
      {actualWorkout?.matchedIntervals.length ? (
        <div className="rounded-2xl border border-white/6 bg-[#171a1d] p-4">
          <p className="text-[10px] font-black uppercase tracking-[0.24em] text-slate-500">{t('calendar.intervalMatches')}</p>
          <div className="mt-4 space-y-3">
            {actualWorkout.matchedIntervals.map((interval) => (
              <div key={`${interval.plannedSegmentOrder}-${interval.actualIntervalId ?? 'planned'}`} className="flex items-center justify-between gap-4 rounded-xl bg-white/[0.03] px-4 py-3">
                <div>
                  <p className="text-sm font-bold text-[#f9f9fd]">{interval.plannedLabel}</p>
                  <p className="text-xs text-slate-400">{formatDurationLabel(interval.plannedDurationSeconds)}</p>
                </div>
                <p className="text-xs font-bold uppercase tracking-[0.2em] text-[#d2ff9a]">
                  {Math.round(interval.complianceScore * 100)}% {t('calendar.compliance')}
                </p>
              </div>
            ))}
          </div>
        </div>
      ) : null}
      {completedIntervals.length ? (
        <div className="rounded-2xl border border-white/6 bg-[#171a1d] p-4">
          <p className="text-[10px] font-black uppercase tracking-[0.24em] text-slate-500">{t('calendar.completedIntervals')}</p>
          <div className="mt-4 space-y-3">
            {completedIntervals.map((interval, index) => (
              <div key={`${interval.id ?? 'interval'}-${index}`} className="flex items-center justify-between gap-4 rounded-xl bg-white/[0.03] px-4 py-3">
                <div>
                  <p className="text-sm font-bold text-[#f9f9fd]">{interval.label ?? `${activity?.activityType ?? t('calendar.workout')} ${index + 1}`}</p>
                  <p className="text-xs text-slate-400">{formatDurationLabel(interval.movingTimeSeconds ?? interval.elapsedTimeSeconds ?? null)}</p>
                </div>
                <div className="text-right">
                  <p className="text-xs font-bold uppercase tracking-[0.2em] text-[#d2ff9a]">
                    {interval.averagePowerWatts !== null ? `${interval.averagePowerWatts} W` : '--'}
                  </p>
                  <p className="text-xs text-slate-400">
                    {interval.averageHeartRateBpm !== null ? `${interval.averageHeartRateBpm} bpm` : '--'}
                  </p>
                </div>
              </div>
            ))}
          </div>
        </div>
      ) : null}
    </div>
  );
}

function firstPositiveValue(...values: Array<number | null | undefined>): number {
  for (const value of values) {
    if (value !== null && value !== undefined && value > 0) {
      return value;
    }
  }

  return 0;
}

function buildActualWorkoutBars(actualWorkout: IntervalEvent['actualWorkout']): Array<{ height: number; color: string }> {
  if (!actualWorkout) {
    return [];
  }

  return actualWorkout.powerValues.slice(0, 24).map((value) => ({
    height: Math.max(20, Math.min(100, Math.round(value / 4))),
    color: '#d2ff9a',
  }));
}

function WorkoutBars({ bars }: { bars: Array<{ height: number; color: string }> }) {
  if (bars.length === 0) {
    return <div className="h-40 rounded-2xl border border-white/6 bg-[#171a1d]" />;
  }

  return (
    <div className="flex h-48 items-end gap-1 rounded-2xl border border-white/6 bg-[#171a1d] p-4">
      {bars.map((bar, index) => (
        <div
          key={`${bar.color}-${index}-${bar.height}`}
          className="flex-1 rounded-t-md"
          style={{ height: `${bar.height}%`, backgroundColor: bar.color }}
        />
      ))}
    </div>
  );
}

function MetricCard({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-2xl border border-white/6 bg-[#171a1d] p-4">
      <p className="text-[10px] font-black uppercase tracking-[0.24em] text-slate-500">{label}</p>
      <p className="mt-2 text-xl font-black text-[#f9f9fd]">{value}</p>
    </div>
  );
}
