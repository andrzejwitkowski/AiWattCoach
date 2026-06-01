import type { ReactNode } from 'react';

type SectionCardProps = {
  title: string;
  expanded: boolean;
  onToggle: () => void;
  children: ReactNode;
};

export function SectionCard({ title, expanded, onToggle, children }: SectionCardProps) {
  return (
    <div className="min-w-0 overflow-hidden rounded-2xl border border-white/10 bg-white/[0.03]">
      <button
        type="button"
        onClick={onToggle}
        className="flex w-full items-center justify-between px-5 py-3 text-left text-xs font-semibold uppercase tracking-[0.15em] text-slate-400"
      >
        <span>{title}</span>
        <span className="text-slate-500">{expanded ? '▾' : '▸'}</span>
      </button>
      {expanded && <div className="border-t border-white/5 px-5 py-4">{children}</div>}
    </div>
  );
}
