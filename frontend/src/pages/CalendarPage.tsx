import { useState } from 'react';

import { CalendarCoachFab } from '../features/calendar/components/CalendarCoachFab';
import { CalendarCoachModal } from '../features/calendar/components/CalendarCoachModal';
import { CalendarGrid } from '../features/calendar/components/CalendarGrid';

type CalendarPageProps = {
  apiBaseUrl: string;
};

export function CalendarPage({ apiBaseUrl }: CalendarPageProps) {
  const [isCoachOpen, setIsCoachOpen] = useState(false);

  return (
    <>
      <CalendarGrid apiBaseUrl={apiBaseUrl} />
      <CalendarCoachFab onClick={() => setIsCoachOpen(true)} />
      <CalendarCoachModal isOpen={isCoachOpen} onClose={() => setIsCoachOpen(false)} />
    </>
  );
}
