import type { ReactNode, Ref } from 'react';

type PlannedRestDayFormDialogProps = {
  title: string;
  dialogRef: Ref<HTMLElement>;
  onBackdropClick: () => void;
  children: ReactNode;
};

export function PlannedRestDayFormDialog({
  title,
  dialogRef,
  onBackdropClick,
  children,
}: PlannedRestDayFormDialogProps) {
  return (
    <div
      className="fixed inset-0 z-50 flex items-end justify-center bg-black/70 p-4 backdrop-blur-sm md:items-center"
      onClick={onBackdropClick}
    >
      <section
        ref={dialogRef}
        aria-modal="true"
        role="dialog"
        aria-labelledby="planned-rest-day-form-title"
        tabIndex={-1}
        className="w-full max-w-xl rounded-[1.8rem] border border-white/10 bg-[#10131b] p-6 shadow-[0_30px_90px_rgba(0,0,0,0.55)]"
        onClick={(event) => event.stopPropagation()}
      >
        <h2 id="planned-rest-day-form-title" className="text-2xl font-black text-white">
          {title}
        </h2>
        {children}
      </section>
    </div>
  );
}
