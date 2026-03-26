import { CalendarGrid } from '../features/calendar/components/CalendarGrid';

type CalendarPageProps = {
  apiBaseUrl: string;
};

export function CalendarPage({ apiBaseUrl }: CalendarPageProps) {
  return <CalendarGrid apiBaseUrl={apiBaseUrl} />;
}
