import { useEffect, useState, useCallback } from 'react';
import { useParams } from 'react-router';
import { Card, Typography, Spin, Table, Tag, Button, Descriptions, Space, Tooltip, message, Statistic, Row, Col } from 'antd';
import {
  ReloadOutlined, PlayCircleOutlined, ClockCircleOutlined, BarChartOutlined,
  HeartOutlined, CopyOutlined, CheckCircleOutlined, CloseCircleOutlined,
} from '@ant-design/icons';
import { fetchMonitor, fetchChecks, fetchTimelineBuckets, runCheck, type Monitor, type CheckResult, type TimelineBucket } from '../api/http';
import dayjs from 'dayjs';
import relativeTime from 'dayjs/plugin/relativeTime';
import 'dayjs/locale/es';

dayjs.extend(relativeTime);
dayjs.locale('es');

const { Title, Text } = Typography;

const STATUS_TAG: Record<string, { color: string; text: string }> = {
  up: { color: 'green', text: 'UP' },
  down: { color: 'red', text: 'DOWN' },
  error: { color: 'orange', text: 'ERROR' },
};

// ── Range options with bucket sizes ──
interface RangeOption {
  label: string;
  labelLong: string;
  hours?: number;
  days?: number;
  bucketSeconds: number;
}

const RANGE_OPTIONS: RangeOption[] = [
  { label: '1h', labelLong: 'Última hora', hours: 1, bucketSeconds: 60 },
  { label: '6h', labelLong: 'Últimas 6 horas', hours: 6, bucketSeconds: 300 },
  { label: '12h', labelLong: 'Últimas 12 horas', hours: 12, bucketSeconds: 600 },
  { label: '24h', labelLong: 'Último día', hours: 24, bucketSeconds: 900 },
  { label: '7d', labelLong: 'Últimos 7 días', days: 7, bucketSeconds: 7200 },
  { label: '15d', labelLong: 'Últimos 15 días', days: 15, bucketSeconds: 14400 },
  { label: '30d', labelLong: 'Último mes', days: 30, bucketSeconds: 28800 },
  { label: '3m', labelLong: 'Últimos 3 meses', days: 90, bucketSeconds: 86400 },
  { label: '6m', labelLong: 'Últimos 6 meses', days: 180, bucketSeconds: 172800 },
];

// ── Helper: get health color for a bucket ──
function healthColor(upPct: number, status: string): string {
  if (status === 'no_data') return '#6b7280';
  if (upPct >= 99) return '#22c55e';
  if (upPct >= 95) return '#4ade80';
  if (upPct >= 90) return '#86efac';
  if (upPct >= 75) return '#facc15';
  if (upPct >= 50) return '#fb923c';
  return '#ef4444';
}

// ── Helper: format bucket time label ──
function formatBucketTime(bucketStart: string, bucketSeconds: number): string {
  const d = dayjs(bucketStart);
  if (bucketSeconds >= 86400) return d.format('DD/MM');
  if (bucketSeconds >= 3600) return d.format('DD/MM HH:mm');
  return d.format('HH:mm');
}

// ── Heartbeat status logic ──
function heartbeatStatus(monitor: Monitor): { label: string; color: string; icon: React.ReactNode } {
  if (!monitor.last_seen_at) {
    return { label: 'Pendiente', color: 'orange', icon: <ClockCircleOutlined style={{ color: '#f59e0b', fontSize: 24 }} /> };
  }
  const elapsed = Date.now() - new Date(monitor.last_seen_at).getTime();
  const grace = (monitor.grace_seconds ?? 3600) * 1000;
  if (elapsed < grace) {
    return { label: 'OK', color: '#22c55e', icon: <CheckCircleOutlined style={{ color: '#22c55e', fontSize: 24 }} /> };
  }
  return { label: 'Perdido', color: '#ef4444', icon: <CloseCircleOutlined style={{ color: '#ef4444', fontSize: 24 }} /> };
}

