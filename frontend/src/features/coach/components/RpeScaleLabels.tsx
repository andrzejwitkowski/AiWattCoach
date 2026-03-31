import { useTranslation } from 'react-i18next';

export function RpeScaleLabels() {
  const { t } = useTranslation();

  return (
    <div className="mt-4 flex justify-between px-1 text-[10px] font-bold uppercase tracking-[0.28em] text-slate-500">
      <span>{t('coach.rpeRecovery')}</span>
      <span>{t('coach.rpeVeryHard')}</span>
      <span>{t('coach.rpeMaximum')}</span>
    </div>
  );
}
