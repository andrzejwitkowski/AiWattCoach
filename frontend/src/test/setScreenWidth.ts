import { vi } from 'vitest';

export function setScreenWidth(width: number) {
  window.matchMedia = vi.fn().mockImplementation((query: string) => ({
    matches: query === '(max-width: 767px)' ? width <= 767 : false,
    media: query,
    onchange: null,
    addListener: () => undefined,
    removeListener: () => undefined,
    addEventListener: () => undefined,
    removeEventListener: () => undefined,
    dispatchEvent: () => false,
  })) as typeof window.matchMedia;
}
