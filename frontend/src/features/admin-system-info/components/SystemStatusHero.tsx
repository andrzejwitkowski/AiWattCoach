export function SystemStatusHero() {
  return (
    <div className="rounded-[2rem] border border-white/15 bg-slate-950/70 p-8 shadow-[0_30px_80px_rgba(15,23,42,0.45)] backdrop-blur">
      <p className="text-sm font-semibold uppercase tracking-[0.35em] text-cyan-300">System Info</p>
      <h1 className="mt-4 max-w-xl font-serif text-4xl leading-tight text-white md:text-5xl">
        Operational diagnostics for the admin workspace.
      </h1>
      <p className="mt-5 max-w-2xl text-base leading-7 text-slate-300 md:text-lg">
        This moved from the old start page so administrators can keep backend visibility without
        exposing operational details on the public landing page.
      </p>
    </div>
  );
}
