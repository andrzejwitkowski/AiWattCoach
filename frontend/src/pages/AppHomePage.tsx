export function AppHomePage() {
  return (
    <section className="rounded-[2rem] border border-white/15 bg-slate-950/70 p-8 shadow-[0_30px_80px_rgba(15,23,42,0.45)] backdrop-blur">
      <p className="text-sm font-semibold uppercase tracking-[0.35em] text-cyan-300">Athlete workspace</p>
      <h1 className="mt-4 max-w-xl font-serif text-4xl leading-tight text-white md:text-5xl">
        Signed in and ready for the next training workflow.
      </h1>
      <p className="mt-5 max-w-2xl text-base leading-7 text-slate-300 md:text-lg">
        This is the post-login home area for normal users. Future training, sync, and coaching
        flows should branch from here.
      </p>
    </section>
  );
}
