import { Tag } from 'antd';
import {
  CheckCircleOutlined, ThunderboltOutlined, PoweroffOutlined,
} from '@ant-design/icons';
import type { BreakerStatus } from '../api/http';

interface BreakerBadgeProps {
  /** Breaker state, or `undefined`/`null` while unknown (e.g. fetch failed). */
  status?: BreakerStatus | null;
}

/** Global storm-breaker indicator for the dashboard header.
 * Renders a neutral placeholder when the state is unknown so the dashboard
 * never breaks on a failed `/api/breaker` call. */
export default function BreakerBadge({ status }: BreakerBadgeProps) {
  if (!status) {
    const unknown = 'Estado del storm breaker desconocido';
    return (
      <Tag
        color="default"
        role="status"
        aria-label={unknown}
        title={unknown}
        data-testid="breaker-badge-unknown"
      >
        Breaker —
      </Tag>
    );
  }

  if (!status.enabled) {
    const off = 'Storm breaker deshabilitado: las notificaciones no se suprimen';
    return (
      <Tag
        color="default"
        icon={<PoweroffOutlined />}
        role="status"
        aria-label={off}
        title={off}
        data-testid="breaker-badge-disabled"
      >
        Breaker: off
      </Tag>
    );
  }

  const open = status.state === 'open';
  const detail = `umbral ${status.threshold_percent}% en ${status.window_seconds}s (mín. ${status.min_monitors} monitores)`;

  if (open) {
    const label = 'Tormenta: notificaciones suprimidas';
    const title = `${label} — ${detail}`;
    return (
      <Tag
        color="error"
        icon={<ThunderboltOutlined />}
        role="status"
        aria-label={title}
        title={title}
        data-testid="breaker-badge-open"
      >
        {label}
      </Tag>
    );
  }

  const label = 'Breaker: sin tormenta';
  const title = `${label} — ${detail}`;
  return (
    <Tag
      color="success"
      icon={<CheckCircleOutlined />}
      role="status"
      aria-label={title}
      title={title}
      data-testid="breaker-badge-closed"
    >
      {label}
    </Tag>
  );
}