// ── Heartbeat view ──
function HeartbeatView({ monitor, checks, loading, loadChecks, checksPerPage }: {
  monitor: Monitor;
  checks: CheckResult[];
  loading: boolean;
  loadChecks: (p: number, pp: number) => void;
  checksPerPage: number;
}) {
  const status = heartbeatStatus(monitor);
  const pulseUrl = `${window.location.origin}/api/heartbeat/${monitor.token}`;

  const checksColumns = [
    { title: 'Estado', dataIndex: 'status', key: 'status', width: 80,
      render: (s: string) => <Tag color={s === 'up' ? 'green' : 'red'}>{s === 'up' ? 'OK' : 'DOWN'}</Tag>,
    },
    { title: 'Fecha', dataIndex: 'checked_at', key: 'date',
      render: (v: string) => dayjs(v).format('DD/MM/YYYY HH:mm:ss'),
    },
  ];

  return (
    <div>
      {/* Header */}
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <Space>
          <HeartOutlined style={{ color: '#ec4899', fontSize: 28 }} />
          <Title level={3} style={{ margin: 0 }}>{monitor.name}</Title>
        </Space>
        <Button icon={<ReloadOutlined />} onClick={() => loadChecks(1, checksPerPage)}>Recargar</Button>
      </div>

      {/* Status card */}
      <Card style={{ marginTop: 16, borderLeft: `4px solid ${status.color}` }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 16 }}>
          {status.icon}
          <div>
            <Text strong style={{ fontSize: 18, color: status.color }}>{status.label}</Text>
            <br />
            <Text type="secondary">
              {monitor.last_seen_at
                ? `Último pulso: ${dayjs(monitor.last_seen_at).fromNow()}`
                : 'Sin pulsos recibidos'}
            </Text>
          </div>
        </div>
      </Card>

      {/* Details */}
      <Descriptions column={2} style={{ marginTop: 16 }} bordered size="small">
        <Descriptions.Item label="Tipo">Heartbeat</Descriptions.Item>
        <Descriptions.Item label="Grace period">{monitor.grace_seconds ?? 3600}s</Descriptions.Item>
        <Descriptions.Item label="Token" span={2}>
          <Space>
            <Text code style={{ fontSize: 12 }}>{monitor.token}</Text>
            <Button size="small" icon={<CopyOutlined />} onClick={() => {
              navigator.clipboard.writeText(monitor.token ?? '');
              message.success('Token copiado');
            }} />
          </Space>
        </Descriptions.Item>
        <Descriptions.Item label="URL de pulso" span={2}>
          <Space>
            <Text code style={{ fontSize: 12 }}>{pulseUrl}</Text>
            <Button size="small" icon={<CopyOutlined />} onClick={() => {
              navigator.clipboard.writeText(pulseUrl);
              message.success('URL copiada');
            }} />
          </Space>
        </Descriptions.Item>
      </Descriptions>

      {/* Pulse history */}
      <Card title="Histórico de pulsos" style={{ marginTop: 16 }}>
        <Table
          dataSource={checks}
          columns={checksColumns}
          rowKey="id"
          loading={loading}
          pagination={{
            pageSize: checksPerPage,
            showSizeChanger: true,
            pageSizeOptions: ['10', '20', '50', '100'],
            onChange: (p, ps) => loadChecks(p, ps),
          }}
          size="small"
        />
      </Card>
    </div>
  );
}

