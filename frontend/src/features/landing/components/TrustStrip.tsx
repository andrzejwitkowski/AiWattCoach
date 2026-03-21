export function TrustStrip() {
  return (
    <div className="flex flex-wrap items-center gap-4 text-sm text-slate-400">
      <span>Minimal scopes</span>
      <span className="h-1 w-1 rounded-full bg-slate-500" />
      <span>Server-side sessions</span>
      <span className="h-1 w-1 rounded-full bg-slate-500" />
      <span>RBAC for admin tools</span>
    </div>
  );
}
