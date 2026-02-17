import { useState, useEffect } from 'react';

// ---- Escape key hook (layered: only topmost modal closes) ----
let escapeLayerId = 0;
const escapeStack = [];
export function useEscapeKey(onClose) {
  useEffect(() => {
    const id = ++escapeLayerId;
    escapeStack.push(id);
    const handler = (e) => {
      if (e.key === 'Escape' && escapeStack[escapeStack.length - 1] === id) {
        e.stopImmediatePropagation();
        onClose();
      }
    };
    document.addEventListener('keydown', handler);
    return () => {
      document.removeEventListener('keydown', handler);
      const idx = escapeStack.indexOf(id);
      if (idx !== -1) escapeStack.splice(idx, 1);
    };
  }, [onClose]);
}

// ---- Responsive hook ----
export function useBreakpoint() {
  const [width, setWidth] = useState(window.innerWidth);
  useEffect(() => {
    const handler = () => setWidth(window.innerWidth);
    window.addEventListener('resize', handler);
    return () => window.removeEventListener('resize', handler);
  }, []);
  return { isMobile: width < 768, isCompact: width < 1024 };
}