// ── Monitor Stats Card (latency, uptime 24h/30d/1y, cert expiry) ──
function MonitorStats({ monitor, latestCheck }: { monitor: Monitor; latestCheck: CheckResult | null }) {
  const [stats, setStats] = useState<{
    latency24h: number | null;
    uptime24h: number | null;
    uptime30d: number | null;
    uptime1y: number | null;
  }>({ latency24h: null, uptime24h: null, uptime30d: null, uptime1y: null });
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    if (monitor.type === 'heartbeat') return;
    setLoading(true);

    // Fetch 24h, 30d, and 1y buckets in parallel
    Promise.all([
      fetchTimelineBuckets(monitor.id, { hours: 24, bucket_seconds: 900 }),
      fetchTimelineBuckets(monitor.id, { days: 30, bucket_seconds: 28800 }),
      fetchTimelineBuckets(monitor.id, { days: 365, bucket_seconds: 86400 }),
    ]).then(([b24h, b30d, b1y]) => {
      const calcUptime = (buckets: TimelineBucket[]) => {
        const withData = buckets.filter(b => b.dominant_status !== 'no_data');
        if (withData.length === 0) return null;
        return Math.round(withData.reduce((sum, b) => sum + b.up_pct, 0) / withData.length * 100) / 100;
      };
      const calcLatency = (buckets: TimelineBucket[]) => {
        const withData = buckets.filter(b => b.dominant_status !== 'no_data');
        if (withData.length === 0) return null;
        return Math.round(withData.reduce((sum, b) => sum + b.avg_response_time_ms, 0) / withData.length);
      };
      setStats({
        latency24h: calcLatency(b24h.buckets),
        uptime24h: calcUptime(b24h.buckets),
        uptime30d: calcUptime(b30d.buckets),
        uptime1y: calcUptime(b1y.buckets),
      });
    }).catch(() => {
      // ignore
    }).finally(() => setLoading(false));
  }, [monitor.id, monitor.type]);

  const certExpiry = latestCheck?.tls_cert_expires_at ?? null;
  const certDaysLeft = latestCheck?.tls_cert_days_left ?? null;

  return (
    <Card title="Estadísticas" style={{ marginTop: 16 }} loading={loading}>
      <Row gutter={[16, 16]}>
        <Col xs={12} sm={8} md={4}>
          <Statistic
            title="Latencia 24h"
            value={stats.latency24h ?? 0}
            suffix="ms"
            precision={0}
            valueStyle={{ color: stats.latency24h !== null && stats.latency24h > 1000 ? '#ef4444' : '#22c55e', fontSize: 22 }}
          />
        </Col>
        <Col xs={12} sm={8} md={4}>
          <Statistic
            title="Uptime 24h"
            value={stats.uptime24h ?? 0}
            suffix="%"
            precision={2}
            valueStyle={{ color: stats.uptime24h !== null && stats.uptime24h < 99 ? '#ef4444' : '#22c55e', fontSize: 22 }}
          />
        </Col>
        <Col xs={12} sm={8} md={4}>
          <Statistic
            title="Uptime 30d"
            value={stats.uptime30d ?? 0}
            suffix="%"
            precision={2}
            valueStyle={{ color: stats.uptime30d !== null && stats.uptime30d < 99 ? '#ef4444' : '#22c55e', fontSize: 22 }}
          />
        </Col>
        <Col xs={12} sm={8} md={4}>
          <Statistic
            title="Uptime 1a"
            value={stats.uptime1y ?? 0}
            suffix="%"
            precision={2}
            valueStyle={{ color: stats.uptime1y !== null && stats.uptime1y < 99 ? '#ef4444' : '#22c55e', fontSize: 22 }}
          />
        </Col>
        <Col xs={12} sm={8} md={4}>
          <Statistic
            title="Caducidad certificado"
            value={certDaysLeft !== null ? `${certDaysLeft} días` : '—'}
            valueStyle={{
              fontSize: 22,
              color: certDaysLeft !== null && certDaysLeft < 30 ? '#ef4444' : certDaysLeft !== null && certDaysLeft < 60 ? '#facc15' : '#22c55e',
            }}
          />
        </Col>
        {certExpiry && (
          <Col xs={12} sm={8} md={4}>
            <Statistic
              title="Expira el"
              value={dayjs(certExpiry).format('DD/MM/YYYY')}
              valueStyle={{ fontSize: 16, color: '#888' }}
            />
          </Col>
        )}
      </Row>
    </Card>
  );
}
function HealthLatencyChart({ buckets, rangeKey }: { buckets: TimelineBucket[]; rangeKey: number }) {
  const range = RANGE_OPTIONS[rangeKey];

  const bucketsWithData = buckets.filter(b => b.dominant_status !== 'no_data');
  const uptime = bucketsWithData.length > 0
    ? Math.round(bucketsWithData.reduce((sum, b) => sum + b.up_pct, 0) / bucketsWithData.length * 100) / 100
    : null;

  if (buckets.length === 0) return <Text type="secondary">Sin datos en {range.labelLong.toLowerCase()}</Text>;

  const maxRt = Math.max(...buckets.map(b => b.avg_response_time_ms), 1);
  const MAX_BARS = 150;
  const bars = buckets.length > MAX_BARS ? buckets.slice(-MAX_BARS) : buckets;
  const labelInterval = Math.max(1, Math.floor(bars.length / 6));

  return (
    <div>
      <div style={{ display: 'flex', gap: 1, alignItems: 'flex-end', height: 120 }}>
        {bars.map((b, i) => {
          const pct = (b.avg_response_time_ms / maxRt) * 100;
          const color = healthColor(b.up_pct, b.dominant_status);
          const tooltipTitle = b.dominant_status === 'no_data'
            ? `Sin datos · ${formatBucketTime(b.bucket_start, range.bucketSeconds)}`
            : `${b.dominant_status.toUpperCase()} · ${b.up_pct.toFixed(1)}% UP · ` +
              `${b.avg_response_time_ms.toFixed(0)}ms media · ` +
              `${b.count} checks · ${formatBucketTime(b.bucket_start, range.bucketSeconds)}`;
          return (
            <Tooltip key={i} title={tooltipTitle}>
              <div
                style={{
                  flex: '1 1 0',
                  minWidth: 2,
                  height: `${Math.max(pct, 3)}%`,
                  background: color,
                  borderRadius: '1px 1px 0 0',
                  cursor: 'pointer',
                  transition: 'opacity 0.15s',
                  opacity: b.up_pct < 50 ? 1 : 0.85,
                }}
                onMouseEnter={e => (e.currentTarget.style.opacity = '1')}
                onMouseLeave={e => {
                  e.currentTarget.style.opacity = b.up_pct < 50 ? '1' : '0.85';
                }}
              />
            </Tooltip>
          );
        })}
      </div>
      {/* Time axis labels */}
      <div style={{ display: 'flex', justifyContent: 'space-between', marginTop: 4, fontSize: 10, color: '#888' }}>
        {bars.length > 0 && (
          <>
            <span>{formatBucketTime(bars[0].bucket_start, range.bucketSeconds)}</span>
            {Array.from({ length: Math.min(5, Math.floor(bars.length / labelInterval)) }, (_, i) => {
              const idx = Math.min((i + 1) * labelInterval, bars.length - 1);
              return (
                <span key={i}>{formatBucketTime(bars[idx].bucket_start, range.bucketSeconds)}</span>
              );
            })}
            <span>{formatBucketTime(bars[bars.length - 1].bucket_start, range.bucketSeconds)}</span>
          </>
        )}
      </div>
      {/* Stats row */}
      <div style={{ marginTop: 8, display: 'flex', gap: 16, flexWrap: 'wrap', fontSize: 12, color: '#888' }}>
        <span><BarChartOutlined /> {bars.length} bloques · pico: {maxRt.toFixed(0)}ms</span>
        <span><ClockCircleOutlined /> {range.labelLong.toLowerCase()}</span>
        {uptime !== null && (
          <span
            style={{
              color: uptime >= 99 ? '#22c55e' : uptime >= 95 ? '#4ade80' : uptime >= 90 ? '#facc15' : '#ef4444',
              fontWeight: 600,
            }}
          >
            Uptime: {uptime.toFixed(2)}%
          </span>
        )}
      </div>
      {/* Dual legend */}
      <div style={{ marginTop: 8, display: 'flex', gap: 16, flexWrap: 'wrap', fontSize: 11, color: '#888', alignItems: 'center' }}>
        <span style={{ fontWeight: 500, color: '#555' }}>Salud (color):</span>
        <span style={{ color: '#22c55e' }}>■ ≥99%</span>
        <span style={{ color: '#86efac' }}>■ ≥90%</span>
        <span style={{ color: '#facc15' }}>■ ≥75%</span>
        <span style={{ color: '#fb923c' }}>■ ≥50%</span>
        <span style={{ color: '#ef4444' }}>■ {"<50%"}</span>
        <span style={{ color: '#6b7280' }}>■ Sin datos</span>
        <span style={{ marginLeft: 8, fontWeight: 500, color: '#555' }}>Latencia (altura):</span>
        <span>▮ más alto = más lento</span>
      </div>
    </div>
  );
}

