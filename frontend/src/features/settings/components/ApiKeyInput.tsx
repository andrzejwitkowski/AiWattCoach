import { useState } from 'react';

type ApiKeyInputProps = {
  id: string;
  label: string;
  placeholder: string;
  isConfigured: boolean;
  configuredLabel?: string;
  value: string;
  onChange: (value: string) => void;
  accentColor?: 'cyan' | 'emerald';
};

export function ApiKeyInput({
  id,
  label,
  placeholder,
  isConfigured,
  configuredLabel = 'Configured',
  value,
  onChange,
  accentColor = 'cyan',
}: ApiKeyInputProps) {
  const [showKey, setShowKey] = useState(false);

  const accentStyles = {
    cyan: {
      border: 'focus:border-cyan-500/50',
      ring: 'focus:ring-cyan-500/30',
      text: 'text-cyan-400',
      bg: 'bg-cyan-500/15',
    },
    emerald: {
      border: 'focus:border-emerald-500/50',
      ring: 'focus:ring-emerald-500/30',
      text: 'text-emerald-400',
      bg: 'bg-emerald-500/15',
    },
  };

  const styles = accentStyles[accentColor];

  return (
    <div>
      <label
        htmlFor={id}
        className="mb-1.5 block text-xs font-medium uppercase tracking-wider text-slate-400"
      >
        {label}
      </label>
      <div className="relative">
        <input
          id={id}
          type={showKey ? 'text' : 'password'}
          autoComplete="off"
          className={`w-full rounded-xl border border-white/10 bg-white/5 px-4 py-2.5 pr-10 text-sm text-white placeholder-slate-500 focus:outline-none focus:ring-1 ${styles.border} ${styles.ring}`}
          placeholder={isConfigured ? '\u2022\u2022\u2022\u2022\u2022\u2022\u2022\u2022\u2022\u2022\u2022\u2022\u2022\u2022\u2022\u2022' : placeholder}
          value={value}
          onChange={(e) => onChange(e.target.value)}
        />
        <button
          type="button"
          className="absolute right-3 top-1/2 -translate-y-1/2 text-slate-400 hover:text-white"
          onClick={() => setShowKey(!showKey)}
          tabIndex={-1}
        >
          {showKey ? (
            <svg className="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M3.98 8.223A10.477 10.477 0 001.934 12C3.226 16.338 7.244 19.5 12 19.5c.993 0 1.953-.138 2.863-.395M6.228 6.228A10.45 10.45 0 0112 4.5c4.756 0 8.773 3.162 10.065 7.498a10.523 10.523 0 01-4.293 5.774M6.228 6.228L3 3m3.228 3.228l3.65 3.65m2.292-2.292l3.65 3.65m0 0a3 3 0 107.779 4.394A10.45 10.45 0 014.5 12c0-1.76.463-3.427 1.287-4.894m10.376 0a3 3 0 00-3.648-4.139m-3.648 0a3 3 0 00-3.648 4.139" />
            </svg>
          ) : (
            <svg className="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M2.036 12.322a1.012 1.012 0 010-.639C3.423 7.51 7.36 4.5 12 4.5c4.638 0 8.573 3.007 9.963 7.178.07.207.07.431 0 .639C20.577 16.49 16.64 19.5 12 19.5c-1.055 0-2.09-.213-3.092-.571m6.92-3.878a11.955 11.955 0 013.647-1.664m3.648 0a11.955 11.955 0 003.648-1.664M9.17 8.672a11.955 11.955 0 013.647-1.664m3.647 1.664a11.955 11.955 0 01-3.647-1.664M12 12a3 3 0 100-6 3 3 0 000 6z" />
            </svg>
          )}
        </button>
        {isConfigured && (
          <span className={`absolute right-10 top-1/2 -translate-y-1/2 text-xs ${styles.text}`}>
            {configuredLabel}
          </span>
        )}
      </div>
    </div>
  );
}
