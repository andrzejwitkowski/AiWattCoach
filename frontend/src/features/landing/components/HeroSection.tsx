export function HeroSection() {
  return (
    <section className="space-y-6">
      <div className="space-y-3">
        <p className="text-sm font-semibold uppercase tracking-[0.45em] text-lime-300">WATTLY</p>
        <h1 className="max-w-xl font-serif text-5xl leading-tight text-white md:text-6xl">
          Welcome to Wattly
        </h1>
        <p className="max-w-xl text-lg leading-8 text-slate-300">
          Unlock your peak performance through precision training, verified identity, and a cleaner
          athlete workflow.
        </p>
      </div>

      <div className="flex flex-wrap gap-3 text-sm text-slate-300">
        <span className="rounded-full border border-white/10 bg-white/6 px-4 py-2">Performance</span>
        <span className="rounded-full border border-white/10 bg-white/6 px-4 py-2">Science</span>
        <span className="rounded-full border border-white/10 bg-white/6 px-4 py-2">Community</span>
      </div>
    </section>
  );
}