export default function MonitorDetail() {
  const { id } = useParams();
  const [monitor, setMonitor] = useState<Monitor | null>(null);
  const [monitorLoading, setMonitorLoading] = useState(true);
  const [checks, setChecks] = useState<CheckResult[]>([]);
  const [buckets, setBuckets] = useState<TimelineBucket[]>([]);
  const [loading, setLoading] = useState(true);
  const [rangeKey, setRangeKey] = useState(3); // default: 24h

  // Pagination state for checks table
  const [checksPage, setChecksPage] = useState(1);
  const [checksPerPage, setChecksPerPage] = useState(20);
  const [checksTotal, setChecksTotal] = useState(0);
  const [checksTotalPages, setChecksTotalPages] = useState(0);

  const range = RANGE_OPTIONS[rangeKey];

  // ── Load monitor data ──
  useEffect(() => {
    if (!id) return;
    setMonitorLoading(true);
    fetchMonitor(id)
      .then(setMonitor)
      .catch(() => setMonitor(null))
      .finally(() => setMonitorLoading(false));
  }, [id]);

  // ── Load timeline buckets (only for non-heartbeat monitors) ──
  useEffect(() => {
    if (!id || (monitor && monitor.type === 'heartbeat')) return;
    const params: Record<string, number> = { bucket_seconds: range.bucketSeconds };
    if (range.hours != null) {
      (params as { hours: number; bucket_seconds: number }).hours = range.hours;
    } else if (range.days != null) {
      (params as { days: number; bucket_seconds: number }).days = range.days;
    }
    fetchTimelineBuckets(
      id,
      params as { bucket_seconds: number } & ({ hours: number } | { days: number }),
    )
      .then(({ buckets: b }) => setBuckets(b))
      .catch(() => setBuckets([]));
  }, [id, rangeKey, range.bucketSeconds, range.hours, range.days, monitor?.type]);

  // ── Load checks ──
  const loadChecks = useCallback(async (cp: number, cpp: number) => {
    if (!id) return;
    setLoading(true);
    try {
      const checksData = await fetchChecks(id, cp, cpp);
      setChecks(checksData.checks);
      setChecksPage(checksData.page);
      setChecksPerPage(checksData.per_page);
      setChecksTotal(checksData.total);
      setChecksTotalPages(checksData.total_pages);
    } catch {
      // ignore
    }
    setLoading(false);
  }, [id]);

  // Initial checks load
  useEffect(() => {
    loadChecks(checksPage, checksPerPage);
  }, [loadChecks]);

  const handleCheck = async () => {
    if (!id) return;
    try {
      await runCheck(id);
      loadChecks(1, checksPerPage);
    } catch { /* ignore */ }
  };

  if (monitorLoading) return <div style={{ textAlign: 'center', padding: 40 }}><Spin size="large" /></div>;
  if (!monitor) return <Typography.Text type="danger">Monitor no encontrado</Typography.Text>;

  // ── Heartbeat branch ──
  if (monitor.type === 'heartbeat') {
    return (
      <HeartbeatView
        monitor={monitor}
        checks={checks}
        loading={loading}
        loadChecks={loadChecks}
        checksPerPage={checksPerPage}
      />
    );
  }

  // ── Regular monitor view ──

  const checksColumns = [
    { title: 'Estado', dataIndex: 'status', key: 'status',
      render: (s: string) => <Tag color={STATUS_TAG[s]?.color}>{STATUS_TAG[s]?.text ?? s}</Tag>,
    },
    { title: 'Código', dataIndex: 'status_code', key: 'code', width: 80,
      render: (v: number | null) => v ?? '—',
    },
    { title: 'Latencia', dataIndex: 'response_time_ms', key: 'latency', width: 100,
      render: (v: number) => `${v} ms`,
    },
    { title: 'Error', dataIndex: 'error_message', key: 'error', ellipsis: true,
      render: (v: string | null) => v ?? '—',
    },
    { title: 'Fecha', dataIndex: 'checked_at', key: 'date', width: 160,
      render: (v: string) => dayjs(v).format('DD/MM/YYYY HH:mm:ss'),
    },
  ];

  return (
    <div>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <Title level={3} style={{ margin: 0 }}>{monitor.name}</Title>
        <Space>
          <Button icon={<ReloadOutlined />} onClick={() => loadChecks(1, checksPerPage)}>Recargar</Button>
          <Button icon={<PlayCircleOutlined />} onClick={handleCheck}>Check ahora</Button>
        </Space>
      </div>

      <Descriptions column={3} style={{ marginTop: 16 }}>
        <Descriptions.Item label="Tipo">{monitor.type}</Descriptions.Item>
        <Descriptions.Item label="Target">{monitor.target}</Descriptions.Item>
        <Descriptions.Item label="Estado">
          <Tag color={checks[0] ? STATUS_TAG[checks[0].status]?.color : 'default'}>
            {checks[0] ? STATUS_TAG[checks[0].status]?.text ?? '—' : 'Sin datos'}
          </Tag>
        </Descriptions.Item>
        <Descriptions.Item label="Intervalo">{monitor.interval_seconds}s</Descriptions.Item>
        <Descriptions.Item label="Timeout">{monitor.timeout_seconds}s</Descriptions.Item>
        <Descriptions.Item label="Umbral latencia">
          {monitor.latency_threshold_ms != null ? `>${monitor.latency_threshold_ms}ms` : '—'}
        </Descriptions.Item>
        <Descriptions.Item label="Plantilla expiry">
          {monitor.message_template_expiry ? 'Personalizada' : 'Por defecto'}
        </Descriptions.Item>
        {monitor.config_json && Object.keys(monitor.config_json).length > 0 && (
          <Descriptions.Item label="Config">
            <Typography.Text code>{JSON.stringify(monitor.config_json)}</Typography.Text>
          </Descriptions.Item>
        )}
      </Descriptions>

      {/* Stats card */}
      <MonitorStats monitor={monitor} latestCheck={checks[0] ?? null} />

      {/* Health & Latency Chart */}
      <Card style={{ marginTop: 16 }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 12 }}>
          <Text strong><BarChartOutlined /> Health & Latency</Text>
          <Space size={4} wrap>
            {RANGE_OPTIONS.map((opt, i) => (
              <Tooltip key={opt.label} title={opt.labelLong}>
                <Button
                  size="small"
                  type={rangeKey === i ? 'primary' : 'default'}
                  onClick={() => setRangeKey(i)}
                >
                  {opt.label}
                </Button>
              </Tooltip>
            ))}
          </Space>
        </div>
        <HealthLatencyChart buckets={buckets} rangeKey={rangeKey} />
      </Card>

      {/* History table */}
      <Card title="Histórico" style={{ marginTop: 16 }}>
        <Table
          dataSource={checks}
          columns={checksColumns}
          rowKey="id"
          pagination={{
            current: checksPage,
            pageSize: checksPerPage,
            total: checksTotal,
            showSizeChanger: true,
            pageSizeOptions: ['10', '20', '50', '100'],
            showTotal: (total, range) => `${range[0]}-${range[1]} de ${total} checks (pág. ${checksPage} de ${checksTotalPages})`,
            onChange: (p, ps) => loadChecks(p, ps),
          }}
          size="small"
        />
      </Card>
    </div>
  );
}