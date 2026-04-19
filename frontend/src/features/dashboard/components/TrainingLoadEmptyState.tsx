import { useTranslation } from 'react-i18next';

export function TrainingLoadEmptyState() {
  const { t } = useTranslation();

  return (
    <section className="rounded-[2rem] border border-dashed border-white/10 bg-[#0d141f] p-10 text-center shadow-[0_30px_80px_rgba(2,8,23,0.45)]">
      <p className="text-[11px] font-semibold uppercase tracking-[0.24em] text-slate-500">{t('dashboard.empty.eyebrow')}</p>
      <h2 className="mt-4 text-3xl font-semibold tracking-tight text-white">{t('dashboard.empty.title')}</h2>
      <p className="mx-auto mt-4 max-w-2xl text-sm leading-7 text-slate-400">
        {t('dashboard.empty.description')}
      </p>
    </section>
  );
}
