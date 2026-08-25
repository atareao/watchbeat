import { useCallback, useEffect, useRef } from 'react';
import { getToken } from '../store/auth';

type SseEvent = {
  type: string;
  [key: string]: unknown;
};

type Listener = (event: SseEvent) => void;

const listeners = new Set<Listener>();

let eventSource: EventSource | null = null;

function connect() {
  if (eventSource?.readyState === EventSource.OPEN || eventSource?.readyState === EventSource.CONNECTING) return;

  const t = getToken();
  if (!t) return;

  const url = `/api/events?token=${encodeURIComponent(t)}`;
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