import { Dumbbell } from 'lucide-react';
import { useTranslation } from 'react-i18next';

export function EmptyWorkoutState() {
  const { t } = useTranslation();

  return (
    <div className="glass-panel flex min-h-[22rem] flex-col items-center justify-center rounded-2xl border border-white/10 px-6 text-center">
      <div className="mb-4 flex h-16 w-16 items-center justify-center rounded-full border border-cyan-300/20 bg-cyan-300/10 text-cyan-200">
        <Dumbbell size={24} />
      </div>
      <h2 className="text-2xl font-bold text-white">{t('coach.emptyStateTitle')}</h2>
      <p className="mt-2 max-w-lg text-slate-400">{t('coach.emptyStateBody')}</p>
    </div>
  );
}
