import type { ReactNode } from 'react';

type PlannedRestDayFormFieldProps = {
  label: string;
  children: ReactNode;
};

export function PlannedRestDayFormField({ label, children }: PlannedRestDayFormFieldProps) {
  return (
    <label className="block space-y-2">
      <span className="text-xs font-bold uppercase tracking-[0.2em] text-slate-500">{label}</span>
      {children}
    </label>
  );
}

type PlannedRestDayModeButtonProps = {
  active: boolean;
  label: string;
  onClick: () => void;
};

export function PlannedRestDayModeButton({ active, label, onClick }: PlannedRestDayModeButtonProps) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-pressed={active}
      className={[
        'rounded-2xl border px-4 py-3 text-sm font-semibold transition',
        active
          ? 'border-violet-300/50 bg-violet-300/12 text-violet-100'
          : 'border-white/10 bg-black/10 text-slate-400 hover:text-slate-200',
      ].join(' ')}
    >
      {label}
    </button>
  );
}

export const plannedRestDayFormInputClassName =
  'w-full rounded-2xl border border-white/10 bg-black/20 px-4 py-3 text-sm text-white';
