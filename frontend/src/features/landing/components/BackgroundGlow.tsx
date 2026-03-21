export function BackgroundGlow() {
  return (
    <div aria-hidden="true" className="fixed inset-0 z-0">
      <img
        alt="Professional cyclist in aero gear riding towards the camera on a moody coastal road"
        className="w-full h-full object-cover"
        src="/images/cyclist-bg.jpg"
        style={{ clipPath: 'inset(0 0 5% 0)' }}
      />
      <div className="absolute inset-0 bg-gradient-to-b from-[#0c0e11]/80 via-[#0c0e11]/40 to-[#0c0e11]" />
      <div className="absolute inset-0 bg-gradient-to-r from-[#0c0e11]/60 via-transparent to-[#0c0e11]/60" />
    </div>
  );
}
