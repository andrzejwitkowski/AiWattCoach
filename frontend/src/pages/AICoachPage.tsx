import { CoachPageLayout } from '../features/coach/components/CoachPageLayout';

type AICoachPageProps = {
  apiBaseUrl: string;
};

export function AICoachPage({ apiBaseUrl }: AICoachPageProps) {
  return <CoachPageLayout apiBaseUrl={apiBaseUrl} />;
}
