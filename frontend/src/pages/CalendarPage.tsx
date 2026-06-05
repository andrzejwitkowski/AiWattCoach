import { useState } from 'react';
import { Link } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { CalendarRange } from 'lucide-react';

import { ApiBaseUrlProvider } from '../lib/apiBaseUrl';
import { CalendarCoachFab } from '../features/calendar/components/CalendarCoachFab';
import { CalendarCoachModal } from '../features/calendar/components/CalendarCoachModal';
import { CalendarGrid } from '../features/calendar/components/CalendarGrid';

type CalendarPageProps = {
  apiBaseUrl: string;
};

export function CalendarPage({ apiBaseUrl }: CalendarPageProps) {
  const { t } = useTranslation();
  const [isCoachOpen, setIsCoachOpen] = useState(false);
  const [calendarRefreshVersion, setCalendarRefreshVersion] = useState(0);

  return (
    <ApiBaseUrlProvider value={apiBaseUrl}>
      <div className="border-b border-white/10 px-4 py-3 md:px-6">
        <Link
          className="inline-flex items-center gap-2 text-sm text-cyan-300 transition hover:text-cyan-200"
          to="/calendar/meso"
        >
          <CalendarRange className="h-4 w-4" />
          {t('mesoCycle.openMesoCalendar')}
        </Link>
      </div>
      <CalendarGrid refreshVersion={calendarRefreshVersion} />
      <CalendarCoachFab onClick={() => setIsCoachOpen(true)} />
      <CalendarCoachModal
        isOpen={isCoachOpen}
        onClose={() => setIsCoachOpen(false)}
        onPlannedWorkoutUpdated={() => setCalendarRefreshVersion((current) => current + 1)}
      />
    </ApiBaseUrlProvider>
  );
}
