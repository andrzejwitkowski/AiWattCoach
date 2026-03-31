type RpeButtonProps = {
  value: number;
  isSelected: boolean;
  disabled?: boolean;
  onClick: () => void;
};

export function RpeButton({ value, isSelected, disabled = false, onClick }: RpeButtonProps) {
  return (
    <button
      type="button"
      className={[
        'h-12 rounded-xl border text-sm font-bold transition',
        isSelected
          ? 'border-cyan-300 bg-cyan-300 text-slate-950 shadow-[0_0_20px_rgba(103,232,249,0.35)]'
          : 'border-white/5 bg-black/20 text-slate-300 hover:border-cyan-300/30 hover:text-white',
        disabled ? 'cursor-not-allowed opacity-60' : '',
      ].join(' ')}
      disabled={disabled}
      onClick={onClick}
    >
      {value}
    </button>
  );
}
