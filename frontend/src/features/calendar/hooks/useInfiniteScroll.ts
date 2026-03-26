import { useEffect, useRef } from 'react';
import type { RefObject } from 'react';

type UseInfiniteScrollOptions = {
  rootRef: RefObject<HTMLElement | null>;
  onReachTop: () => void;
  onReachBottom: () => void;
  disabled?: boolean;
};

type UseInfiniteScrollResult = {
  topSentinelRef: RefObject<HTMLDivElement | null>;
  bottomSentinelRef: RefObject<HTMLDivElement | null>;
};

export function useInfiniteScroll({
  rootRef,
  onReachTop,
  onReachBottom,
  disabled = false,
}: UseInfiniteScrollOptions): UseInfiniteScrollResult {
  const topSentinelRef = useRef<HTMLDivElement>(null);
  const bottomSentinelRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (disabled) {
      return;
    }

    const root = rootRef.current;
    const topSentinel = topSentinelRef.current;
    const bottomSentinel = bottomSentinelRef.current;

    if (!root || !topSentinel || !bottomSentinel || typeof IntersectionObserver === 'undefined') {
      return;
    }

    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (!entry.isIntersecting) {
            continue;
          }

          if (entry.target === topSentinel) {
            onReachTop();
          }

          if (entry.target === bottomSentinel) {
            onReachBottom();
          }
        }
      },
      {
        root,
        rootMargin: '200px 0px',
        threshold: 0.01,
      },
    );

    observer.observe(topSentinel);
    observer.observe(bottomSentinel);

    return () => {
      observer.disconnect();
    };
  }, [disabled, onReachBottom, onReachTop, rootRef]);

  return {
    topSentinelRef,
    bottomSentinelRef,
  };
}
