export type DateMode = 'single' | 'range';

export type PlannedRestDayDraft = {
  mode: DateMode;
  startDate: string;
  endDate: string;
  title: string;
  note: string;
};

export const defaultPlannedRestDayDraft: PlannedRestDayDraft = {
  mode: 'single',
  startDate: '',
  endDate: '',
  title: '',
  note: '',
};

export function resolveDraftEndDate(draft: PlannedRestDayDraft): string {
  return draft.mode === 'single' ? draft.startDate : draft.endDate;
}

export function isPlannedRestDayDraftValid(draft: PlannedRestDayDraft): boolean {
  const endDate = resolveDraftEndDate(draft);
  return draft.startDate.length > 0 && endDate.length > 0 && endDate >= draft.startDate;
}
