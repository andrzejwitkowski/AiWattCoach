import { useEffect, type RefObject } from 'react';

const FOCUSABLE_SELECTOR = [
  'a[href]',
  'button:not([disabled])',
  'input:not([disabled]):not([type="hidden"])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  '[tabindex]:not([tabindex="-1"])',
].join(',');

type FocusableElement = HTMLElement & {
  disabled?: boolean;
};

export function useDialogFocusTrap(
  isOpen: boolean,
  dialogRef: RefObject<HTMLElement | null>,
  initialFocusRef?: RefObject<HTMLElement | null>,
) {
  useEffect(() => {
    if (!isOpen) {
      return undefined;
    }

    const dialog = dialogRef.current;
    if (!dialog) {
      return undefined;
    }

    const previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;

    const focusInitialElement = () => {
      const focusableElements = getFocusableElements(dialog);
      const initialFocus = initialFocusRef?.current ?? focusableElements[0] ?? dialog;
      initialFocus.focus();
    };

    focusInitialElement();

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key !== 'Tab') {
        return;
      }

      const focusableElements = getFocusableElements(dialog);
      if (focusableElements.length === 0) {
        event.preventDefault();
        dialog.focus();
        return;
      }

      const firstElement = focusableElements[0];
      const activeElement = document.activeElement;

      if (!dialog.contains(activeElement)) {
        event.preventDefault();
        firstElement.focus();
        return;
      }

      event.preventDefault();

      const currentIndex = focusableElements.findIndex((element) => element === activeElement);
      if (currentIndex === -1) {
        firstElement.focus();
        return;
      }

      const nextIndex = event.shiftKey
        ? (currentIndex === 0 ? focusableElements.length - 1 : currentIndex - 1)
        : (currentIndex === focusableElements.length - 1 ? 0 : currentIndex + 1);

      focusableElements[nextIndex]?.focus();
    };

    document.addEventListener('keydown', handleKeyDown);

    return () => {
      document.removeEventListener('keydown', handleKeyDown);
      previousFocus?.focus();
    };
  }, [dialogRef, initialFocusRef, isOpen]);
}

function getFocusableElements(container: HTMLElement): FocusableElement[] {
  return Array.from(container.querySelectorAll<FocusableElement>(FOCUSABLE_SELECTOR)).filter((element) => {
    if (element.hasAttribute('disabled')) {
      return false;
    }

    if (element.getAttribute('aria-hidden') === 'true') {
      return false;
    }

    return element.tabIndex >= 0;
  });
}
