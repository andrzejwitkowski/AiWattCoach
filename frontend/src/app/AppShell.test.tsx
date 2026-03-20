import { render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { describe, expect, it } from 'vitest';

import { AppShell } from './AppShell';

describe('AppShell', () => {
  it('renders navigation and backend connectivity sections', () => {
    render(
      <MemoryRouter>
        <AppShell
          backendStatus={{
            health: { status: 'ok', service: 'AiWattCoach' },
            readiness: { status: 'ok', reason: null },
            state: 'online',
            checkedAtLabel: 'just now'
          }}
        />
      </MemoryRouter>
    );

    expect(screen.getByRole('heading', { name: /aiwattcoach control center/i })).toBeInTheDocument();
    expect(screen.getByRole('link', { name: /settings/i })).toBeInTheDocument();
    expect(screen.getByText(/backend status/i)).toBeInTheDocument();
    expect(screen.getByText(/online/i)).toBeInTheDocument();
  });
});
