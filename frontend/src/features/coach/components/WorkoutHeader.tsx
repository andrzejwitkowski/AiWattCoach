import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';

import type { IntervalEvent } from '../../intervals/types';
import { formatDurationLabel } from '../../calendar/workoutDetails';
import { WorkoutCategoryTag } from './WorkoutCategoryTag';

type WorkoutHeaderProps = {
  event: IntervalEvent;
  hasConversation: boolean;
};

function formatWorkoutSubtitle(event: IntervalEvent): string {
  const formatter = new Intl.DateTimeFormat(undefined, {
    month: 'long',
    day: 'numeric',
    year: 'numeric',
  });
  const durationSeconds = event.actualWorkout?.matchedIntervals.reduce((total, interval) => {
    return Math.max(total, interval.actualEndTimeSeconds ?? 0);
  }, 0) || event.eventDefinition.summary.totalDurationSeconds;

  return `${formatter.format(new Date(event.startDateLocal))} • ${formatDurationLabel(durationSeconds)}`;
}

export function WorkoutHeader({ event, hasConversation }: WorkoutHeaderProps) {
  const { t } = useTranslation();
  const title = event.name?.trim() || t('coach.untitledWorkout');
  const subtitle = useMemo(() => formatWorkoutSubtitle(event), [event]);

  return (
    <header className="flex flex-col gap-4 lg:flex-row lg:items-end lg:justify-between">
      <div>
        <h1 className="text-4xl font-bold tracking-tight text-white">{title}</h1>
        <p className="mt-2 text-lg text-slate-400">{subtitle}</p>
      </div>
      <div className="flex flex-wrap gap-2">
        <WorkoutCategoryTag label={event.category} tone="primary" />
        <WorkoutCategoryTag label={hasConversation ? t('coach.statusDone') : t('coach.statusPending')} />
        <WorkoutCategoryTag label={event.indoor ? t('coach.indoor') : t('coach.outdoor')} />
      </div>
    </header>
  );
}
