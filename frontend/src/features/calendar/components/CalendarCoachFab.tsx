import { Bot, Sparkles } from 'lucide-react';
import { useTranslation } from 'react-i18next';

type CalendarCoachFabProps = {
  onClick: () => void;
};

export function CalendarCoachFab({ onClick }: CalendarCoachFabProps) {
  const { t } = useTranslation();

  return (
    <button
      type="button"
      onClick={onClick}
      aria-label={t('calendar.openCoach')}
      className="fixed bottom-4 right-4 z-40 inline-flex items-center gap-3 overflow-hidden rounded-full border border-[#d2ff9a]/30 bg-[#0f1511]/95 px-4 py-3 text-left shadow-[0_20px_60px_rgba(0,0,0,0.45)] backdrop-blur transition hover:-translate-y-0.5 hover:border-[#d2ff9a]/55 hover:bg-[#131a15] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#d2ff9a]/70 focus-visible:ring-offset-2 focus-visible:ring-offset-[#0a0f1a] md:bottom-6 md:right-6 md:px-5"
    >
      <span aria-hidden="true" className="absolute inset-0 bg-[radial-gradient(circle_at_top,rgba(210,255,154,0.2),transparent_58%)]" />
      <span className="relative flex h-12 w-12 shrink-0 items-center justify-center rounded-full border border-[#d2ff9a]/30 bg-[#1f2a1b] text-[#d2ff9a] shadow-[0_0_25px_rgba(210,255,154,0.18)]">
        <Bot size={20} />
      </span>
      <span className="relative hidden min-w-0 flex-col md:flex">
        <span className="text-sm font-black uppercase tracking-[0.14em] text-[#f3ffe2]">
          {t('calendar.coachFabLabel')}
        </span>
      </span>
      <span className="relative text-[#d2ff9a]">
        <Sparkles size={16} />
      </span>
    </button>
  );
}
