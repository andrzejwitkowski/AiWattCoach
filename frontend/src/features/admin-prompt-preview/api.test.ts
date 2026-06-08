import { afterEach, describe, expect, it, vi } from 'vitest';

import { mockFetch } from '../../test/mockFetch';
import {
  loadAdminCalendarCoachPromptPreview,
  loadAdminMesoCyclePromptPreview,
  loadAdminPostWorkoutPromptPreview,
} from './api';

const originalFetch = global.fetch;

afterEach(() => {
  global.fetch = originalFetch;
  vi.restoreAllMocks();
});

describe('admin prompt preview api', () => {
  it('loads post-workout preview', async () => {
    mockFetch({
      meta: {
        userId: 'user-1',
        date: '2026-05-01',
        surface: 'post_workout',
        provider: 'openrouter',
        model: 'test-model',
        focusDate: '2026-05-01',
        selectedWorkoutId: 'ride-1',
        selectionMethod: 'single_workout',
      },
      request: {
        systemPrompt: 'system',
        stableContext: 'stable',
        volatileContext: 'volatile',
        conversation: [],
        tools: [],
        toolChoice: 'auto',
      },
      providerMessages: [{ role: 'system', content: 'system' }],
    });

    const result = await loadAdminPostWorkoutPromptPreview('', 'user-1', '2026-05-01');
    expect(result.meta.surface).toBe('post_workout');
    expect(result.meta.selectedWorkoutId).toBe('ride-1');
  });

  it('loads calendar coach preview', async () => {
    mockFetch({
      meta: {
        userId: 'user-1',
        date: '2026-05-01',
        surface: 'calendar_coach',
        provider: 'openrouter',
        model: 'test-model',
        focusDate: '2026-05-01',
      },
      request: {
        systemPrompt: 'system',
        stableContext: 'stable',
        volatileContext: 'volatile',
        conversation: [{ role: 'user', content: 'hello' }],
        tools: [],
        toolChoice: 'none',
      },
      providerMessages: [],
    });

    const result = await loadAdminCalendarCoachPromptPreview('', 'user-1', '2026-05-01');
    expect(result.meta.surface).toBe('calendar_coach');
  });

  it('loads meso cycle preview', async () => {
    mockFetch({
      meta: {
        userId: 'user-1',
        date: '2026-05-01',
        surface: 'meso_cycle_coach',
        provider: 'openrouter',
        model: 'test-model',
        focusDate: '2026-05-01',
        mesoStart: '2026-05-02',
        mesoEnd: '2026-05-31',
      },
      request: {
        systemPrompt: 'system',
        stableContext: 'stable',
        volatileContext: 'volatile',
        conversation: [{ role: 'user', content: 'generate' }],
        tools: [],
        toolChoice: 'auto',
      },
      providerMessages: [],
    });

    const result = await loadAdminMesoCyclePromptPreview('', 'user-1', '2026-05-01');
    expect(result.meta.surface).toBe('meso_cycle_coach');
    expect(result.meta.mesoStart).toBe('2026-05-02');
  });
});
