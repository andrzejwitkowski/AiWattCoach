import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import '../../../i18n';
import { RpeSelector } from './RpeSelector';

afterEach(() => {
  cleanup();
});

describe('RpeSelector', () => {
  it('renders ten selectable rpe buttons', () => {
    render(<RpeSelector value={7} onChange={() => undefined} />);

    expect(screen.getByRole('button', { name: '1' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '10' })).toBeInTheDocument();
    expect(screen.getByText(/rate of perceived exertion/i)).toBeInTheDocument();
  });

  it('calls onChange with the clicked rpe value', () => {
    const onChange = vi.fn();

    render(<RpeSelector value={null} onChange={onChange} />);

    fireEvent.click(screen.getByRole('button', { name: '8' }));

    expect(onChange).toHaveBeenCalledWith(8);
  });

  it('disables all rpe buttons when requested', () => {
    render(<RpeSelector value={5} disabled onChange={() => undefined} />);

    expect(screen.getByRole('button', { name: '5' })).toBeDisabled();
    expect(screen.getByRole('button', { name: '9' })).toBeDisabled();
  });
});
