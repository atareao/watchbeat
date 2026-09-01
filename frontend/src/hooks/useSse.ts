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

export type SseEvent = CheckEvent;

function getToken(): string | null {
  try {
    return sessionStorage.getItem('watchbeat_token') || localStorage.getItem('watchbeat_token');
  } catch {
    return null;
  }
}

/**
 * Hook that connects to the backend SSE endpoint /api/events?token=<jwt>
 * and calls onEvent for each parsed event.
 * Auto-reconnects on error/close after 5s delay.
 * Cleans up on unmount.
 */
export function useSse(onEvent: (event: SseEvent) => void): void {
  const onEventRef = useRef(onEvent);
  onEventRef.current = onEvent;

  useEffect(() => {
    let eventSource: EventSource | null = null;
    let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
    let destroyed = false;

    function connect() {
      if (destroyed) return;

      const token = getToken();
      if (!token) return;

      eventSource = new EventSource(`/api/events?token=${encodeURIComponent(token)}`);

      eventSource.onmessage = (msg) => {
        if (!msg.data) return;
        try {
          const event = JSON.parse(msg.data) as SseEvent;
          onEventRef.current(event);
        } catch {
          // ignore malformed JSON
        }
      };

      eventSource.onerror = () => {
        if (eventSource) {
          eventSource.close();
          eventSource = null;
        }
        if (!destroyed) {
          reconnectTimer = setTimeout(connect, 5000);
        }
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