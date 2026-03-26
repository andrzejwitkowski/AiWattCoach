import { AlertTriangle } from 'lucide-react';
import { useCallback, useLayoutEffect, useMemo, useRef } from 'react';
import { useTranslation } from 'react-i18next';

import {
  CALENDAR_ANCHOR_SCROLL_TOP,
  CALENDAR_PREVIEW_VISIBLE_HEIGHT,
  CALENDAR_WEEK_ROW_HEIGHT,
} from '../constants';
import { useCalendarData } from '../hooks/useCalendarData';
import type { CalendarWeek } from '../types';
import { CalendarPerformanceCards } from './CalendarPerformanceCards';
import { CalendarWeekDayHeader } from './CalendarWeekDayHeader';
import { CalendarWeekSection } from './CalendarWeekSection';

type CalendarGridProps = {
  apiBaseUrl: string;
};

export function CalendarGrid({ apiBaseUrl }: CalendarGridProps) {
  const { t } = useTranslation();
  const {
    state,
    weeks,
    topPreviewWeek,
    bottomPreviewWeek,
    isLoadingPast,
    isLoadingFuture,
    scrollAdjustment,
    loadMorePast,
    loadMoreFuture,
  } = useCalendarData({ apiBaseUrl });
  const scrollRef = useRef<HTMLDivElement>(null);
  const appliedAdjustmentVersionRef = useRef(0);
  const initializingScrollRef = useRef(true);

  const visibleRangeLabel = useMemo(() => {
    const firstWeek = weeks[0];
    const lastWeek = weeks[weeks.length - 1];

    if (!firstWeek || !lastWeek) {
      return t('calendar.fiveWeeks');
    }

    const firstLabel = new Intl.DateTimeFormat(undefined, { month: 'long', year: 'numeric' }).format(firstWeek.mondayDate);
    const lastDate = lastWeek.days[lastWeek.days.length - 1]?.date ?? lastWeek.mondayDate;
    const lastLabel = new Intl.DateTimeFormat(undefined, { month: 'long', year: 'numeric' }).format(lastDate);

    return firstLabel === lastLabel ? firstLabel : `${firstLabel} - ${lastLabel}`;
  }, [t, weeks]);

  const handleReachTop = useCallback(() => {
    if (isLoadingPast) {
      return;
    }

    void loadMorePast();
  }, [isLoadingPast, loadMorePast]);

  const handleReachBottom = useCallback(() => {
    if (isLoadingFuture) {
      return;
    }

    void loadMoreFuture();
  }, [isLoadingFuture, loadMoreFuture]);

  const handleScroll = useCallback(() => {
    const container = scrollRef.current;
    if (!container || state !== 'ready') {
      return;
    }

    const topTrigger = CALENDAR_PREVIEW_VISIBLE_HEIGHT;
    const bottomTrigger = (CALENDAR_WEEK_ROW_HEIGHT * 2) - CALENDAR_PREVIEW_VISIBLE_HEIGHT;

    if (container.scrollTop <= topTrigger) {
      void handleReachTop();
      return;
    }

    if (container.scrollTop >= bottomTrigger) {
      void handleReachBottom();
    }
  }, [handleReachBottom, handleReachTop, state]);

  useLayoutEffect(() => {
    const container = scrollRef.current;

    if (!container || scrollAdjustment.version === 0 || appliedAdjustmentVersionRef.current === scrollAdjustment.version) {
      return;
    }

    container.scrollTop += scrollAdjustment.topDelta;
    appliedAdjustmentVersionRef.current = scrollAdjustment.version;
  }, [scrollAdjustment]);

  useLayoutEffect(() => {
    const container = scrollRef.current;
    if (!container || !initializingScrollRef.current || weeks.length === 0) {
      return;
    }

    container.scrollTop = CALENDAR_ANCHOR_SCROLL_TOP;
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
              className="no-scrollbar overflow-y-auto pr-1"
              style={{
                maxHeight: `${(CALENDAR_WEEK_ROW_HEIGHT * 5) + (CALENDAR_PREVIEW_VISIBLE_HEIGHT * 2)}px`,
              }}
            >
              <CalendarWeekDayHeader />
              <div className="mt-8 space-y-10">
                <PreviewRow week={topPreviewWeek} edge="top" />
                {state === 'loading' && weeks.length === 0 ? (
                  weeks.map((week) => <CalendarWeekSection key={week.weekKey} week={week} />)
                ) : weeks.length > 0 ? (
                  weeks.map((week) => <CalendarWeekSection key={week.weekKey} week={week} />)
                ) : (
                  <div className="rounded-xl border border-white/5 bg-[#171a1d] p-6 text-center text-sm text-slate-400">
                    {t('calendar.noEvents')}
                  </div>
                )}
                <PreviewRow week={bottomPreviewWeek} edge="bottom" />
              </div>
            </div>
          </div>
        </div>
      </div>

      <CalendarPerformanceCards />
    </section>
  );
}

function PreviewRow({ week, edge }: { week: CalendarWeek; edge: 'top' | 'bottom' }) {
  const transform = edge === 'top'
    ? `translateY(-${CALENDAR_WEEK_ROW_HEIGHT - CALENDAR_PREVIEW_VISIBLE_HEIGHT}px)`
    : 'translateY(0)';
  const overlayClass = edge === 'top'
    ? 'bg-gradient-to-b from-[#0a0f1a] via-[#0a0f1a]/86 to-transparent'
    : 'bg-gradient-to-t from-[#0a0f1a] via-[#0a0f1a]/86 to-transparent';

  return (
    <div
      aria-hidden="true"
      className="pointer-events-none relative overflow-hidden opacity-50"
      style={{ height: `${CALENDAR_PREVIEW_VISIBLE_HEIGHT}px` }}
    >
      <div className="scale-[0.992] blur-[0.6px] saturate-75 brightness-[0.82]" style={{ transform }}>
        <CalendarWeekSection week={week} />
      </div>
      <div className={`absolute inset-0 ${overlayClass}`} />
    </div>
  );
}

function MetricPill({ title, value, accent }: { title: string; value: string; accent: string }) {
  return (
    <div className="rounded-xl border border-white/5 bg-[#171a1d]/70 px-4 py-3 sm:min-w-[12rem]">
      <span className="block text-[10px] font-bold uppercase tracking-[0.24em] text-slate-500">{title}</span>
      <span className={`mt-2 block text-sm font-bold uppercase tracking-tight ${accent}`}>{value}</span>
    </div>
  );
}
