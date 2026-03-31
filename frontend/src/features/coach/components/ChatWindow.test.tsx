import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import '../../../i18n';
import { ChatWindow } from './ChatWindow';

window.HTMLElement.prototype.scrollIntoView = vi.fn();

afterEach(() => {
  cleanup();
});

describe('ChatWindow', () => {
  it('renders coach messages and a typing indicator', () => {
    render(
      <ChatWindow
        messages={[
          {
            id: 'message-1',
            role: 'coach',
            content: 'Great job on the session.',
            createdAtEpochSeconds: 1711000200,
          },
        ]}
        isCoachTyping
        isConnected
        hasSelectedWorkout
        error={null}
        onSendMessage={async () => undefined}
      />,
    );

    expect(screen.getByText(/great job on the session/i)).toBeInTheDocument();
    expect(screen.getByText(/coach is typing/i)).toBeInTheDocument();
  });

  it('sends trimmed input text', async () => {
    const onSendMessage = vi.fn().mockResolvedValue(undefined);

    render(
      <ChatWindow
        messages={[]}
        isCoachTyping={false}
        isConnected
        hasSelectedWorkout
        error={null}
        onSendMessage={onSendMessage}
      />,
    );

    fireEvent.change(screen.getByPlaceholderText(/describe your muscle state/i), {
      target: { value: '  Heavy, but manageable  ' },
    });
    fireEvent.click(screen.getByRole('button', { name: /send message/i }));

    await waitFor(() => {
      expect(onSendMessage).toHaveBeenCalledWith('Heavy, but manageable');
    });
  });

  it('renders an empty selection prompt when no workout is selected', () => {
    render(
      <ChatWindow
        messages={[]}
        isCoachTyping={false}
        isConnected={false}
        hasSelectedWorkout={false}
        error={null}
        onSendMessage={async () => undefined}
      />,
    );

    expect(screen.getAllByText(/select a workout from the left panel/i)).toHaveLength(2);
  });
});
