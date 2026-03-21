import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { LandingPage } from './LandingPage';

describe('LandingPage', () => {
  it('renders the Google login call to action', () => {
    render(<LandingPage onLogin={() => {}} />);

    expect(screen.getByRole('button', { name: /sign in with google/i })).toBeInTheDocument();
    expect(screen.getByText(/welcome to wattly/i)).toBeInTheDocument();
  });
});
