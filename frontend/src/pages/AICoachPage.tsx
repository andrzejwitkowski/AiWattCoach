import { CoachPageLayout } from '../features/coach/components/CoachPageLayout';
import { CoachSessionCacheProvider } from '../features/coach/context/CoachSessionCache';

type AICoachPageProps = {
  apiBaseUrl: string;
};

export function AICoachPage({ apiBaseUrl }: AICoachPageProps) {
  return (
    <CoachSessionCacheProvider>
      <CoachPageLayout apiBaseUrl={apiBaseUrl} />
    </CoachSessionCacheProvider>
  );
}
