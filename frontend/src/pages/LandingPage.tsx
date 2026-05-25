import { LoginPanel } from '../features/landing/components/LoginPanel';
import { BackgroundGlow } from '../features/landing/components/BackgroundGlow';

type LandingPageProps = {
  onLogin: () => void;
  onJoinWhitelist: (email: string) => Promise<void>;
  authMessage?: string | null;
  whitelistMessage?: string | null;
  devAuthEnabled?: boolean;
};

/**
 * Wattly public landing page — centered glass-panel login card over a full-screen
 * cyclist background, with a fixed navbar and footer.
 */
export function LandingPage({
  onLogin,
  onJoinWhitelist,
  authMessage = null,
  whitelistMessage = null,
  devAuthEnabled = false
}: LandingPageProps) {
  return (
    <div className="relative min-h-dvh overflow-y-auto bg-[#0c0e11] text-slate-100">
      <BackgroundGlow />

      <header className="fixed top-0 z-50 w-full bg-[#0c0e11]/80 px-4 py-4 backdrop-blur-2xl md:px-12">
        <nav className="flex justify-between items-center w-full">
          <div className="text-2xl font-black tracking-tighter text-[#d2ff9a] uppercase italic font-['Manrope']">
            WATTLY
          </div>
          <div className="hidden md:flex items-center space-x-8 font-['Manrope'] font-bold tracking-tight">
            <a className="text-slate-400 hover:text-white transition-colors" href="#">Performance</a>
            <a className="text-slate-400 hover:text-white transition-colors" href="#">Science</a>
            <a className="text-slate-400 hover:text-white transition-colors" href="#">Community</a>
          </div>
          <button
            className="rounded bg-[#d2ff9a] px-4 py-2 font-['Manrope'] font-bold tracking-tight text-[#3d6500] transition-opacity hover:opacity-80 md:px-6"
            onClick={onLogin}
            type="button"
          >
            Get Started
          </button>
        </nav>
      </header>

      <main className="relative z-10 flex min-h-dvh items-center justify-center px-4 pb-8 pt-24 md:min-h-screen md:pb-24 md:pt-16">
        <LoginPanel
          onLogin={onLogin}
          onJoinWhitelist={onJoinWhitelist}
          authMessage={authMessage}
          whitelistMessage={whitelistMessage}
          devAuthEnabled={devAuthEnabled}
        />
      </main>

      <footer className="relative z-10 px-4 py-6 md:fixed md:bottom-0 md:w-full md:py-8">
        <div className="flex flex-wrap justify-center gap-4 font-['Inter'] text-[10px] font-medium uppercase tracking-[0.05em] md:space-x-8 md:gap-0">
          <span className="text-slate-500">© {new Date().getFullYear()} WATTLY PERFORMANCE LABS</span>
          <a className="text-slate-500 hover:text-[#d2ff9a] transition-colors" href="#">Privacy Policy</a>
          <a className="text-slate-500 hover:text-[#d2ff9a] transition-colors" href="#">Terms of Service</a>
          <a className="text-slate-500 hover:text-[#d2ff9a] transition-colors" href="#">Cookie Settings</a>
        </div>
      </footer>
    </div>
  );
}
