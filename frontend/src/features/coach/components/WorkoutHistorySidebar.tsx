import { useTranslation } from 'react-i18next';

import type { CoachWorkoutListItem } from '../types';
import { WorkoutHistoryItem } from './WorkoutHistoryItem';
import { WorkoutHistoryPagination } from './WorkoutHistoryPagination';

type WorkoutHistorySidebarProps = {
  items: CoachWorkoutListItem[];
  selectedWorkoutId: string | null;
  state: 'loading' | 'ready' | 'error' | 'credentials-required';
  error: string | null;
  weekLabel: string;
  compact?: boolean;
  canGoToNewerWeek: boolean;
  onOlderWeek: () => void;
  onNewerWeek: () => void;
  onSelectWorkout: (workoutId: string) => void;
};

export function WorkoutHistorySidebar({
  items,
  selectedWorkoutId,
  state,
  error,
  weekLabel,
  compact = false,
  canGoToNewerWeek,
  onOlderWeek,
  onNewerWeek,
  onSelectWorkout,
}: WorkoutHistorySidebarProps) {
  const { t } = useTranslation();

  return (
    <aside className={[
      'rounded-2xl border border-white/10 bg-white/5',
      compact ? 'p-4' : 'p-6',
    ].join(' ')}>
      <h2 className={[
        'text-sm font-bold uppercase tracking-[0.28em] text-slate-400',
        compact ? 'mb-4' : 'mb-6',
      ].join(' ')}>
        {t('coach.previousWorkouts')}
      </h2>
      <WorkoutHistoryPagination
        weekLabel={weekLabel}
        canGoToNewerWeek={canGoToNewerWeek}
        onOlderWeek={onOlderWeek}
        onNewerWeek={onNewerWeek}
      />
      <div className={compact ? 'grid auto-cols-[16rem] grid-flow-col gap-3 overflow-x-auto pb-1' : 'space-y-3'}>
        {state === 'loading' ? (
          <div className="rounded-2xl border border-white/10 bg-black/20 px-4 py-10 text-center text-slate-400">
            {t('coach.loadingWorkouts')}
          </div>
        ) : null}
        {state === 'credentials-required' ? (
          <div className="rounded-2xl border border-amber-300/20 bg-amber-300/10 px-4 py-6 text-sm text-amber-100">
            {t('calendar.connectionRequired')}
          </div>
        ) : null}
        {state === 'error' ? (
          <div className="rounded-2xl border border-red-400/25 bg-red-500/10 px-4 py-6 text-sm text-red-200">
            {error ?? t('coach.loadingError')}
          </div>
        ) : null}
        {state === 'ready' && items.length === 0 ? (
          <div className="rounded-2xl border border-white/10 bg-black/20 px-4 py-10 text-center text-slate-400">
            {t('coach.noWorkouts')}
          </div>
        ) : null}
        {state === 'ready'
          ? items.map((item) => (
            <WorkoutHistoryItem
              key={item.id}
              item={item}
              isSelected={selectedWorkoutId === item.id}
              onSelect={() => {
                onSelectWorkout(item.id);
              }}
            />
          ))
          : null}
      </div>
    </aside>
  );
}
