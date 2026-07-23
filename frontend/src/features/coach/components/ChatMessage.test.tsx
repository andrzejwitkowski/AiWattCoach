import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import '../../../i18n';
import { ChatMessage } from './ChatMessage';

afterEach(() => {
  cleanup();
});

describe('ChatMessage', () => {
  it('renders the power chart image when imageUrl is present', () => {
    render(
      <ChatMessage
        message={{
          id: 'message-1',
          role: 'user',
          content: 'How did I do?',
          createdAtEpochSeconds: 1711000200,
          imageUrl: '/api/workout-summaries/workout-1/power-chart.png',
        }}
      />,
    );

    const image = screen.getByRole('img', { name: 'Power chart' });
    expect(image).toHaveAttribute('src', '/api/workout-summaries/workout-1/power-chart.png');
  });

  it('maximizes the power chart on click and closes on backdrop click', () => {
    render(
      <ChatMessage
        message={{
          id: 'message-1',
          role: 'user',
          content: 'How did I do?',
          createdAtEpochSeconds: 1711000200,
          imageUrl: '/api/workout-summaries/workout-1/power-chart.png',
        }}
      />,
    );

    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Maximize power chart' }));
    expect(screen.getByRole('dialog')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('dialog'));
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('does not render an image when imageUrl is absent', () => {
    render(
      <ChatMessage
        message={{
          id: 'message-1',
          role: 'user',
          content: 'How did I do?',
          createdAtEpochSeconds: 1711000200,
        }}
      />,
    );

    expect(screen.queryByRole('img')).not.toBeInTheDocument();
  });
});
