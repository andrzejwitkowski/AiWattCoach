import { LoginPanel } from '../features/landing/components/LoginPanel';
import { BackgroundGlow } from '../features/landing/components/BackgroundGlow';

type LandingPageProps = {
  onLogin: () => void;
};

/**
 * Wattly public landing page — centered glass-panel login card over a full-screen
 * cyclist background, with a fixed navbar and footer.
 */
export function LandingPage({ onLogin }: LandingPageProps) {
  return (
    <div className="relative min-h-screen overflow-y-auto bg-[#0c0e11] text-slate-100">
      <BackgroundGlow />

      <header className="fixed top-0 w-full z-50 px-6 py-4 md:px-12 bg-[#0c0e11]/80 backdrop-blur-2xl">
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
            className="bg-[#d2ff9a] text-[#3d6500] px-6 py-2 rounded font-['Manrope'] font-bold tracking-tight hover:opacity-80 transition-opacity"
            onClick={onLogin}
            type="button"
          >
            Get Started
          </button>
        </nav>
      </header>

      <main className="relative z-10 flex items-center justify-center min-h-screen px-4 pt-16">
        <LoginPanel onLogin={onLogin} />
      </main>

      <footer className="fixed bottom-0 w-full z-50 px-4 py-8">
        <div className="flex justify-center space-x-8 font-['Inter'] text-[10px] tracking-[0.05em] uppercase font-medium">
          <span className="text-slate-500">© 2024 WATTLY PERFORMANCE LABS</span>
          <a className="text-slate-500 hover:text-[#d2ff9a] transition-colors" href="#">Privacy Policy</a>
          <a className="text-slate-500 hover:text-[#d2ff9a] transition-colors" href="#">Terms of Service</a>
          <a className="text-slate-500 hover:text-[#d2ff9a] transition-colors" href="#">Cookie Settings</a>
        </div>
      </footer>
    </div>
  );
}
