import { FeatureGrid } from '../features/landing/components/FeatureGrid';
import { HeroSection } from '../features/landing/components/HeroSection';
import { LoginPanel } from '../features/landing/components/LoginPanel';
import { TrustStrip } from '../features/landing/components/TrustStrip';
import { BackgroundGlow } from '../features/landing/components/BackgroundGlow';

type LandingPageProps = {
  onLogin: () => void;
};

export function LandingPage({ onLogin }: LandingPageProps) {
  return (
    <div className="relative min-h-screen overflow-hidden bg-[linear-gradient(180deg,_#090b0f_0%,_#0f1418_45%,_#111827_100%)] text-slate-100">
      <BackgroundGlow />

      <div className="relative mx-auto flex min-h-screen w-full max-w-7xl flex-col gap-10 px-5 py-8 sm:px-8 lg:px-10">
        <header className="flex items-center justify-between">
          <div>
            <p className="text-sm font-semibold uppercase tracking-[0.5em] text-lime-300">WATTLY</p>
            <p className="mt-2 text-sm text-slate-400">Performance Lab</p>
          </div>
        </header>

        <main className="grid flex-1 items-center gap-8 lg:grid-cols-[minmax(0,1.2fr)_26rem]">
          <div className="space-y-8">
            <HeroSection />
            <TrustStrip />
            <FeatureGrid />
          </div>

          <LoginPanel onLogin={onLogin} />
        </main>
      </div>
    </div>
  );
}
