import { useEffect, useRef } from 'react';

export interface CheckEvent {
  type: 'check';
  monitor_id: string;
  monitor_name: string;
  status: 'up' | 'down' | 'error' | 'warning';
  response_time_ms: number;
  error_message: string | null;
  checked_at: string;
}

export interface ResyncEvent {
  type: 'resync';
}

export type SseEvent = CheckEvent | ResyncEvent;

function getToken(): string | null {
  try {
    return sessionStorage.getItem('watchbeat_token') || localStorage.getItem('watchbeat_token');
  } catch {
    return null;
  }
}

/**
 * Hook that connects to the backend SSE endpoint /api/events?token=<jwt>
 * and calls onEvent for each parsed 'check' event and onResync for each
 * 'resync' event.
 * Auto-reconnects on error/close with exponential backoff
 * (2000ms initial, doubling up to 30000ms max; resets on a valid message).
 * Cleans up on unmount.
 */
export function useSse(
  onEvent: (event: CheckEvent) => void,
  onResync?: () => void,
): void {
  const onEventRef = useRef(onEvent);
  onEventRef.current = onEvent;
  const onResyncRef = useRef(onResync);
  onResyncRef.current = onResync;

  useEffect(() => {
    let eventSource: EventSource | null = null;
    let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
    let destroyed = false;
    // Exponential backoff state.
    let attempt = 0;

    function connect() {
      if (destroyed) return;

      const token = getToken();
      if (!token) return;

      eventSource = new EventSource(`/api/events?token=${encodeURIComponent(token)}`);

      eventSource.onmessage = (msg) => {
        if (!msg.data) return;
        // A valid, parseable message means the connection is alive: reset backoff.
        let event: SseEvent;
        try {
          event = JSON.parse(msg.data) as SseEvent;
        } catch {
          // ignore malformed JSON
          return;
        }
        attempt = 0;
        if (event.type === 'check') {
          onEventRef.current(event);
        } else if (event.type === 'resync' && onResyncRef.current) {
          onResyncRef.current();
        }
      };

      eventSource.onerror = () => {
        if (eventSource) {
          eventSource.close();
          eventSource = null;
        }
        if (destroyed) return;
        const delay = Math.min(2000 * 2 ** attempt, 30000);
        attempt += 1;
        reconnectTimer = setTimeout(connect, delay);
      };
    }

    connect();

    return () => {
      destroyed = true;
      if (eventSource) {
        eventSource.close();
        eventSource = null;
      }
      if (reconnectTimer) {
        clearTimeout(reconnectTimer);
        reconnectTimer = null;
      }
    };
  }, []);
}