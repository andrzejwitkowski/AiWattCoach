import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import '../../../i18n';
import { ChatInput } from './ChatInput';

afterEach(() => {
  cleanup();
});

describe('ChatInput', () => {
  it('sends only once when the send button is clicked twice before onSend resolves', async () => {
    let resolveSend: ((value: boolean) => void) | undefined;
    const onSend = vi.fn(
      () =>
        new Promise<boolean>((resolve) => {
          resolveSend = resolve;
        }),
    );

    render(<ChatInput onSend={onSend} />);

    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'Legs felt strong' } });
    const sendButton = screen.getByRole('button', { name: /send/i });
    fireEvent.click(sendButton);
    fireEvent.click(sendButton);

    expect(onSend).toHaveBeenCalledTimes(1);
    expect(onSend).toHaveBeenCalledWith('Legs felt strong');
    expect(sendButton).toBeDisabled();

    resolveSend?.(true);

    await waitFor(() => {
      expect(screen.getByRole('textbox')).toHaveValue('');
    });
  });
});
