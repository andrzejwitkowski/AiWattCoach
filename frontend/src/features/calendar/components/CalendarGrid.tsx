import { AlertTriangle } from 'lucide-react';
import { useCallback, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import {
  CALENDAR_BUFFER_WEEKS,
  CALENDAR_PAGINATION_LOCK_RELEASE_DISTANCE,
  CALENDAR_PAGINATION_TRIGGER_OFFSET,
  CALENDAR_SHIFT_WEEKS,
  CALENDAR_VISIBLE_WEEKS,
  CALENDAR_WEEK_BLOCK_HEIGHT,
  CALENDAR_WEEK_ROW_GAP,
} from '../constants';
import { useCalendarData } from '../hooks/useCalendarData';
import { selectDayItemDetail, type CalendarDayItemsSelection } from '../dayItems';
import type { CalendarRaceLabel } from '../types';
import type { WorkoutDetailSelection } from '../workoutDetails';
import { CalendarPerformanceCards } from './CalendarPerformanceCards';
import { DayItemsModal } from './DayItemsModal';
import { RaceDayDetailModal } from './RaceDayDetailModal';
import { WorkoutDetailModal } from './WorkoutDetailModal';
import { CalendarWeekDayHeader } from './CalendarWeekDayHeader';
import { CalendarWeekSection } from './CalendarWeekSection';

type CalendarGridProps = {
  apiBaseUrl: string;
};

export function CalendarGrid({ apiBaseUrl }: CalendarGridProps) {
  const { t, i18n } = useTranslation();
  const locale = i18n.resolvedLanguage ?? i18n.language ?? 'en';
  const {
    state,
    weeks,
    renderedWeeks,
    isLoadingPast,
    isLoadingFuture,
    scrollAdjustment,
    loadMorePast,
    loadMoreFuture,
  } = useCalendarData({ apiBaseUrl });
  const [selection, setSelection] = useState<WorkoutDetailSelection | null>(null);
  const [dayItemsSelection, setDayItemsSelection] = useState<CalendarDayItemsSelection | null>(null);
  const [raceSelection, setRaceSelection] = useState<CalendarRaceLabel | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const appliedAdjustmentVersionRef = useRef(0);
  const initializingScrollRef = useRef(true);
  const pendingAnchorRef = useRef<{ weekKey: string; top: number } | null>(null);
  const edgeLockRef = useRef<{ edge: 'top' | 'bottom'; releaseScrollTop: number } | null>(null);

  const visibleRangeLabel = useMemo(() => {
    const firstWeek = weeks[0];
    const lastWeek = weeks[weeks.length - 1];

    if (!firstWeek || !lastWeek) {
      return t('calendar.fiveWeeks');
    }

    const firstLabel = new Intl.DateTimeFormat(locale, { month: 'long', year: 'numeric' }).format(firstWeek.mondayDate);
    const lastDate = lastWeek.days[lastWeek.days.length - 1]?.date ?? lastWeek.mondayDate;
    const lastLabel = new Intl.DateTimeFormat(locale, { month: 'long', year: 'numeric' }).format(lastDate);

    return firstLabel === lastLabel ? firstLabel : `${firstLabel} - ${lastLabel}`;
  }, [locale, t, weeks]);

  const handleReachTop = useCallback(() => {
    if (isLoadingPast || isLoadingFuture) {
      return;
    }

    const container = scrollRef.current;
    pendingAnchorRef.current = captureScrollAnchor(scrollRef.current, renderedWeeks[0]?.weekKey ?? null);
    edgeLockRef.current = {
      edge: 'top',
      releaseScrollTop: (container?.scrollTop ?? 0) + CALENDAR_PAGINATION_LOCK_RELEASE_DISTANCE,
    };
    void loadMorePast();
  }, [isLoadingFuture, isLoadingPast, loadMorePast, renderedWeeks]);

  const handleReachBottom = useCallback(() => {
    if (isLoadingFuture || isLoadingPast) {
      return;
    }

    const container = scrollRef.current;
    pendingAnchorRef.current = captureScrollAnchor(
      scrollRef.current,
      renderedWeeks[CALENDAR_SHIFT_WEEKS]?.weekKey ?? renderedWeeks[0]?.weekKey ?? null,
    );
    edgeLockRef.current = {
      edge: 'bottom',
      releaseScrollTop: (container?.scrollTop ?? 0) - CALENDAR_PAGINATION_LOCK_RELEASE_DISTANCE,
    };
    void loadMoreFuture();
  }, [isLoadingFuture, isLoadingPast, loadMoreFuture, renderedWeeks]);

  const handleScroll = useCallback(() => {
    const container = scrollRef.current;
    if (!container || state !== 'ready') {
      return;
    }

    const { scrollTop, clientHeight, scrollHeight } = container;
    const edgeLock = edgeLockRef.current;

    if (edgeLock) {
      const shouldRelease = edgeLock.edge === 'top'
        ? scrollTop >= edgeLock.releaseScrollTop
        : scrollTop <= edgeLock.releaseScrollTop;

      if (!shouldRelease) {
        return;
      }

      edgeLockRef.current = null;
    }

    const edgeThreshold = CALENDAR_PAGINATION_TRIGGER_OFFSET;
    const isAtTopEdge = scrollTop <= edgeThreshold;
    const isAtBottomEdge = scrollTop + clientHeight >= scrollHeight - edgeThreshold;

    if (isAtTopEdge) {
      void handleReachTop();
      return;
    }

    if (isAtBottomEdge) {
      void handleReachBottom();
    }
  }, [handleReachBottom, handleReachTop, state]);

  useLayoutEffect(() => {
    const container = scrollRef.current;

    if (!container || scrollAdjustment.version === 0 || appliedAdjustmentVersionRef.current === scrollAdjustment.version) {
      return;
    }

    const pendingAnchor = pendingAnchorRef.current;
    if (pendingAnchor) {
      const anchorElement = container.querySelector<HTMLElement>(`[data-week-key="${pendingAnchor.weekKey}"]`);

      if (anchorElement) {
        const nextTop = anchorElement.offsetTop - container.scrollTop;
        container.scrollTop += nextTop - pendingAnchor.top;
      } else {
        container.scrollTop += scrollAdjustment.topDelta;
      }
    } else {
      container.scrollTop += scrollAdjustment.topDelta;
    }

    pendingAnchorRef.current = null;
    appliedAdjustmentVersionRef.current = scrollAdjustment.version;
  }, [scrollAdjustment]);

  useLayoutEffect(() => {
    const container = scrollRef.current;
    if (!container || !initializingScrollRef.current || weeks.length === 0) {
      return;
    }

    const anchorElement = container.querySelector<HTMLElement>(`[data-week-key="${weeks[0]?.weekKey}"]`);
    container.scrollTop = anchorElement?.offsetTop ?? (CALENDAR_BUFFER_WEEKS * CALENDAR_WEEK_BLOCK_HEIGHT);
    initializingScrollRef.current = false;
  }, [weeks.length]);

  if (state === 'credentials-required') {
    return (
      <div className="rounded-[1.5rem] border border-[#ff7351]/25 bg-[#1d2024] p-8 text-center">
        <AlertTriangle className="mx-auto mb-4 text-[#ff7351]" size={28} />
        <h2 className="text-2xl font-black text-[#f9f9fd]">{t('calendar.title')}</h2>
        <p className="mt-3 text-sm text-slate-400">{t('calendar.connectionRequired')}</p>
      </div>
    );
  }

  if (state === 'error') {
    return (
      <div className="rounded-[1.5rem] border border-[#ff7351]/25 bg-[#1d2024] p-8 text-center">
        <AlertTriangle className="mx-auto mb-4 text-[#ff7351]" size={28} />
        <h2 className="text-2xl font-black text-[#f9f9fd]">{t('calendar.title')}</h2>
        <p className="mt-3 text-sm text-slate-400">{t('calendar.loadError')}</p>
      </div>
    );
  }

  return (
    <section className="space-y-8">
      <div className="rounded-[1.75rem] border border-white/5 bg-[linear-gradient(180deg,rgba(17,20,23,0.96),rgba(12,14,17,0.92))] p-4 shadow-[0_24px_80px_rgba(0,0,0,0.35)] md:p-8">
        <div className="mb-6 flex flex-col gap-5 lg:flex-row lg:items-end lg:justify-between">
          <div>
            <p className="text-[10px] font-black uppercase tracking-[0.35em] text-slate-500">{t('calendar.baseMonth')}</p>
            <h2 className="mt-2 text-3xl font-black uppercase tracking-tight text-[#d2ff9a] md:text-4xl">
              {t('calendar.performanceCalendar')}
            </h2>
          </div>
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 sm:gap-6">
            <MetricPill title={t('calendar.visibleWindow')} value={visibleRangeLabel} accent="text-[#d2ff9a]" />
            <MetricPill title={t('calendar.scrollMode')} value={t('calendar.infinite')} accent="text-[#00e3fd]" />
          </div>
        </div>

        <div className="overflow-x-auto pb-1">
          <div className="min-w-[960px]">
            <div
              ref={scrollRef}
              onScroll={handleScroll}
              tabIndex={0}
              role="region"
              aria-label={t('calendar.performanceCalendar')}
              className="no-scrollbar overflow-y-auto pr-1"
              style={{
                maxHeight: `${(CALENDAR_WEEK_BLOCK_HEIGHT * CALENDAR_VISIBLE_WEEKS) - CALENDAR_WEEK_ROW_GAP}px`,
              }}
            >
              <CalendarWeekDayHeader />
              <div className="mt-8 space-y-10">
                {renderedWeeks.length > 0 ? (
                      renderedWeeks.map((week) => (
                      <div key={week.weekKey} data-week-key={week.weekKey}>
                      <CalendarWeekSection
                        week={week}
                        onSelectWorkout={setSelection}
                        onSelectDayItems={setDayItemsSelection}
                        onSelectRace={setRaceSelection}
                      />
                      </div>
                    ))
                ) : (
                  <div className="rounded-xl border border-white/5 bg-[#171a1d] p-6 text-center text-sm text-slate-400">
                    {t('calendar.noEvents')}
                  </div>
                )}
              </div>
            </div>
          </div>
        </div>
      </div>

      <CalendarPerformanceCards />
      <DayItemsModal
        selection={dayItemsSelection}
        onClose={() => setDayItemsSelection(null)}
        onSelectItem={(item) => {
          setDayItemsSelection(null);
          if (item.kind === 'race') {
            setRaceSelection(item.race);
            return;
          }

          const nextSelection = selectDayItemDetail(item);
          if (nextSelection) {
            setSelection(nextSelection);
          }
        }}
      />
      <RaceDayDetailModal selection={raceSelection} onClose={() => setRaceSelection(null)} />
      <WorkoutDetailModal apiBaseUrl={apiBaseUrl} selection={selection} onClose={() => setSelection(null)} />
    </section>
  );
}

function captureScrollAnchor(container: HTMLDivElement | null, weekKey: string | null) {
  if (!container || !weekKey) {
    return null;
  }

  const anchorElement = container.querySelector<HTMLElement>(`[data-week-key="${weekKey}"]`);
  if (!anchorElement) {
    return null;
  }

  return {
    weekKey,
    top: anchorElement.offsetTop - container.scrollTop,
  };
}

function MetricPill({ title, value, accent }: { title: string; value: string; accent: string }) {
  return (
    <div className="rounded-xl border border-white/5 bg-[#171a1d]/70 px-4 py-3 sm:min-w-[12rem]">
      <span className="block text-[10px] font-bold uppercase tracking-[0.24em] text-slate-500">{title}</span>
      <span className={`mt-2 block text-sm font-bold uppercase tracking-tight ${accent}`}>{value}</span>
    </div>
  );
}
