import { renderHook, type RenderHookOptions, type RenderHookResult } from '@testing-library/react';

import { CoachSessionCacheProvider } from '../context/CoachSessionCache';

type HookCallback<TProps, TResult> = (props: TProps) => TResult;

export function renderCoachHook<TResult>(callback: () => TResult): RenderHookResult<TResult, undefined>;
export function renderCoachHook<TProps, TResult>(
  callback: HookCallback<TProps, TResult>,
  options: RenderHookOptions<TProps>,
): RenderHookResult<TResult, TProps>;
export function renderCoachHook<TProps, TResult>(
  callback: HookCallback<TProps, TResult>,
  options?: RenderHookOptions<TProps>,
): RenderHookResult<TResult, TProps> {
  return renderHook(callback, {
    wrapper: ({ children }) => <CoachSessionCacheProvider>{children}</CoachSessionCacheProvider>,
    ...options,
  });
}
