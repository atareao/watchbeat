import { useState, useCallback } from 'react';

const THEME_KEY = 'watchbeat-theme';

export function useTheme() {
  const [isDark, setIsDark] = useState(() => {
    return localStorage.getItem(THEME_KEY) === 'dark';
  });

  const toggle = useCallback(() => {
    setIsDark(prev => {
      const next = !prev;
      localStorage.setItem(THEME_KEY, next ? 'dark' : 'light');
      return next;
    });
  }, []);

  return { isDark, toggle };
}