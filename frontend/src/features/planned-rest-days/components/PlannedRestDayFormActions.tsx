import type { Ref } from 'react';
import { useTranslation } from 'react-i18next';

type PlannedRestDayFormActionsProps = {
  cancelButtonRef: Ref<HTMLButtonElement>;
  isEditing: boolean;
  isValid: boolean;
  isSaving: boolean;
  isDeleting: boolean;
  onCancel: () => void;
  onDelete: () => void;
  onSave: () => void;
};

export function PlannedRestDayFormActions({
  cancelButtonRef,
  isEditing,
  isValid,
  isSaving,
  isDeleting,
  onCancel,
  onDelete,
  onSave,
}: PlannedRestDayFormActionsProps) {
  const { t } = useTranslation();
  const busy = isSaving || isDeleting;

  return (
    <div className="mt-8 flex flex-wrap items-center justify-between gap-3">
      <div className="flex gap-2">
        <button
          ref={cancelButtonRef}
          type="button"
          onClick={onCancel}
          disabled={busy}
          className="rounded-full border border-white/10 px-4 py-2 text-sm font-semibold text-slate-300"
        >
          {t('plannedRestDays.cancel')}
        </button>
        {isEditing ? (
          <button
            type="button"
            onClick={onDelete}
            disabled={busy}
            className="rounded-full border border-rose-400/30 px-4 py-2 text-sm font-semibold text-rose-200"
          >
            {isDeleting ? t('plannedRestDays.deleting') : t('plannedRestDays.delete')}
          </button>
        ) : null}
      </div>

      <button
        type="button"
        onClick={onSave}
        disabled={!isValid || busy}
        className="rounded-full bg-violet-300 px-5 py-2 text-sm font-black uppercase tracking-[0.16em] text-slate-950 disabled:opacity-50"
      >
        {isSaving ? t('plannedRestDays.saving') : t('plannedRestDays.save')}
      </button>
    </div>
  );
}
