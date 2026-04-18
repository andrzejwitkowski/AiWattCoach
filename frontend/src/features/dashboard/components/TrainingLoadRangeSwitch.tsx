import type { DashboardRange } from '../types';

type TrainingLoadRangeSwitchProps = {
  value: DashboardRange;
  onChange: (next: DashboardRange) => void;
};

const OPTIONS: Array<{ value: DashboardRange; label: string }> = [
  { value: '90d', label: '90 DAYS' },
  { value: 'season', label: 'SEASON' },
  { value: 'all-time', label: 'ALL TIME' },
];

export function TrainingLoadRangeSwitch({ value, onChange }: TrainingLoadRangeSwitchProps) {
  return (
    <fieldset className="flex w-full flex-wrap rounded-2xl border border-white/8 bg-[#0d131a] p-1.5 shadow-[0_20px_50px_rgba(2,8,23,0.35)] sm:inline-flex sm:w-auto">
      <legend className="sr-only">Dashboard time range</legend>
      {OPTIONS.map((option) => {
        const active = option.value === value;
        const optionId = `dashboard-range-${option.value}`;

        return (
          <div key={option.value} className="flex-1 sm:flex-none">
            <input
              id={optionId}
              name="dashboard-range"
              type="radio"
              className="peer sr-only"
              checked={active}
              onChange={() => { onChange(option.value); }}
            />
            <label
              htmlFor={optionId}
              className={[
                'block cursor-pointer rounded-xl px-3 py-2.5 text-center text-[11px] font-black tracking-[0.18em] transition sm:px-4 sm:tracking-[0.22em]',
                active
                  ? 'bg-[#202a36] text-white shadow-[inset_0_1px_0_rgba(255,255,255,0.04),0_8px_30px_rgba(15,23,42,0.45)]'
                  : 'text-slate-400 hover:bg-white/5 hover:text-slate-200',
                'peer-focus-visible:outline peer-focus-visible:outline-2 peer-focus-visible:outline-offset-2 peer-focus-visible:outline-cyan-300/70'
              ].join(' ')}
            >
              {option.label}
            </label>
          </div>
        );
      })}
    </fieldset>
  );
}
