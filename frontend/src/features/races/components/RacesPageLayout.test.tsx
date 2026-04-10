import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import '../../../i18n';
import type { Race } from '../types';
import { RacesPageLayout } from './RacesPageLayout';

vi.mock('../hooks/useRaces', () => ({
  useRaces: vi.fn(),
}));

vi.mock('../api/races', () => ({
  createRace: vi.fn(),
  updateRace: vi.fn(),
}));

import { useRaces } from '../hooks/useRaces';
import { createRace, updateRace } from '../api/races';

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

function makeRace(overrides: Partial<Race> = {}): Race {
  return {
    raceId: 'race-1',
    date: '2026-09-12',
    name: 'Gravel Attack',
    distanceMeters: 120000,
    discipline: 'gravel',
    priority: 'A',
    syncStatus: 'synced',
    linkedIntervalsEventId: 41,
    lastError: null,
    ...overrides,
  };
}

describe('RacesPageLayout', () => {
  it('renders upcoming and completed race sections', () => {
    vi.mocked(useRaces).mockReturnValue({
      races: [makeRace(), makeRace({ raceId: 'race-2', date: '2026-07-01', name: 'Past Challenge' })],
      upcomingRaces: [makeRace()],
      completedRaces: [makeRace({ raceId: 'race-2', date: '2026-07-01', name: 'Past Challenge' })],
      isLoading: false,
      error: null,
      refresh: vi.fn(),
    });

    render(<RacesPageLayout apiBaseUrl="" />);

    expect(screen.getByText(/road races/i)).toBeInTheDocument();
    expect(screen.getByText(/upcoming races/i)).toBeInTheDocument();
    expect(screen.getByText(/completed races/i)).toBeInTheDocument();
    expect(screen.getByText('Gravel Attack')).toBeInTheDocument();
    expect(screen.getByText('Past Challenge')).toBeInTheDocument();
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('creates a race from the form and refreshes the list', async () => {
    const refresh = vi.fn().mockResolvedValue(undefined);
    vi.mocked(useRaces).mockReturnValue({
      races: [],
      upcomingRaces: [],
      completedRaces: [],
      isLoading: false,
      error: null,
      refresh,
    });
    vi.mocked(createRace).mockResolvedValue(makeRace());

    render(<RacesPageLayout apiBaseUrl="" />);

    fireEvent.click(screen.getByRole('button', { name: /add race/i }));

    expect(screen.getByRole('dialog')).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText(/race name/i), { target: { value: 'Tour Test' } });
    fireEvent.change(screen.getByLabelText(/^date$/i), { target: { value: '2026-09-18' } });
    fireEvent.change(screen.getByLabelText(/distance \(km\)/i), { target: { value: '85' } });
    fireEvent.change(screen.getByLabelText(/discipline/i), { target: { value: 'road' } });
    fireEvent.click(screen.getByRole('button', { name: /cat\. a/i }));
    fireEvent.click(screen.getAllByRole('button', { name: /add race/i }).at(-1)!);

    await waitFor(() => {
      expect(createRace).toHaveBeenCalledWith('', {
        date: '2026-09-18',
        name: 'Tour Test',
        distanceMeters: 85000,
        discipline: 'road',
        priority: 'A',
      });
    });
    expect(refresh).toHaveBeenCalled();
    await waitFor(() => {
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    });
  });

  it('loads an existing race into the editor and updates it', async () => {
    vi.mocked(useRaces).mockReturnValue({
      races: [makeRace()],
      upcomingRaces: [makeRace()],
      completedRaces: [],
      isLoading: false,
      error: null,
      refresh: vi.fn(),
    });
    vi.mocked(updateRace).mockResolvedValue(makeRace({ name: 'Updated Attack' }));

    render(<RacesPageLayout apiBaseUrl="" />);

    fireEvent.click(screen.getAllByRole('button', { name: /edit race/i })[0]!);
    expect(screen.getByRole('dialog')).toBeInTheDocument();
    expect(screen.getByLabelText(/race name/i)).toHaveValue('Gravel Attack');

    fireEvent.change(screen.getByLabelText(/race name/i), { target: { value: 'Updated Attack' } });
    fireEvent.click(screen.getByRole('button', { name: /save race/i }));

    await waitFor(() => {
      expect(updateRace).toHaveBeenCalledWith('', 'race-1', {
        date: '2026-09-12',
        name: 'Updated Attack',
        distanceMeters: 120000,
        discipline: 'gravel',
        priority: 'A',
      });
    });
  });

  it('closes the race modal when cancel is clicked', () => {
    vi.mocked(useRaces).mockReturnValue({
      races: [],
      upcomingRaces: [],
      completedRaces: [],
      isLoading: false,
      error: null,
      refresh: vi.fn(),
    });

    render(<RacesPageLayout apiBaseUrl="" />);

    fireEvent.click(screen.getByRole('button', { name: /add race/i }));
    expect(screen.getByRole('dialog')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: /cancel/i }));

    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('keeps the race modal open while save is in flight', async () => {
    vi.mocked(useRaces).mockReturnValue({
      races: [],
      upcomingRaces: [],
      completedRaces: [],
      isLoading: false,
      error: null,
      refresh: vi.fn(),
    });
    vi.mocked(createRace).mockReturnValue(new Promise(() => {}));

    render(<RacesPageLayout apiBaseUrl="" />);

    fireEvent.click(screen.getByRole('button', { name: /add race/i }));
    fireEvent.change(screen.getByLabelText(/race name/i), { target: { value: 'Tour Test' } });
    fireEvent.change(screen.getByLabelText(/^date$/i), { target: { value: '2026-09-18' } });
    fireEvent.change(screen.getByLabelText(/distance \(km\)/i), { target: { value: '85' } });
    fireEvent.change(screen.getByLabelText(/discipline/i), { target: { value: 'road' } });
    fireEvent.click(screen.getByRole('button', { name: /cat\. a/i }));
    fireEvent.click(screen.getAllByRole('button', { name: /add race/i }).at(-1)!);

    expect(screen.getByRole('button', { name: /saving/i })).toBeDisabled();
    fireEvent.keyDown(window, { key: 'Escape' });
    fireEvent.click(screen.getByRole('dialog').parentElement as HTMLElement);

    expect(screen.getByRole('dialog')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /cancel/i })).toBeDisabled();
  });

  it('marks the selected priority button as pressed', () => {
    vi.mocked(useRaces).mockReturnValue({
      races: [],
      upcomingRaces: [],
      completedRaces: [],
      isLoading: false,
      error: null,
      refresh: vi.fn(),
    });

    render(<RacesPageLayout apiBaseUrl="" />);

    fireEvent.click(screen.getByRole('button', { name: /add race/i }));

    expect(screen.getByRole('button', { name: /cat\. b/i })).toHaveAttribute('aria-pressed', 'true');
    expect(screen.getByRole('button', { name: /cat\. a/i })).toHaveAttribute('aria-pressed', 'false');
  });
});
