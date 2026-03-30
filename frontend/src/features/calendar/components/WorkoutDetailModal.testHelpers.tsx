import { render, screen } from '@testing-library/react';
import { vi } from 'vitest';

import '../../../i18n';
import { downloadFit, loadActivity, loadEvent } from '../../intervals/api/intervals';
import type { IntervalActivity, IntervalEvent } from '../../intervals/types';
import { makeSelection } from '../testData';
import type { WorkoutDetailSelection } from '../workoutDetails';
import { WorkoutDetailModal } from './WorkoutDetailModal';

vi.mock('../../intervals/api/intervals', () => ({
  downloadFit: vi.fn(),
  loadEvent: vi.fn(),
  loadActivity: vi.fn(),
}));

export const mockedDownloadFit = vi.mocked(downloadFit);
export const mockedLoadActivity = vi.mocked(loadActivity);
export const mockedLoadEvent = vi.mocked(loadEvent);

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
      <WorkoutDetailModal
        apiBaseUrl={options.apiBaseUrl ?? ''}
        selection={selection}
        onClose={onClose}
      />,
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
