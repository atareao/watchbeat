import { useCallback, useEffect, useRef, useSyncExternalStore } from 'react';

type SseEvent = {
  type: string;
  [key: string]: unknown;
};

type Listener = (event: SseEvent) => void;

const listeners = new Set<Listener>();

let eventSource: EventSource | null = null;
let token: string | null = null;

function getCookie(name: string): string | null {
  const match = document.cookie.match(`(?:^|; )${name}=([^;]*)`);
  return match ? decodeURIComponent(match[1]) : null;
}

function connect() {
  if (eventSource?.readyState === EventSource.OPEN || eventSource?.readyState === EventSource.CONNECTING) return;

  const t = token || getCookie('token');
  if (!t) return;

  const url = t ? `/api/events?token=${encodeURIComponent(t)}` : '/api/events';
  eventSource = new EventSource(url);

  eventSource.onmessage = (e) => {
    try {
      const data = JSON.parse(e.data) as SseEvent;
      listeners.forEach(fn => fn(data));
    } catch { /* ignore malformed */ }
  };

  eventSource.onerror = () => {
    eventSource?.close();
    eventSource = null;
    setTimeout(connect, 5000);
  };
}

function disconnect() {
  eventSource?.close();
  eventSource = null;
}

export function setSseToken(t: string | null) {
  token = t;
  if (t) connect();
  else disconnect();
}

export function useSse(): { lastEvent: SseEvent | null; subscribe: (fn: Listener) => () => void } {
  const lastEventRef = useRef<SseEvent | null>(null);

  const subscribe = useCallback((fn: Listener) => {
    listeners.add(fn);
    return () => { listeners.delete(fn); };
  }, []);

  useEffect(() => {
    connect();
    return () => disconnect();
  }, []);

  return {
    lastEvent: lastEventRef.current,
    subscribe,
  };
}