import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import '../../../i18n';
import { ConfirmWithoutChatModal } from './ConfirmWithoutChatModal';

afterEach(() => {
  cleanup();
});

describe('ConfirmWithoutChatModal', () => {
  it('does not render while closed', () => {
    render(
      <ConfirmWithoutChatModal
        isOpen={false}
        isSaving={false}
        onCancel={() => undefined}
        onConfirm={() => undefined}
      />,
    );

    expect(screen.queryByText(/save without chatting/i)).not.toBeInTheDocument();
  });

  it('renders copy and emits cancel and confirm actions', () => {
    const onCancel = vi.fn();
    const onConfirm = vi.fn();

    render(
      <ConfirmWithoutChatModal
        isOpen
        isSaving={false}
        onCancel={onCancel}
        onConfirm={onConfirm}
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: /cancel/i }));
    fireEvent.click(screen.getByRole('button', { name: /yes, save it/i }));

    expect(screen.getByText(/this workout does not have a coach conversation yet/i)).toBeInTheDocument();
    expect(onCancel).toHaveBeenCalledTimes(1);
    expect(onConfirm).toHaveBeenCalledTimes(1);
  });
});
