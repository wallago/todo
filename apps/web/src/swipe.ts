import { useEffect, useRef } from "react";

/** How far a finger has to travel sideways to count as a swipe. */
const MIN_SWIPE_PX = 60;

/**
 * Calls `onSwipe(1)` on a swipe to the left (next page) and `onSwipe(-1)`
 * on a swipe to the right, unless `enabled` is false. Mostly-vertical moves
 * are scrolls and are ignored.
 */
export function useSwipe(onSwipe: (step: 1 | -1) => void, enabled: boolean) {
  // A ref, so a re-render mid-swipe doesn't drop the touch that started it.
  const callback = useRef(onSwipe);
  callback.current = onSwipe;
  useEffect(() => {
    if (!enabled) return;
    let start: { x: number; y: number } | null = null;
    const onStart = (event: TouchEvent) => {
      const touch = event.touches[0];
      start = event.touches.length === 1 && touch ? { x: touch.clientX, y: touch.clientY } : null;
    };
    const onEnd = (event: TouchEvent) => {
      const touch = event.changedTouches[0];
      if (!start || !touch) return;
      const dx = touch.clientX - start.x;
      const dy = touch.clientY - start.y;
      start = null;
      if (Math.abs(dx) >= MIN_SWIPE_PX && Math.abs(dx) > 2 * Math.abs(dy)) {
        callback.current(dx < 0 ? 1 : -1);
      }
    };
    document.addEventListener("touchstart", onStart, { passive: true });
    document.addEventListener("touchend", onEnd);
    return () => {
      document.removeEventListener("touchstart", onStart);
      document.removeEventListener("touchend", onEnd);
    };
  }, [enabled]);
}
