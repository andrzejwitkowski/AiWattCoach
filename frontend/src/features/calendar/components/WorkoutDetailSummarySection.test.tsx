import {render, screen} from '@testing-library/react';
import {describe, expect, it} from 'vitest';

import {WorkoutSummarySection} from './WorkoutDetailSummarySection';

describe('WorkoutSummarySection', () => {
  it('renders plain text summaries without markdown parsing', () => {
    render(
      <WorkoutSummarySection
        isLoading={false}
        summary={{
          workoutId: 'w1',
          text: 'Strong aerobic control with only a small fade near the end.',
          generatedAtEpochSeconds: 1,
        }}
        summaryError={null}
      />,
    );

    expect(
      screen.getByText('Strong aerobic control with only a small fade near the end.'),
    ).toBeInTheDocument();
    expect(screen.queryByRole('heading')).not.toBeInTheDocument();
  });

  it('renders markdown summaries with formatted elements', () => {
    const {container} = render(
      <WorkoutSummarySection
        isLoading={false}
        summary={{
          workoutId: 'w1',
          text: '### Workout Recap\n\n**Execution Quality:** solid effort.',
          generatedAtEpochSeconds: 1,
        }}
        summaryError={null}
      />,
    );

    expect(screen.getByRole('heading', {name: 'Workout Recap'})).toBeInTheDocument();
    expect(screen.getByText('Execution Quality:')).toBeInTheDocument();
    expect(container.textContent).not.toContain('**Execution Quality:**');
    expect(container.textContent).not.toContain('### Workout Recap');
  });
});
