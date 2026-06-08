type IntervalsDraft = {
  apiKey: string;
  athleteId: string;
};

export function buildIntervalsSaveRequest(draft: IntervalsDraft, cleanDraft: IntervalsDraft) {
  const trimmedApiKey = draft.apiKey.trim();
  const trimmedAthleteId = draft.athleteId.trim();
  const cleanApiKey = cleanDraft.apiKey.trim();
  const cleanAthleteId = cleanDraft.athleteId.trim();
  const request: Record<string, string | null> = {};

  if (trimmedApiKey !== cleanApiKey) {
    request.apiKey = trimmedApiKey ? trimmedApiKey : null;
  }
  if (trimmedAthleteId !== cleanAthleteId) {
    request.athleteId = trimmedAthleteId ? trimmedAthleteId : null;
  }

  return request;
}

export function buildIntervalsTestRequest(draft: IntervalsDraft, cleanDraft: IntervalsDraft) {
  const trimmedApiKey = draft.apiKey.trim();
  const trimmedAthleteId = draft.athleteId.trim();
  const cleanApiKey = cleanDraft.apiKey.trim();
  const cleanAthleteId = cleanDraft.athleteId.trim();
  const request: Record<string, string> = {};

  if (trimmedApiKey && trimmedApiKey !== cleanApiKey) {
    request.apiKey = trimmedApiKey;
  }
  if (trimmedAthleteId && trimmedAthleteId !== cleanAthleteId) {
    request.athleteId = trimmedAthleteId;
  }

  return request;
}
