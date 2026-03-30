import { X } from 'lucide-react';
import { type KeyboardEvent, type ReactNode, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { downloadFit, loadActivity, loadEvent } from '../../intervals/api/intervals';
import type { IntervalActivity, IntervalEvent } from '../../intervals/types';
import { AuthenticationError } from '../../../lib/httpClient';
import type { WorkoutDetailSelection } from '../workoutDetails';
import {
  buildCompletedWorkoutBars,
  buildMatchedWorkoutBars,
  buildPlannedWorkoutBars,
  extractCompletedPowerValues,
  formatDurationLabel,
} from '../workoutDetails';

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
  const detailsUnavailableMessage = !actualWorkout ? activity?.detailsUnavailableReason : null;
  const powerSeries = actualWorkout?.powerValues.length
    ? actualWorkout.powerValues
    : activity
      ? extractCompletedPowerValues(activity)
      : [];

  const bars = isCompletedActivityOnly && activity
    ? buildCompletedWorkoutBars(activity)
    : isPlannedVsActual
      ? buildMatchedWorkoutBars(actualWorkout)
      : [];
  const compliance = actualWorkout ? `${Math.round(actualWorkout.complianceScore * 100)}% ${t('calendar.compliance')}` : null;
  const completedIntervals = !actualWorkout
    ? activity?.details.intervals.filter(isDisplayableCompletedInterval) ?? []
    : [];
  const durationSeconds = isCompletedActivityOnly
    ? firstPositiveValue(activity?.movingTimeSeconds, activity?.elapsedTimeSeconds)
    : isPlannedVsActual
      ? firstPositiveValue(
          activity?.movingTimeSeconds,
          activity?.elapsedTimeSeconds,
          event?.eventDefinition.summary.totalDurationSeconds,
        )
      : 0;
  const completedIntervalTotalDurationSeconds = completedIntervalsTotalDuration(completedIntervals, durationSeconds);
  const matchedIntervalTotalDurationSeconds = matchedIntervalsTotalDuration(actualWorkout?.matchedIntervals ?? [], durationSeconds);
  const chartIntervalOverlays = chartIntervals(event, actualWorkout, activity);
  const intervalRowRefs = useRef(new Map<string, HTMLButtonElement>());
  const [hoveredIntervalKey, setHoveredIntervalKey] = useState<string | null>(null);
  const [selectedIntervalKey, setSelectedIntervalKey] = useState<string | null>(null);
  const highlightedIntervalKey = chartIntervalOverlays.some((interval) => interval.id === (hoveredIntervalKey ?? selectedIntervalKey))
    ? (hoveredIntervalKey ?? selectedIntervalKey)
    : null;
  const activeInterval = chartIntervalOverlays.find((interval) => interval.id === highlightedIntervalKey) ?? null;

  useEffect(() => {
    if (!highlightedIntervalKey) {
      return;
    }

    const node = intervalRowRefs.current.get(highlightedIntervalKey);
    if (node && typeof node.scrollIntoView === 'function') {
      node.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
    }
  }, [highlightedIntervalKey]);
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
      {powerSeries.length ? (
        <PowerChart
          activeIntervalKey={highlightedIntervalKey}
          activeInterval={activeInterval}
          intervals={chartIntervalOverlays}
          onHoverIntervalChange={setHoveredIntervalKey}
          onSelectIntervalChange={setSelectedIntervalKey}
          title={t('calendar.powerChart')}
          values={powerSeries}
        />
      ) : null}
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
              <IntervalRow
                key={`${interval.plannedSegmentOrder}-${interval.actualIntervalId ?? 'planned'}`}
                durationFillPercent={durationFillPercent(matchedIntervalDurationSeconds(interval), matchedIntervalTotalDurationSeconds)}
                isHighlighted={highlightedIntervalKey === `${interval.plannedSegmentOrder}-${interval.actualIntervalId ?? 'planned'}`}
                onHover={() => setHoveredIntervalKey(`${interval.plannedSegmentOrder}-${interval.actualIntervalId ?? 'planned'}`)}
                onHoverEnd={() => setHoveredIntervalKey(null)}
                onSelect={() => setSelectedIntervalKey((current) => current === `${interval.plannedSegmentOrder}-${interval.actualIntervalId ?? 'planned'}` ? null : `${interval.plannedSegmentOrder}-${interval.actualIntervalId ?? 'planned'}`)}
                rowRef={(node) => updateIntervalRowRef(intervalRowRefs.current, `${interval.plannedSegmentOrder}-${interval.actualIntervalId ?? 'planned'}`, node)}
                left={(
                  <div>
                    <p className="text-sm font-bold text-[#f9f9fd]">{interval.plannedLabel}</p>
                    <p className="text-xs text-slate-400">{formatDurationLabel(interval.plannedDurationSeconds)}</p>
                  </div>
                )}
                right={(
                  <p className="text-xs font-bold uppercase tracking-[0.2em] text-[#d2ff9a]">
                    {Math.round(interval.complianceScore * 100)}% {t('calendar.compliance')}
                  </p>
                )}
              />
            ))}
          </div>
        </div>
      ) : null}
      {completedIntervals.length ? (
        <div className="rounded-2xl border border-white/6 bg-[#171a1d] p-4">
          <p className="text-[10px] font-black uppercase tracking-[0.24em] text-slate-500">{t('calendar.completedIntervals')}</p>
          <div className="mt-4 space-y-3">
            {completedIntervals.map((interval, index) => (
              <IntervalRow
                key={`${interval.id ?? 'interval'}-${index}`}
                durationFillPercent={durationFillPercent(completedIntervalDurationSeconds(interval), completedIntervalTotalDurationSeconds)}
                isHighlighted={highlightedIntervalKey === `${interval.id ?? 'interval'}-${index}`}
                onHover={() => setHoveredIntervalKey(`${interval.id ?? 'interval'}-${index}`)}
                onHoverEnd={() => setHoveredIntervalKey(null)}
                onSelect={() => setSelectedIntervalKey((current) => current === `${interval.id ?? 'interval'}-${index}` ? null : `${interval.id ?? 'interval'}-${index}`)}
                rowRef={(node) => updateIntervalRowRef(intervalRowRefs.current, `${interval.id ?? 'interval'}-${index}`, node)}
                left={(
                  <div>
                    <p className="text-sm font-bold text-[#f9f9fd]">{interval.label ?? `${activity?.activityType ?? t('calendar.workout')} ${index + 1}`}</p>
                    <p className="text-xs text-slate-400">{formatDurationLabel(interval.movingTimeSeconds ?? interval.elapsedTimeSeconds ?? null)}</p>
                  </div>
                )}
                right={(
                  <div className="text-right">
                    <p className="text-xs font-bold uppercase tracking-[0.2em] text-[#d2ff9a]">
                      {interval.averagePowerWatts !== null ? `${interval.averagePowerWatts} W` : '--'}
                    </p>
                    <p className="text-xs text-slate-400">
                      {interval.averageHeartRateBpm !== null ? `${interval.averageHeartRateBpm} bpm` : '--'}
                    </p>
                  </div>
                )}
              />
            ))}
          </div>
        </div>
      ) : null}
      {detailsUnavailableMessage ? (
        <div className="rounded-2xl border border-amber-300/20 bg-amber-300/10 p-4 text-sm text-amber-100">
          {t('calendar.importedWorkoutDetailsUnavailable')}
        </div>
      ) : null}
    </div>
  );
}

