import { useTranslation } from 'react-i18next';

type WorkoutActionButtonsProps = {
  disabled?: boolean;
  isSaving: boolean;
  isEditing: boolean;
  canSave?: boolean;
  onSave: () => void;
  onEdit: () => void;
};

export function WorkoutActionButtons({
  disabled = false,
  isSaving,
  isEditing,
  canSave = true,
  onSave,
  onEdit,
}: WorkoutActionButtonsProps) {
  const { t } = useTranslation();

  return (
    <div className="flex flex-col gap-4 sm:flex-row">
      <button
        type="button"
        className="flex-1 rounded-2xl bg-cyan-300 px-6 py-5 text-lg font-extrabold uppercase tracking-[0.18em] text-slate-950 transition hover:bg-cyan-200 disabled:cursor-not-allowed disabled:opacity-50"
        disabled={disabled || isSaving || !isEditing || !canSave}
        onClick={onSave}
      >
        {isSaving ? t('coach.saving') : t('coach.save')}
      </button>
      <button
        type="button"
        className="flex-1 rounded-2xl border border-cyan-300/25 px-6 py-5 text-lg font-extrabold uppercase tracking-[0.18em] text-cyan-200 transition hover:bg-cyan-300/5 disabled:cursor-not-allowed disabled:opacity-50"
        disabled={disabled || isEditing}
        onClick={onEdit}
      >
        {t('coach.edit')}
      </button>
    </div>
  );
}
