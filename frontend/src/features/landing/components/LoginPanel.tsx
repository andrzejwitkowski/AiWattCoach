type LoginPanelProps = {
  onLogin: () => void;
};

export function LoginPanel({ onLogin }: LoginPanelProps) {
  return (
    <aside className="rounded-[2rem] border border-lime-300/18 bg-slate-950/65 p-6 shadow-[0_25px_80px_rgba(8,15,23,0.55)] backdrop-blur">
      <p className="text-xs uppercase tracking-[0.35em] text-slate-400">bolt</p>
      <h2 className="mt-4 text-2xl font-semibold text-white">Get Started</h2>
      <p className="mt-3 text-sm leading-7 text-slate-300">
        Sign in with Google to enter the app and continue into your athlete or admin workflow.
      </p>

      <button
        className="mt-8 flex w-full items-center justify-center rounded-full bg-[linear-gradient(135deg,_#d2ff9a,_#98ff70)] px-5 py-3 text-sm font-semibold text-slate-950 transition hover:brightness-105"
        onClick={onLogin}
        type="button"
      >
        Sign in with Google
      </button>

      <div className="mt-6 flex items-center gap-3 text-xs uppercase tracking-[0.3em] text-slate-500">
        <div className="h-px flex-1 bg-white/10" />
        <span>or</span>
        <div className="h-px flex-1 bg-white/10" />
      </div>

      <div className="mt-6 rounded-[1.5rem] border border-white/10 bg-white/4 p-4">
        <p className="text-xs uppercase tracking-[0.25em] text-slate-400">Athlete ID</p>
        <div className="mt-3 rounded-full border border-white/10 bg-slate-950/60 px-4 py-3 text-sm text-slate-500">
          Continue after Google sign-in
        </div>
      </div>
    </aside>
  );
}
