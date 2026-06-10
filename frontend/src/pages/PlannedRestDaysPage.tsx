import { PlannedRestDaysPageLayout } from '../features/planned-rest-days/components/PlannedRestDaysPageLayout';

type PlannedRestDaysPageProps = {
  apiBaseUrl: string;
};

export function PlannedRestDaysPage({ apiBaseUrl }: PlannedRestDaysPageProps) {
  return <PlannedRestDaysPageLayout apiBaseUrl={apiBaseUrl} />;
}
