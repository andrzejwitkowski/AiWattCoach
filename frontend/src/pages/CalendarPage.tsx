export function CalendarPage() {
  return (
    <div className="rounded-2xl border border-white/10 bg-white/5 p-8 text-center">
      <div className="mb-4 flex justify-center">
        <svg className="h-16 w-16 text-slate-500" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1}>
          <path strokeLinecap="round" strokeLinejoin="round" d="M6.75 3v2.25M17.25 3v2.25M3 18.75V7.5a2.25 2.25 0 012.25-2.25h13.5A2.25 2.25 0 0121 7.5v11.25m-18 0A2.25 2.25 0 005.25 21h13.5A2.25 2.25 0 0021 18.75m-18 0v-7.5A2.25 2.25 0 015.25 9h13.5A2.25 2.25 0 0121 11.25v7.5" />
        </svg>
      </div>
      <h2 className="mb-2 font-serif text-2xl text-white">Calendar</h2>
      <p className="text-slate-400">Training calendar is coming soon.</p>
    </div>
  );
}
