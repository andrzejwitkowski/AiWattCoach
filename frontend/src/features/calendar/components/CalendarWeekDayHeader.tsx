import { useTranslation } from 'react-i18next';

const DAY_KEYS = [
  'calendar.monday',
  'calendar.tuesday',
  'calendar.wednesday',
  'calendar.thursday',
  'calendar.friday',
  'calendar.saturday',
  'calendar.sunday',
];

export function CalendarWeekDayHeader() {
  const { t } = useTranslation();

  return (
    <div className="calendar-grid sticky top-0 z-20 gap-4 border-b border-white/5 bg-[#0a0f1a]/95 px-1 pb-3 pt-2 backdrop-blur-xl">
      {DAY_KEYS.map((dayKey) => (
        <div
          key={dayKey}
          className="text-center text-[10px] font-bold uppercase tracking-[0.2em] text-slate-500"
        >
          {t(dayKey)}
        </div>
      ))}
    </div>
  );
}
