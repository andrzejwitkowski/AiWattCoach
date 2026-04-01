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
        isSaved={false}
        requiresRpe={false}
        error={null}
        onSendMessage={async () => true}
      />,
    );

    expect(screen.getByText(/great job on the session/i)).toBeInTheDocument();
    expect(screen.getByText(/coach is typing/i)).toBeInTheDocument();
  });

  it('sends trimmed input text', async () => {
    const onSendMessage = vi.fn().mockResolvedValue(true);

    render(
      <ChatWindow
        messages={[]}
        isCoachTyping={false}
        isConnected
        hasSelectedWorkout
        isSaved={false}
        requiresRpe={false}
        error={null}
        onSendMessage={onSendMessage}
      />,
    );

    fireEvent.change(screen.getAllByPlaceholderText(/describe your muscle state/i)[0], {
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
        isSaved={false}
        requiresRpe={false}
        error={null}
        onSendMessage={async () => true}
      />,
    );

    expect(screen.getAllByText(/select a workout from the left panel/i)[0]).toBeInTheDocument();
  });

  it('disables chat input when editing is locked', () => {
    render(
      <ChatWindow
        messages={[]}
        isCoachTyping={false}
        isConnected
        hasSelectedWorkout
        isSaved
        requiresRpe={false}
        error={null}
        inputDisabled
        onSendMessage={async () => true}
      />,
    );

    expect(screen.getAllByPlaceholderText(/describe your muscle state/i)[0]).toBeDisabled();
    expect(screen.getByRole('button', { name: /send message/i })).toBeDisabled();
  });

  it('shows rpe requirement before chat unlocks', () => {
    render(
      <ChatWindow
        messages={[]}
        isCoachTyping={false}
        isConnected={false}
        hasSelectedWorkout
        isSaved={false}
        requiresRpe
        error={null}
        inputDisabled
        onSendMessage={async () => true}
      />,
    );

    expect(screen.getByText(/choose an rpe first to unlock coaching/i)).toBeInTheDocument();
  });
});
