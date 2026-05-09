import { render, screen } from '@testing-library/react';
import { vi } from 'vitest';

import '../../../i18n';
import { ApiBaseUrlProvider } from '../../../lib/apiBaseUrl';
import { HttpError } from '../../../lib/httpClient';
import {
  downloadFit,
  loadCompletedWorkoutSummary,
  loadActivity,
  loadEvent,
  syncPlannedWorkoutToIntervals,
  syncPlannedWorkoutToWahoo,
} from '../../intervals/api/intervals';
import type { IntervalActivity, IntervalEvent } from '../../intervals/types';
import { makeSelection } from '../testData';
import type { WorkoutDetailSelection } from '../workoutDetails';
import { WorkoutDetailModal } from './WorkoutDetailModal';

vi.mock('../../intervals/api/intervals', () => ({
  downloadFit: vi.fn(),
  loadCompletedWorkoutSummary: vi.fn(),
  loadEvent: vi.fn(),
  loadActivity: vi.fn(),
  syncPlannedWorkoutToIntervals: vi.fn(),
  syncPlannedWorkoutToWahoo: vi.fn(),
}));

export const mockedDownloadFit = vi.mocked(downloadFit);
export const mockedLoadCompletedWorkoutSummary = vi.mocked(loadCompletedWorkoutSummary);
export const mockedLoadActivity = vi.mocked(loadActivity);
export const mockedLoadEvent = vi.mocked(loadEvent);
export const mockedSyncPlannedWorkoutToIntervals = vi.mocked(syncPlannedWorkoutToIntervals);
export const mockedSyncPlannedWorkoutToWahoo = vi.mocked(syncPlannedWorkoutToWahoo);

mockedLoadCompletedWorkoutSummary.mockImplementation(async () => {
  throw new HttpError(404, 'missing');
});

type RenderWorkoutDetailModalOptions = {
  selection?: WorkoutDetailSelection;
  event?: IntervalEvent | null;
  activity?: IntervalActivity | null;
  apiBaseUrl?: string;
  onClose?: () => void;
};

export function renderWorkoutDetailModal(options: RenderWorkoutDetailModalOptions = {}) {
  const selection =
    options.selection ??
    makeSelection({
      event: options.event ?? null,
      activity: options.activity ?? null,
    });
  const onClose = options.onClose ?? vi.fn();

  return {
    onClose,
    selection,
    ...render(
      <ApiBaseUrlProvider value={options.apiBaseUrl ?? ''}>
        <WorkoutDetailModal
          selection={selection}
          onClose={onClose}
        />
      </ApiBaseUrlProvider>,
    ),
  };
}

export function metricCard(label: string) {
  return screen.getByText(label).closest('div') as HTMLElement;
}

export function setChartRect(chart: HTMLElement, width = 1000, height = 220) {
  Object.defineProperty(chart, 'getBoundingClientRect', {
    configurable: true,
    value: () => ({
      left: 0,
      top: 0,
      width,
      height,
      right: width,
      bottom: height,
      x: 0,
      y: 0,
      toJSON: () => ({}),
    }),
  });
}
