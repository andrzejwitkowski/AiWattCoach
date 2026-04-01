import { useTranslation } from 'react-i18next';

import { RpeButton } from './RpeButton';
import { RpeScaleLabels } from './RpeScaleLabels';

type RpeSelectorProps = {
  value: number | null;
  disabled?: boolean;
  onChange: (value: number) => void;
};

export function RpeSelector({ value, disabled = false, onChange }: RpeSelectorProps) {
  const { t } = useTranslation();

  return (
    <section className="relative overflow-hidden rounded-2xl border border-white/10 bg-white/5 p-6 md:p-8">
      <div className="absolute right-6 top-5 text-slate-700">
        <div className="rounded-full bg-white/5 p-5">
          <span className="text-5xl">◔</span>
        </div>
      </div>
      <div className="max-w-5xl space-y-6">
        <div>
          <h2 className="text-2xl font-bold text-white">{t('coach.rpeTitle')}</h2>
          <p className="mt-1 text-sm text-slate-400">{t('coach.rpeDescription')}</p>
        </div>
        <div className="grid grid-cols-5 gap-2 md:grid-cols-10">
          {Array.from({ length: 10 }, (_, index) => index + 1).map((rpe) => (
            <RpeButton
              key={rpe}
              value={rpe}
              isSelected={value === rpe}
              disabled={disabled}
              onClick={() => {
                onChange(rpe);
              }}
            />
          ))}
        </div>
        <RpeScaleLabels />
      </div>
    </section>
  );
}
