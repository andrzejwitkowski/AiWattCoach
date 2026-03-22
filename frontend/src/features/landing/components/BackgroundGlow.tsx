/**
 * Full-screen background layer: cyclist photo with gradient overlays.
 * Shows a dark fallback if the image fails to load.
 */
export function BackgroundGlow() {
  return (
    <div aria-hidden="true" className="fixed inset-0 z-0 bg-[#0c0e11]">
      <img
        alt=""
        role="presentation"
        className="w-full h-full object-cover"
        src="/images/cyclist-bg.jpg"
        style={{ clipPath: 'inset(0 0 5% 0)' }}
        onError={(e) => {
          const img = e.currentTarget;
          img.style.display = 'none';
        }}
      />
      <div className="absolute inset-0 bg-gradient-to-b from-[#0c0e11]/80 via-[#0c0e11]/40 to-[#0c0e11]" />
      <div className="absolute inset-0 bg-gradient-to-r from-[#0c0e11]/60 via-transparent to-[#0c0e11]/60" />
    </div>
  );
}
