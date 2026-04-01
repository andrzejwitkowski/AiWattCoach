type WorkoutCategoryTagProps = {
  label: string;
  tone?: 'primary' | 'neutral';
};

export function WorkoutCategoryTag({ label, tone = 'neutral' }: WorkoutCategoryTagProps) {
  return (
    <span
      className={[
        'rounded-full px-3 py-1 text-[10px] font-bold uppercase tracking-[0.22em]',
        tone === 'primary'
          ? 'border border-cyan-300/20 bg-cyan-300/10 text-cyan-200'
          : 'border border-white/10 bg-white/5 text-slate-300',
      ].join(' ')}
    >
      {label}
    </span>
  );
}
