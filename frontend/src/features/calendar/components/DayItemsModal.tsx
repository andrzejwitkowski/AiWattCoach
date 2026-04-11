import { Bike, Flag, Trophy, X } from 'lucide-react';
import { useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';

import { useDialogFocusTrap } from '../../../lib/useDialogFocusTrap';
import type { CalendarDayItemsSelection, CalendarDayItem } from '../dayItems';

type DayItemsModalProps = {
  selection: CalendarDayItemsSelection | null;
  onClose: () => void;
  onSelectItem: (item: CalendarDayItem) => void;
};

export function DayItemsModal({ selection, onClose, onSelectItem }: DayItemsModalProps) {
  const { t } = useTranslation();
  const dialogRef = useRef<HTMLDivElement>(null);
  const closeButtonRef = useRef<HTMLButtonElement>(null);

  useDialogFocusTrap(Boolean(selection), dialogRef, closeButtonRef);

  useEffect(() => {
    if (!selection) {
      return undefined;
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        onClose();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => {
      window.removeEventListener('keydown', handleKeyDown);
    };
  }, [selection, onClose]);

  if (!selection) {
    return null;
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/65 px-4 py-6 backdrop-blur-sm" onClick={onClose}>
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby="day-items-title"
        tabIndex={-1}
        className="w-full max-w-2xl overflow-hidden rounded-[1.5rem] border border-white/8 bg-[#111417] shadow-[0_24px_80px_rgba(0,0,0,0.5)]"
        onClick={(event) => event.stopPropagation()}
      >
        <div className="flex items-center justify-between border-b border-white/6 px-6 py-4 md:px-8">
          <div>
            <p className="text-[10px] font-black uppercase tracking-[0.28em] text-slate-500">{t('calendar.dayItems')}</p>
            <h2 id="day-items-title" className="mt-2 text-2xl font-black uppercase tracking-tight text-[#f9f9fd] md:text-3xl">
              {t('calendar.viewItems', { count: selection.items.length })}
            </h2>
          </div>
          <button
            ref={closeButtonRef}
            type="button"
            onClick={onClose}
            aria-label={t('calendar.closeDayItems')}
            className="rounded-full border border-white/10 bg-white/5 p-2 text-slate-300 transition hover:bg-white/10 hover:text-white"
          >
            <X size={18} />
          </button>
        </div>

        <div className="max-h-[70vh] space-y-3 overflow-y-auto px-6 py-6 md:px-8">
          {selection.items.map((item) => (
            <button
              key={item.id}
              type="button"
              onClick={item.kind === 'event' ? undefined : () => onSelectItem(item)}
              disabled={item.kind === 'event'}
              className="flex w-full items-center justify-between gap-4 rounded-2xl border border-white/8 bg-white/[0.03] px-4 py-4 text-left transition hover:bg-white/[0.06] disabled:cursor-default disabled:opacity-75"
            >
              <div className="flex min-w-0 items-center gap-3">
                <span className="flex h-10 w-10 shrink-0 items-center justify-center rounded-2xl border border-white/8 bg-white/[0.04] text-slate-200">
                  <ItemIcon item={item} />
                </span>
                <div className="min-w-0">
                  <p className="text-[10px] font-black uppercase tracking-[0.22em] text-slate-500">{itemLabel(item, t)}</p>
                  <p className="truncate text-sm font-bold text-[#f9f9fd]">{item.title}</p>
                </div>
              </div>
              {item.subtitle ? (
                <p className="shrink-0 text-xs font-semibold text-slate-300">{item.subtitle}</p>
              ) : null}
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}

function ItemIcon({ item }: { item: CalendarDayItem }) {
  switch (item.kind) {
    case 'race':
      return <Trophy size={16} />;
    case 'planned':
      return <Flag size={16} />;
    case 'completed':
    case 'event':
    default:
      return <Bike size={16} />;
  }
}

function itemLabel(item: CalendarDayItem, t: ReturnType<typeof useTranslation>['t']): string {
  switch (item.kind) {
    case 'race':
      return t('calendar.raceDay');
    case 'planned':
      return t('calendar.plannedWorkout');
    case 'completed':
      return t('calendar.completedWorkout');
    case 'event':
    default:
      return t('calendar.eventOther');
  }
}
