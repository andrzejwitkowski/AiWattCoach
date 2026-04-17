import { useState } from 'react';

type LoginPanelProps = {
  onLogin: () => void;
  onJoinWhitelist: (email: string) => Promise<void>;
  devAuthEnabled?: boolean;
  authMessage?: string | null;
  whitelistMessage?: string | null;
};

/**
 * Centered glass-panel login card for the Wattly landing page.
 * Offers Google OAuth sign-in and a waitlist request flow.
 */
export function LoginPanel({
  onLogin,
  onJoinWhitelist,
  devAuthEnabled = false,
  authMessage = null,
  whitelistMessage = null
}: LoginPanelProps) {
  const [email, setEmail] = useState('');
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  async function handleJoinWhitelist() {
    const trimmedEmail = email.trim();
    if (!trimmedEmail) {
      return;
    }

    setIsSubmitting(true);
    setErrorMessage(null);

    try {
      await onJoinWhitelist(trimmedEmail);
      setEmail('');
    } catch {
      setErrorMessage('Failed to join the whitelist. Please try again.');
    } finally {
      setIsSubmitting(false);
    }
  }

  return (
    <div className="glass-panel w-full max-w-md p-10 rounded-xl shadow-2xl border border-white/5 flex flex-col items-center text-center">
      <div className="mb-8">
        <div className="inline-flex items-center justify-center w-16 h-16 rounded-full bg-[#d2ff9a]/10 mb-6">
          <svg className="w-8 h-8 text-[#d2ff9a]" viewBox="0 0 24 24" fill="currentColor">
            <path d="M11 21h-1l1-7H7.5c-.58 0-.57-.32-.38-.66.19-.34.05-.08.07-.12C8.48 10.94 10.42 7.54 13 3h1l-1 7h3.5c.49 0 .56.33.47.51l-.07.15C12.96 17.55 11 21 11 21z"/>
          </svg>
        </div>
        <h1 className="text-4xl font-['Manrope'] font-extrabold tracking-tighter text-[#f9f9fd] mb-3">
          Welcome to <span className="text-neon">Wattly</span>
        </h1>
        <p className="text-[#aaabaf] font-['Inter'] text-sm leading-relaxed max-w-[280px] mx-auto">
          Unlock your peak performance through precision training.
        </p>
      </div>

      <div className="w-full space-y-4">
        {devAuthEnabled ? (
          <div className="rounded-lg border border-[#d2ff9a]/20 bg-[#d2ff9a]/8 px-4 py-3 text-left" role="note">
            <p className="text-[10px] font-['Inter'] uppercase tracking-[0.24em] text-[#d2ff9a]">Dev auth enabled</p>
            <p className="mt-2 text-sm font-['Inter'] text-[#d7dbc9]">
              Google sign-in uses the configured mock athlete, so you can enter the app without a live OAuth redirect.
            </p>
          </div>
        ) : null}

        {authMessage ? (
          <div className="rounded-lg border border-amber-400/20 bg-amber-400/10 px-4 py-3 text-left" role="status">
            <p className="text-sm font-['Inter'] text-amber-100">{authMessage}</p>
          </div>
        ) : null}

        {whitelistMessage ? (
          <div className="rounded-lg border border-emerald-400/20 bg-emerald-400/10 px-4 py-3 text-left" role="status">
            <p className="text-sm font-['Inter'] text-emerald-100">{whitelistMessage}</p>
          </div>
        ) : null}

        {errorMessage ? (
          <div className="rounded-lg border border-rose-400/20 bg-rose-400/10 px-4 py-3 text-left" role="alert">
            <p className="text-sm font-['Inter'] text-rose-100">{errorMessage}</p>
          </div>
        ) : null}

        <button
          className="w-full flex items-center justify-center gap-3 bg-[#23262a] border border-[#46484b]/30 py-4 px-6 rounded-lg text-[#f9f9fd] font-['Inter'] font-semibold hover:bg-[#292c31] transition-all duration-300"
          onClick={onLogin}
          type="button"
        >
          <svg className="w-5 h-5" viewBox="0 0 24 24">
            <path d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92c-.26 1.37-1.04 2.53-2.21 3.31v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.09z" fill="#4285F4" />
            <path d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z" fill="#34A853" />
            <path d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62z" fill="#FBBC05" />
            <path d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z" fill="#EA4335" />
          </svg>
          <span>Sign in with Google</span>
        </button>

        <div className="relative flex items-center py-4">
          <div className="flex-grow border-t border-[#46484b]/20" />
          <span className="flex-shrink mx-4 text-[10px] font-['Inter'] uppercase tracking-widest text-[#747579]">or</span>
          <div className="flex-grow border-t border-[#46484b]/20" />
        </div>

        <div className="space-y-3">
          <div className="text-left">
            <label className="block text-[10px] font-['Inter'] uppercase tracking-widest text-[#747579] mb-1 ml-1" htmlFor="waitlist-email">
              Email
            </label>
            <input
              className="w-full bg-[#111417] border-none rounded-lg focus:ring-1 focus:ring-[#d2ff9a]/50 text-[#f9f9fd] placeholder:text-[#747579]/50 py-3 px-4 font-['Inter']"
              id="waitlist-email"
              placeholder="you@example.com"
              type="email"
              value={email}
              onChange={(event) => setEmail(event.target.value)}
            />
          </div>
          <button
            className="w-full bg-[#292c31] text-[#d2ff9a] border border-[#d2ff9a]/20 py-3 px-6 rounded-lg font-['Inter'] font-bold hover:bg-[#d2ff9a] hover:text-[#3d6500] transition-all duration-300 disabled:opacity-40"
            type="button"
            onClick={() => {
              void handleJoinWhitelist();
            }}
            disabled={!email.trim() || isSubmitting}
          >
            {isSubmitting ? 'Sending...' : 'Join whitelist'}
          </button>
        </div>
      </div>

      <div className="mt-8">
        <p className="text-[10px] font-['Inter'] text-[#747579] uppercase tracking-wider">
          Google sign-in is enabled after manual approval.
        </p>
      </div>
    </div>
  );
}