function IntervalRow({
  durationFillPercent,
  isHighlighted,
  onHover,
  onHoverEnd,
  onSelect,
  rowRef,
  left,
  right,
}: {
  durationFillPercent: number;
  isHighlighted: boolean;
  onHover: () => void;
  onHoverEnd: () => void;
  onSelect: () => void;
  rowRef: (node: HTMLButtonElement | null) => void;
  left: ReactNode;
  right: ReactNode;
}) {
  const handleKeyDown = (event: KeyboardEvent<HTMLButtonElement>) => {
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      onSelect();
    }
  };

  return (
    <button
      ref={rowRef}
      type="button"
      onClick={onSelect}
      onFocus={onHover}
      onMouseEnter={onHover}
      onMouseLeave={onHoverEnd}
      onBlur={onHoverEnd}
      onKeyDown={handleKeyDown}
      data-interval-row-active={isHighlighted ? 'true' : 'false'}
      className={`relative block w-full overflow-hidden rounded-xl px-4 py-3 text-left transition focus:outline-none focus-visible:ring-2 focus-visible:ring-[#d2ff9a]/40 ${isHighlighted ? 'bg-[#20291d] ring-1 ring-[#d2ff9a]/30' : 'bg-white/[0.03]'}`}
    >
      <div
        data-interval-duration-fill="true"
        className={`absolute inset-y-0 left-0 rounded-r-xl ${isHighlighted ? 'bg-[linear-gradient(90deg,rgba(210,255,154,0.24)_0%,rgba(210,255,154,0.08)_100%)]' : 'bg-[linear-gradient(90deg,rgba(210,255,154,0.14)_0%,rgba(210,255,154,0.04)_100%)]'}`}
        style={{ width: `${durationFillPercent}%` }}
      />
      <div className="relative flex items-center justify-between gap-4">
        {left}
        {right}
      </div>
    </button>
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

function WorkoutBars({ bars }: { bars: Array<{ height: number; color: string; widthUnits?: number }> }) {
  if (bars.length === 0) {
    return <div className="h-40 rounded-2xl border border-white/6 bg-[#171a1d]" />;
  }

  return (
    <div className="flex h-48 items-end gap-1 rounded-2xl border border-white/6 bg-[#171a1d] p-4">
      {bars.map((bar, index) => (
        <div
          key={`${bar.color}-${index}-${bar.height}-${bar.widthUnits ?? 1}`}
          data-chart-bar="detail"
          className="min-w-[6px] rounded-t-md"
          style={{ flexBasis: 0, flexGrow: Math.max(1, bar.widthUnits ?? 1), height: `${bar.height}%`, backgroundColor: bar.color }}
        />
      ))}
    </div>
  );
}

type ChartIntervalOverlay = {
  id: string;
  startSecond: number;
  endSecond: number;
  label: string;
};

type ChartSamplePoint = {
  value: number;
  second: number;
};

function PowerChart({
  activeInterval,
  activeIntervalKey,
  intervals,
  onHoverIntervalChange,
  onSelectIntervalChange,
  title,
  values,
}: {
  activeInterval: ChartIntervalOverlay | null;
  activeIntervalKey: string | null;
  intervals: ChartIntervalOverlay[];
  onHoverIntervalChange: (intervalKey: string | null) => void;
  onSelectIntervalChange: (intervalKey: string | null) => void;
  title: string;
  values: number[];
}) {
  const totalSeconds = Math.max(values.length, intervals.reduce((max, interval) => Math.max(max, interval.endSecond), 0), 1);
  const sampledPoints = samplePowerValues(values, 180);
  const [hoveredSampleIndex, setHoveredSampleIndex] = useState<number | null>(null);

  if (sampledPoints.length === 0) {
    return null;
  }

  const hoveredSample = hoveredSampleIndex !== null ? sampledPoints[hoveredSampleIndex] : null;
  const pinnedSample = activeInterval ? samplePointForInterval(sampledPoints, activeInterval) : null;
  const displayedSample = hoveredSample ?? pinnedSample;
  const maxValue = Math.max(...sampledPoints.map((point) => point.value), 1);
  const chartHeight = 220;
  const chartWidth = 1000;
  const points = sampledPoints
    .map((point, index) => {
      const x = sampledPoints.length === 1 ? 0 : (index / (sampledPoints.length - 1)) * chartWidth;
      const normalized = Math.max(0, point.value) / maxValue;
      const y = chartHeight - (normalized * chartHeight);
      return `${x},${y}`;
    })
    .join(' ');

  const markerIndex = hoveredSampleIndex ?? (pinnedSample ? sampledPoints.findIndex((point) => point.second === pinnedSample.second && point.value === pinnedSample.value) : null);
  const markerX = markerIndex === null || markerIndex < 0 || sampledPoints.length === 1
    ? null
    : (markerIndex / (sampledPoints.length - 1)) * chartWidth;
  const markerY = displayedSample === null
    ? null
    : chartHeight - ((Math.max(0, displayedSample.value) / maxValue) * chartHeight);

  const handleChartPointerMove = (event: React.MouseEvent<SVGSVGElement>) => {
    const bounds = event.currentTarget.getBoundingClientRect();
    if (bounds.width <= 0 || sampledPoints.length === 0) {
      return;
    }

    const relativeX = Math.max(0, Math.min(1, (event.clientX - bounds.left) / bounds.width));
    const nextIndex = Math.round(relativeX * Math.max(0, sampledPoints.length - 1));
    setHoveredSampleIndex(nextIndex);
    const hoveredSecond = sampledPoints[nextIndex]?.second ?? 0;
    const nextInterval = intervals.find((interval) => hoveredSecond >= interval.startSecond && hoveredSecond <= interval.endSecond);
    onHoverIntervalChange(nextInterval?.id ?? null);
  };

  return (
    <div className="rounded-2xl border border-white/6 bg-[#171a1d] p-4">
      <div className="flex items-start justify-between gap-4">
        <p className="text-[10px] font-black uppercase tracking-[0.24em] text-slate-500">{title}</p>
        <div className="flex items-center gap-4">
          {displayedSample ? (
            <p data-hover-power-readout="true" className="text-xs font-bold uppercase tracking-[0.18em] text-slate-300">
              {formatChartTimeLabel(displayedSample.second)} • {displayedSample.value} W
            </p>
          ) : null}
          <p className="text-xs font-bold uppercase tracking-[0.18em] text-[#d2ff9a]">{maxValue} W max</p>
        </div>
      </div>
      <div className="mt-4 overflow-hidden rounded-2xl border border-white/5 bg-[linear-gradient(180deg,rgba(210,255,154,0.16)_0%,rgba(210,255,154,0.03)_100%)] p-3">
        <svg
          aria-label={title}
          className="h-56 w-full"
          data-power-chart="true"
          viewBox={`0 0 ${chartWidth} ${chartHeight}`}
          onMouseLeave={() => {
            setHoveredSampleIndex(null);
            onHoverIntervalChange(null);
          }}
          onMouseMove={handleChartPointerMove}
          preserveAspectRatio="none"
          role="img"
        >
          <defs>
            <linearGradient id="power-chart-stroke" x1="0%" y1="0%" x2="100%" y2="0%">
              <stop offset="0%" stopColor="#52c41a" />
              <stop offset="55%" stopColor="#d2ff9a" />
              <stop offset="100%" stopColor="#facc15" />
            </linearGradient>
          </defs>
          {intervals.map((interval, index) => {
            const startX = (Math.max(0, interval.startSecond) / totalSeconds) * chartWidth;
            const endX = (Math.max(interval.startSecond, interval.endSecond) / totalSeconds) * chartWidth;
            const width = Math.max(6, endX - startX);
            const isActive = interval.id === activeIntervalKey;

            return (
              <g key={`${interval.label}-${index}-${interval.startSecond}`}>
                <rect
                  data-interval-overlay="true"
                  x={startX}
                  y={0}
                  width={width}
                  height={chartHeight}
                  fill={isActive ? 'rgba(210,255,154,0.16)' : index % 2 === 0 ? 'rgba(255,255,255,0.05)' : 'rgba(255,255,255,0.02)'}
                />
                <line x1={startX} x2={startX} y1={0} y2={chartHeight} stroke={isActive ? 'rgba(210,255,154,0.32)' : 'rgba(255,255,255,0.08)'} strokeWidth="2" />
              </g>
            );
          })}
          <path d={`M 0 ${chartHeight} L ${points} L ${chartWidth} ${chartHeight} Z`} fill="rgba(210,255,154,0.18)" />
          <polyline
            fill="none"
            points={points}
            stroke="url(#power-chart-stroke)"
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth="8"
          />
          {displayedSample !== null && markerX !== null && markerY !== null ? (
            <g data-power-chart-marker="true">
              <line x1={markerX} x2={markerX} y1={0} y2={chartHeight} stroke="rgba(255,255,255,0.22)" strokeDasharray="8 8" strokeWidth="2" />
              <circle cx={markerX} cy={markerY} r="8" fill="#d2ff9a" stroke="#111417" strokeWidth="4" />
            </g>
          ) : null}
        </svg>
      </div>
      <div className="mt-3 flex items-center justify-between gap-2 text-[10px] font-bold uppercase tracking-[0.16em] text-slate-500">
        {buildTimeTicks(totalSeconds).map((tick, index) => (
          <span key={`${index}-${tick.second}-${tick.label}`}>{tick.label}</span>
        ))}
      </div>
      {intervals.length ? (
        <div className="mt-3 flex flex-wrap gap-2">
          {intervals.map((interval, index) => (
            <span
              key={`${interval.label}-${interval.startSecond}-${index}`}
              data-interval-chip-active={interval.id === activeIntervalKey ? 'true' : 'false'}
              className={`rounded-full border px-3 py-1 text-[10px] font-bold uppercase tracking-[0.16em] transition ${interval.id === activeIntervalKey ? 'border-[#d2ff9a]/40 bg-[#d2ff9a]/12 text-[#f4ffd9]' : 'border-white/8 bg-white/[0.04] text-slate-300'}`}
              onClick={() => onSelectIntervalChange(activeIntervalKey === interval.id ? null : interval.id)}
              onMouseEnter={() => onHoverIntervalChange(interval.id)}
              onMouseLeave={() => onHoverIntervalChange(null)}
              onKeyDown={(event) => {
                if (event.key === 'Enter' || event.key === ' ') {
                  event.preventDefault();
                  onSelectIntervalChange(activeIntervalKey === interval.id ? null : interval.id);
                }
              }}
              role="button"
              tabIndex={0}
            >
              {interval.label}
            </span>
          ))}
        </div>
      ) : null}
    </div>
  );
}

function samplePowerValues(values: number[], maxPoints: number): ChartSamplePoint[] {
  if (values.length <= maxPoints) {
    return values.map((value, index) => ({ value, second: index }));
  }

  const bucketSize = values.length / maxPoints;
  const sampled: ChartSamplePoint[] = [];

  for (let index = 0; index < maxPoints; index += 1) {
    const start = Math.floor(index * bucketSize);
    const end = Math.min(values.length, Math.floor((index + 1) * bucketSize));
    const bucket = values.slice(start, Math.max(start + 1, end));
    const average = bucket.reduce((sum, value) => sum + value, 0) / bucket.length;
    sampled.push({
      value: Math.round(average),
      second: Math.round((start + Math.max(start, end - 1)) / 2),
    });
  }

  return sampled;
}

function samplePointForInterval(points: ChartSamplePoint[], interval: ChartIntervalOverlay): ChartSamplePoint | null {
  if (points.length === 0) {
    return null;
  }

  const midpoint = interval.startSecond + ((interval.endSecond - interval.startSecond) / 2);

  return points.reduce((closest, point) => {
    if (closest === null) {
      return point;
    }

    const closestDistance = Math.abs(closest.second - midpoint);
    const nextDistance = Math.abs(point.second - midpoint);
    return nextDistance < closestDistance ? point : closest;
  }, null as ChartSamplePoint | null);
}

function buildTimeTicks(totalSeconds: number): Array<{ second: number; label: string }> {
  const safeTotalSeconds = Math.max(1, totalSeconds - 1);
  return [0, 0.25, 0.5, 0.75, 1].map((ratio) => {
    const second = Math.round(safeTotalSeconds * ratio);
    return {
      second,
      label: formatChartTimeLabel(second),
    };
  });
}

function formatChartTimeLabel(totalSeconds: number): string {
  const safeSeconds = Math.max(0, Math.round(totalSeconds));
  const hours = Math.floor(safeSeconds / 3600);
  const minutes = Math.floor((safeSeconds % 3600) / 60);
  const seconds = safeSeconds % 60;

  if (hours > 0) {
    return `${hours}:${String(minutes).padStart(2, '0')}:${String(seconds).padStart(2, '0')}`;
  }

  return `${minutes}:${String(seconds).padStart(2, '0')}`;
}

function chartIntervals(event: IntervalEvent | null, actualWorkout: IntervalEvent['actualWorkout'], activity: IntervalActivity | null): ChartIntervalOverlay[] {
  if (actualWorkout?.matchedIntervals.length) {
    return actualWorkout.matchedIntervals
      .filter((interval) => interval.actualStartTimeSeconds !== null || interval.actualEndTimeSeconds !== null)
      .map((interval, index) => ({
        id: `${interval.plannedSegmentOrder}-${interval.actualIntervalId ?? 'planned'}`,
        startSecond: interval.actualStartTimeSeconds ?? runningStart(actualWorkout.matchedIntervals, index),
        endSecond: interval.actualEndTimeSeconds ?? ((interval.actualStartTimeSeconds ?? runningStart(actualWorkout.matchedIntervals, index)) + interval.plannedDurationSeconds),
        label: interval.plannedLabel,
      }));
  }

  if (actualWorkout && event?.eventDefinition.segments.length) {
    return event.eventDefinition.segments.map((segment) => ({
      id: `${segment.order}-planned`,
      startSecond: segment.startOffsetSeconds,
      endSecond: segment.endOffsetSeconds,
      label: segment.label,
    }));
  }

  if (!activity) {
    return [];
  }

  return buildCompletedChartIntervals(activity);
}

function isDisplayableCompletedInterval(interval: IntervalActivity['details']['intervals'][number]): boolean {
  return interval.label !== null
    || interval.movingTimeSeconds !== null
    || interval.elapsedTimeSeconds !== null
    || interval.averagePowerWatts !== null
    || interval.averageHeartRateBpm !== null;
}

function completedIntervalsTotalDuration(intervals: IntervalActivity['details']['intervals'], fallbackDurationSeconds: number): number {
  const inferredTotal = intervals.reduce((sum, interval) => sum + completedIntervalDurationSeconds(interval), 0);
  return Math.max(fallbackDurationSeconds, inferredTotal, 1);
}

function matchedIntervalsTotalDuration(
  intervals: NonNullable<IntervalEvent['actualWorkout']>['matchedIntervals'],
  fallbackDurationSeconds: number,
): number {
  const inferredTotal = intervals.reduce((sum, interval) => sum + matchedIntervalDurationSeconds(interval), 0);
  return Math.max(fallbackDurationSeconds, inferredTotal, 1);
}

function completedIntervalDurationSeconds(interval: IntervalActivity['details']['intervals'][number]): number {
  const timedDuration = interval.startTimeSeconds !== null && interval.endTimeSeconds !== null
    ? interval.endTimeSeconds - interval.startTimeSeconds
    : interval.movingTimeSeconds ?? interval.elapsedTimeSeconds;

  return Math.max(1, timedDuration ?? 1);
}

function matchedIntervalDurationSeconds(interval: NonNullable<IntervalEvent['actualWorkout']>['matchedIntervals'][number]): number {
  const timedDuration = interval.actualStartTimeSeconds !== null && interval.actualEndTimeSeconds !== null
    ? interval.actualEndTimeSeconds - interval.actualStartTimeSeconds
    : interval.plannedDurationSeconds;

  return Math.max(1, timedDuration);
}

function durationFillPercent(durationSeconds: number, totalDurationSeconds: number): number {
  return Math.max(4, Math.min(100, (durationSeconds / Math.max(1, totalDurationSeconds)) * 100));
}

function runningStart<T extends { actualStartTimeSeconds?: number | null; actualEndTimeSeconds?: number | null; plannedDurationSeconds?: number; startTimeSeconds?: number | null; endTimeSeconds?: number | null; movingTimeSeconds?: number | null; elapsedTimeSeconds?: number | null }>(intervals: T[], index: number): number {
  return intervals.slice(0, index).reduce((sum, interval) => {
    if (interval.actualEndTimeSeconds !== null && interval.actualEndTimeSeconds !== undefined) {
      return interval.actualEndTimeSeconds;
    }
    if (interval.endTimeSeconds !== null && interval.endTimeSeconds !== undefined) {
      return interval.endTimeSeconds;
    }

    const duration = interval.plannedDurationSeconds
      ?? interval.movingTimeSeconds
      ?? interval.elapsedTimeSeconds
      ?? ((interval.actualEndTimeSeconds ?? interval.endTimeSeconds ?? 0) - (interval.actualStartTimeSeconds ?? interval.startTimeSeconds ?? 0));

    return sum + Math.max(0, duration);
  }, 0);
}

function buildCompletedChartIntervals(activity: IntervalActivity): ChartIntervalOverlay[] {
  const overlays: ChartIntervalOverlay[] = [];
  let cursor = 0;

  activity.details.intervals.forEach((interval, index) => {
    const startSecond = interval.startTimeSeconds ?? cursor;
    const durationSeconds = completedIntervalDurationSeconds(interval);
    const endSecond = interval.endTimeSeconds ?? (startSecond + durationSeconds);

    if (isDisplayableCompletedInterval(interval) && hasChartTiming(interval)) {
      overlays.push({
        id: `${interval.id ?? 'interval'}-${index}`,
        startSecond,
        endSecond,
        label: interval.label ?? `${activity.activityType ?? 'Workout'} ${index + 1}`,
      });
    }

    cursor = Math.max(cursor, endSecond);
  });

  return overlays;
}

function hasChartTiming(interval: IntervalActivity['details']['intervals'][number]): boolean {
  return interval.startTimeSeconds !== null
    || interval.endTimeSeconds !== null
    || interval.movingTimeSeconds !== null
    || interval.elapsedTimeSeconds !== null;
}

function updateIntervalRowRef(
  refs: Map<string, HTMLButtonElement>,
  key: string,
  node: HTMLButtonElement | null,
) {
  if (node) {
    refs.set(key, node);
  } else {
    refs.delete(key);
  }
}

function MetricCard({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-2xl border border-white/6 bg-[#171a1d] p-4">
      <p className="text-[10px] font-black uppercase tracking-[0.24em] text-slate-500">{label}</p>
      <p className="mt-2 text-xl font-black text-[#f9f9fd]">{value}</p>
    </div>
  );
}
