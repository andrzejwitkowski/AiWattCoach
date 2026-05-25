import { renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { useMediaQuery } from './useMediaQuery';

const originalMatchMedia = window.matchMedia;

afterEach(() => {
  Object.defineProperty(window, 'matchMedia', {
    writable: true,
    value: originalMatchMedia,
  });
  vi.restoreAllMocks();
});

describe('useMediaQuery', () => {
  it('falls back to legacy matchMedia listeners when addEventListener is unavailable', () => {
    const addListener = vi.fn();
    const removeListener = vi.fn();
    const mediaQueryList = {
      matches: true,
      media: '(max-width: 767px)',
      onchange: null,
      addListener,
      removeListener,
      addEventListener: undefined,
      removeEventListener: undefined,
      dispatchEvent: () => false,
    } as unknown as MediaQueryList;

    Object.defineProperty(window, 'matchMedia', {
      writable: true,
      value: vi.fn().mockReturnValue(mediaQueryList),
    });

    const { result, unmount } = renderHook(() => useMediaQuery('(max-width: 767px)'));

    expect(result.current).toBe(true);
    expect(addListener).toHaveBeenCalledWith(expect.any(Function));

    const listener = addListener.mock.calls[0]?.[0];
    expect(listener).toBeTypeOf('function');

    unmount();

    expect(removeListener).toHaveBeenCalledWith(listener);
  });
});
