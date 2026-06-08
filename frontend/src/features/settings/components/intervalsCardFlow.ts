import { testIntervalsConnection } from '../api/settings';
import { connectionErrorStatus } from './settingsCardFlow';
import type { SettingsStatus } from './SettingsStatusBanner';
import type { TestIntervalsConnectionResponse } from '../types';

type IntervalsTestDraft = {
  apiKey: string;
  athleteId: string;
};

type RunIntervalsConnectionTestOptions = {
  apiBaseUrl: string;
  submittedTestRequest: Record<string, string>;
  draft: IntervalsTestDraft;
  testRunId: number;
  getCurrentTestRunId: () => number;
  onStalePersistedUpdate: (request: Record<string, string>) => void;
  onSave: () => void;
  setStatus: (status: SettingsStatus) => void;
  setCleanDraft: (draft: IntervalsTestDraft) => void;
  setIsTesting: (value: boolean) => void;
};

export async function runIntervalsConnectionTest({
  apiBaseUrl,
  submittedTestRequest,
  draft,
  testRunId,
  getCurrentTestRunId,
  onStalePersistedUpdate,
  onSave,
  setStatus,
  setCleanDraft,
  setIsTesting,
}: RunIntervalsConnectionTestOptions) {
  try {
    const result = await testIntervalsConnection(apiBaseUrl, submittedTestRequest);
    if (testRunId !== getCurrentTestRunId()) {
      if (result.persistedStatusUpdated) {
        onStalePersistedUpdate(submittedTestRequest);
        onSave();
      }
      return;
    }

    setStatusFromIntervalsTest(result, setStatus);
    if (result.persistedStatusUpdated) {
      setCleanDraft(draft);
      onSave();
    }
  } catch (err) {
    if (testRunId !== getCurrentTestRunId()) {
      return;
    }
    setStatus(connectionErrorStatus(err, 'Failed to test Intervals.icu connection'));
  } finally {
    if (testRunId === getCurrentTestRunId()) {
      setIsTesting(false);
    }
  }
}

function setStatusFromIntervalsTest(
  result: TestIntervalsConnectionResponse,
  setStatus: (status: SettingsStatus) => void,
) {
  setStatus({
    tone: result.connected ? 'success' : 'error',
    label: result.connected ? 'OK' : 'FAILED',
    message: result.message,
  });
}
