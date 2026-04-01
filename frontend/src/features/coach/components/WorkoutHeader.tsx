import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';

import type { CoachWorkoutListItem } from '../types';
import { formatDurationLabel } from '../../calendar/workoutDetails';
import { WorkoutCategoryTag } from './WorkoutCategoryTag';

type WorkoutHeaderProps = {
  item: CoachWorkoutListItem;
  hasConversation: boolean;
};

function formatWorkoutSubtitle(item: CoachWorkoutListItem): string {
  const formatter = new Intl.DateTimeFormat(undefined, {
    month: 'long',
    day: 'numeric',
    year: 'numeric',
  });
  const durationSeconds = item.activity?.elapsedTimeSeconds
    ?? item.activity?.movingTimeSeconds
    ?? item.event?.actualWorkout?.matchedIntervals.reduce((total, interval) => {
      return Math.max(total, interval.actualEndTimeSeconds ?? 0);
    }, 0)
    ?? item.event?.eventDefinition.summary.totalDurationSeconds
    ?? null;

  const durationLabel = durationSeconds === null ? '--' : formatDurationLabel(durationSeconds);
  return `${formatter.format(new Date(item.startDateLocal))} • ${durationLabel}`;
}

export function WorkoutHeader({ item, hasConversation }: WorkoutHeaderProps) {
  const { t } = useTranslation();
  const title = item.activity?.name?.trim()
    || item.event?.name?.trim()
    || item.activity?.activityType?.trim()
    || t('coach.untitledWorkout');
  const subtitle = useMemo(() => formatWorkoutSubtitle(item), [item]);
  const primaryTag = item.activity?.activityType?.trim() || item.event?.category || t('coach.untitledWorkout');
  const locationTag = item.activity?.trainer || item.event?.indoor ? t('coach.indoor') : t('coach.outdoor');

  return (
    <header className="flex flex-col gap-4 lg:flex-row lg:items-end lg:justify-between">
      <div>
        <h1 className="text-4xl font-bold tracking-tight text-white">{title}</h1>
        <p className="mt-2 text-lg text-slate-400">{subtitle}</p>
      </div>
      <div className="flex flex-wrap gap-2">
        <WorkoutCategoryTag label={primaryTag} tone="primary" />
        <WorkoutCategoryTag label={hasConversation ? t('coach.statusDone') : t('coach.statusPending')} />
        <WorkoutCategoryTag label={locationTag} />
      </div>
    </header>
  );
}
