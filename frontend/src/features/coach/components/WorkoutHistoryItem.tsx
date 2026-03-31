import { MessageSquareMore } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import type { CoachWorkoutListItem } from '../types';

type WorkoutHistoryItemProps = {
  item: CoachWorkoutListItem;
  isSelected: boolean;
  onSelect: () => void;
};

function formatDateLabel(value: string): string {
  return new Intl.DateTimeFormat(undefined, {
    month: 'short',
    day: 'numeric',
    year: 'numeric',
  }).format(new Date(value));
}

export function WorkoutHistoryItem({ item, isSelected, onSelect }: WorkoutHistoryItemProps) {
  const { t } = useTranslation();
  const name = item.event.name?.trim() || t('coach.untitledWorkout');
  const statusLabel = item.hasConversation ? t('coach.statusDone') : t('coach.statusPending');

  return (
    <button
      type="button"
      className={[
        'w-full rounded-2xl border p-4 text-left transition',
        isSelected
          ? 'border-cyan-300/30 bg-cyan-300/10 shadow-[0_0_0_1px_rgba(103,232,249,0.08)]'
          : 'border-transparent bg-transparent hover:border-white/10 hover:bg-white/5',
      ].join(' ')}
      onClick={onSelect}
    >
      <div className="flex items-start justify-between gap-3">
        <div>
          <p className="text-[10px] font-bold uppercase tracking-[0.28em] text-cyan-200">
            {isSelected ? t('coach.activeWorkout') : statusLabel}
          </p>
          <p className="mt-3 text-2xl font-medium text-white">{name}</p>
          <p className="mt-1 text-sm text-slate-400">{formatDateLabel(item.event.startDateLocal)}</p>
        </div>
        <div className={item.hasConversation ? 'text-cyan-300' : 'text-slate-600'}>
          <MessageSquareMore size={18} />
        </div>
      </div>
    </button>
  );
}
