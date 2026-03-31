import { useTranslation } from 'react-i18next';

type ConfirmWithoutChatModalProps = {
  isOpen: boolean;
  isSaving: boolean;
  onCancel: () => void;
  onConfirm: () => void;
};

export function ConfirmWithoutChatModal({
  isOpen,
  isSaving,
  onCancel,
  onConfirm,
}: ConfirmWithoutChatModalProps) {
  const { t } = useTranslation();

  if (!isOpen) {
    return null;
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-[#070b12]/80 px-4 backdrop-blur-sm">
      <div className="w-full max-w-md rounded-2xl border border-white/10 bg-[#111417] p-6 shadow-2xl">
        <h2 className="text-2xl font-bold text-white">{t('coach.confirmWithoutChatTitle')}</h2>
        <p className="mt-3 text-sm leading-6 text-slate-300">{t('coach.confirmWithoutChatBody')}</p>
        <div className="mt-6 flex gap-3">
          <button
            type="button"
            className="flex-1 rounded-xl border border-white/10 px-4 py-3 text-sm font-semibold text-slate-200 transition hover:bg-white/5"
            onClick={onCancel}
          >
            {t('coach.cancel')}
          </button>
          <button
            type="button"
            className="flex-1 rounded-xl bg-cyan-300 px-4 py-3 text-sm font-semibold text-slate-950 transition hover:bg-cyan-200 disabled:cursor-not-allowed disabled:opacity-50"
            disabled={isSaving}
            onClick={onConfirm}
          >
            {isSaving ? t('coach.saving') : t('coach.confirmSave')}
          </button>
        </div>
      </div>
    </div>
  );
}
