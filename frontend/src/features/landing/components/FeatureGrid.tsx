const items = [
  {
    title: 'Verified Google identity',
    description: 'Use a trusted Google account so the application can confirm a real email address.'
  },
  {
    title: 'Role-aware access',
    description: 'Open the athlete experience by default and unlock admin diagnostics only where needed.'
  },
  {
    title: 'Future-ready coaching flows',
    description: 'This login foundation supports the next wave of sync, planning, and support tools.'
  }
];

export function FeatureGrid() {
  return (
    <section className="grid gap-4 md:grid-cols-3">
      {items.map((item) => (
        <article
          key={item.title}
          className="rounded-[1.8rem] border border-white/10 bg-slate-950/45 p-5 backdrop-blur"
        >
          <h2 className="text-lg font-semibold text-white">{item.title}</h2>
          <p className="mt-3 text-sm leading-7 text-slate-300">{item.description}</p>
        </article>
      ))}
    </section>
  );
}
