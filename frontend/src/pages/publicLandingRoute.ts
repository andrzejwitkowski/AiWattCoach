import type { Location, NavigateFunction } from 'react-router-dom';

export const WHITELIST_REQUESTED_MESSAGE =
  'Requested whitelist access. We will reach out after approval.';
export const PENDING_APPROVAL_MESSAGE =
  'Your account is pending approval. Join the whitelist or wait for approval before signing in.';

export function resolvePublicLandingReturnTo(location: Location): string {
  const searchParams = new URLSearchParams(location.search);
  const searchReturnTo = searchParams.get('returnTo');
  const stateValue = (location.state as { from?: unknown } | null)?.from;
  const stateReturnTo = typeof stateValue === 'string' && stateValue.length > 0 ? stateValue : null;

  if (typeof searchReturnTo === 'string' && searchReturnTo.length > 0) {
    return searchReturnTo;
  }

  return stateReturnTo || '/calendar';
}

export function resolvePublicLandingMessages(search: string) {
  const searchParams = new URLSearchParams(search);
  const whitelistStatus = searchParams.get('whitelist');
  const authStatus = searchParams.get('auth');

  return {
    whitelistMessage: whitelistStatus === 'requested' ? WHITELIST_REQUESTED_MESSAGE : null,
    authMessage: authStatus === 'pending-approval' ? PENDING_APPROVAL_MESSAGE : null,
  };
}

export function navigateToWhitelistRequested(
  location: Location,
  navigate: NavigateFunction,
) {
  const params = new URLSearchParams(location.search);
  params.delete('auth');
  params.set('whitelist', 'requested');
  void navigate(
    {
      pathname: location.pathname,
      search: `?${params.toString()}`,
    },
    {
      replace: true,
      state: location.state,
    },
  );
}
