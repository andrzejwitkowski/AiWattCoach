import { useState } from 'react';

import { logout } from '../api/auth';
import { useAuth } from '../context/AuthProvider';

type UserMenuProps = {
  apiBaseUrl: string;
  compact?: boolean;
};

export function UserMenu({ apiBaseUrl, compact = false }: UserMenuProps) {
  const auth = useAuth();
  const [isLoggingOut, setIsLoggingOut] = useState(false);
  const currentUser = auth.user;

  if (auth.status !== 'authenticated' || !currentUser) {
    return null;
  }

  return (
    <div className="flex items-center gap-3">
      {!compact ? (
        <div className="text-right">
          <p className="text-sm font-semibold text-white">{currentUser.displayName ?? currentUser.email}</p>
          <p className="text-xs uppercase tracking-[0.2em] text-slate-400">{currentUser.roles.join(' · ')}</p>
        </div>
      ) : null}
      <button
        className={[
          'rounded-full border border-white/15 bg-white/8 text-sm font-semibold text-slate-100 transition hover:bg-white/15',
          compact ? 'px-3 py-2' : 'px-4 py-2',
        ].join(' ')}
        disabled={isLoggingOut}
        onClick={async () => {
          setIsLoggingOut(true);
          try {
            await logout(apiBaseUrl);
            await auth.refreshAuth();
          } finally {
            setIsLoggingOut(false);
          }
        }}
        type="button"
      >
        {isLoggingOut ? 'Signing out...' : 'Sign out'}
      </button>
    </div>
  );
}
