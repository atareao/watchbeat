import { useEffect, useState, useCallback } from 'react';
import { useParams } from 'react-router';
import { Card, Typography, Spin, Table, Tag, Button, Descriptions, Space, Tooltip } from 'antd';
import { ReloadOutlined, PlayCircleOutlined, ClockCircleOutlined, BarChartOutlined } from '@ant-design/icons';
import { fetchMonitors, fetchChecks, fetchTimelineBuckets, runCheck, type Monitor, type CheckResult, type TimelineBucket } from '../api/http';
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
  if (status === 'no_data') return '#6b7280';  // gray for no data
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

export default function MonitorDetail() {
  const { id } = useParams();
  const [monitor, setMonitor] = useState<Monitor | null>(null);
  const [checks, setChecks] = useState<CheckResult[]>([]);
  const [buckets, setBuckets] = useState<TimelineBucket[]>([]);
  const [loading, setLoading] = useState(true);
  const [rangeKey, setRangeKey] = useState(4); // default: 7d

  const range = RANGE_OPTIONS[rangeKey];

  const load = useCallback(async () => {
    if (!id) return;
    setLoading(true);
    try {
      const { monitors } = await fetchMonitors();
      const m = monitors.find(m => m.id === id) ?? null;
      setMonitor(m);

      const { checks: c } = await fetchChecks(id, 100);
      setChecks(c);

      const params: Record<string, number> = { bucket_seconds: range.bucketSeconds };
      if (range.hours != null) {
        (params as { hours: number; bucket_seconds: number }).hours = range.hours;
      } else if (range.days != null) {
        (params as { days: number; bucket_seconds: number }).days = range.days;
      }
      const { buckets: b } = await fetchTimelineBuckets(
        id,
        params as { bucket_seconds: number } & ({ hours: number } | { days: number }),
      );
      setBuckets(b);
    } catch { /* ignore */ }
    setLoading(false);
  }, [id, rangeKey, range.bucketSeconds, range.hours, range.days]);

  useEffect(() => { load(); }, [load]);

  const handleCheck = async () => {
    if (!id) return;
    try {
      await runCheck(id);
      load();
    } catch { /* ignore */ }
  };

  if (loading) return <div style={{ textAlign: 'center', padding: 40 }}><Spin size="large" /></div>;
  if (!monitor) return <Typography.Text type="danger">Monitor no encontrado</Typography.Text>;

  // ── Uptime calculation from buckets (exclude no_data) ──
  const bucketsWithData = buckets.filter(b => b.dominant_status !== 'no_data');
  const uptime = bucketsWithData.length > 0
    ? Math.round(bucketsWithData.reduce((sum, b) => sum + b.up_pct, 0) / bucketsWithData.length * 100) / 100
    : null;

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

  // ── Health & Latency chart (unified) ──
  //   - Bar height = latency (como el chart de latencia)
  //   - Bar color  = % UP (verde = bien, rojo = mal)
  //   Así un solo gráfico muestra ambas dimensiones
  const renderHealthLatencyChart = () => {
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
        {/* Dual legend: health (color) + latency (height) */}
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
  };

  return (
    <div className="fade-in-up">
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <Title level={3} style={{ margin: 0 }}>{monitor.name}</Title>
        <Space>
          <Button icon={<ReloadOutlined />} onClick={load}>Recargar</Button>
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
        <Descriptions.Item label={`Uptime ${range.label}`}>
          {uptime !== null ? `${uptime.toFixed(2)}%` : '—'}
        </Descriptions.Item>
        {monitor.config_json && Object.keys(monitor.config_json).length > 0 && (
          <Descriptions.Item label="Config">
            <Typography.Text code>{JSON.stringify(monitor.config_json)}</Typography.Text>
          </Descriptions.Item>
        )}
      </Descriptions>

      {/* ── Unified Health & Latency Chart ── */}
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
        {renderHealthLatencyChart()}
      </Card>

      {/* ── History table ── */}
      <Card title="Histórico" style={{ marginTop: 16 }}>
        <Table
          dataSource={checks}
          columns={checksColumns}
          rowKey="id"
          pagination={{ pageSize: 20, showSizeChanger: true, pageSizeOptions: ['10', '20', '50', '100'] }}
          size="small"
        />
      </Card>
    </div>
  );
}