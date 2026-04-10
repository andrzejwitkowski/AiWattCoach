import { RacesPageLayout } from '../features/races/components/RacesPageLayout';

type RacesPageProps = {
  apiBaseUrl: string;
};

export function RacesPage({ apiBaseUrl }: RacesPageProps) {
  return <RacesPageLayout apiBaseUrl={apiBaseUrl} />;
}
