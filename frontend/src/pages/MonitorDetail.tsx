import { useEffect, useState, useCallback } from 'react';
import { useParams } from 'react-router';
import { Card, Typography, Spin, Table, Tag, Button, Descriptions, Space, Tooltip } from 'antd';
import { ReloadOutlined, PlayCircleOutlined, ClockCircleOutlined } from '@ant-design/icons';
import { fetchMonitors, fetchChecks, fetchTimeline, runCheck, type Monitor, type CheckResult, type TimelinePoint } from '../api/http';
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

const STATUS_COLOR: Record<string, string> = {
  up: '#22c55e',
  down: '#ef4444',
  error: '#f59e0b',
};

const RANGE_OPTIONS = [
  { label: '1h', value: { hours: 1 } },
  { label: '6h', value: { hours: 6 } },
  { label: '12h', value: { hours: 12 } },
  { label: '1d', value: { days: 1 } },
  { label: '7d', value: { days: 7 } },
  { label: '15d', value: { days: 15 } },
  { label: '30d', value: { days: 30 } },
  { label: '6m', value: { days: 180 } },
] as const;

type RangeValue = (typeof RANGE_OPTIONS[number]['value']);

function getRangeHours(r: RangeValue): number | undefined {
  return 'hours' in r ? r.hours : undefined;
}
function getRangeDays(r: RangeValue): number | undefined {
  return 'days' in r ? r.days : undefined;
}

export default function MonitorDetail() {
  const { id } = useParams();
  const [monitor, setMonitor] = useState<Monitor | null>(null);
  const [checks, setChecks] = useState<CheckResult[]>([]);
  const [timeline, setTimeline] = useState<TimelinePoint[]>([]);
  const [loading, setLoading] = useState(true);
  const [range, setRange] = useState<RangeValue>({ days: 7 });

  const load = useCallback(async () => {
    if (!id) return;
    setLoading(true);
    try {
      const { monitors } = await fetchMonitors();
      const m = monitors.find(m => m.id === id) ?? null;
      setMonitor(m);

      const { checks: c } = await fetchChecks(id, 100);
      setChecks(c);

      const { timeline: t } = await fetchTimeline(id, range);
      setTimeline(t);
    } catch { /* ignore */ }
    setLoading(false);
  }, [id, range]);

  useEffect(() => { load(); }, [load]);

  const handleCheck = async () => {
    if (!id) return;
    try {
      await runCheck(id);
      load();
    } catch { /* ignore */ }
  };

  const rangeLabel = RANGE_OPTIONS.find(
    o => getRangeHours(o.value) === getRangeHours(range) || getRangeDays(o.value) === getRangeDays(range)
  )?.label ?? '7d';

  if (loading) return <div style={{ textAlign: 'center', padding: 40 }}><Spin size="large" /></div>;
  if (!monitor) return <Typography.Text type="danger">Monitor no encontrado</Typography.Text>;

  const uptime = timeline.length > 0
    ? Math.round((timeline.filter(t => t.status === 'up').length / timeline.length) * 100)
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

  // ── Timeline bars ──
  const renderTimelineBars = () => {
    if (timeline.length === 0) return <Text type="secondary">Sin datos en este rango</Text>;
    return (
      <div style={{ display: 'flex', gap: 2, flexWrap: 'wrap' }}>
        {timeline.map((t, i) => (
          <Tooltip key={i} title={`${t.status.toUpperCase()} · ${t.response_time_ms ?? '—'}ms · ${dayjs(t.checked_at).format('DD/MM HH:mm')}`}>
            <div
              style={{
                width: 12,
                height: 24,
                background: STATUS_COLOR[t.status] ?? '#6b7280',
                borderRadius: 2,
                cursor: 'pointer',
              }}
            />
          </Tooltip>
        ))}
      </div>
    );
  };

  // ── Latency chart ──
  const renderLatencyChart = () => {
    const withRt = timeline.filter(t => t.response_time_ms != null && t.response_time_ms > 0);
    if (withRt.length === 0) return <Text type="secondary">Sin datos de latencia en este rango</Text>;

    const maxRt = Math.max(...withRt.map(t => t.response_time_ms ?? 0), 1);
    // Show max 200 bars to avoid visual overload
    const bars = withRt.length > 200 ? withRt.slice(-200) : withRt;

    return (
      <div>
        <div style={{ display: 'flex', gap: 2, alignItems: 'flex-end', height: 100 }}>
          {bars.map((t, i) => {
            const rt = t.response_time_ms ?? 0;
            const pct = (rt / maxRt) * 100;
            return (
              <Tooltip key={i} title={`${rt}ms · ${dayjs(t.checked_at).format('DD/MM HH:mm')}`}>
                <div
                  style={{
                    flex: '1 1 0',
                    minWidth: 3,
                    height: `${Math.max(pct, 4)}%`,
                    background: t.status === 'up' ? '#1677ff' : t.status === 'down' ? '#ef4444' : '#f59e0b',
                    borderRadius: '2px 2px 0 0',
                    cursor: 'pointer',
                    transition: 'opacity 0.15s',
                  }}
                  onMouseEnter={e => (e.currentTarget.style.opacity = '0.7')}
                  onMouseLeave={e => (e.currentTarget.style.opacity = '1')}
                />
              </Tooltip>
            );
          })}
        </div>
        <Text type="secondary" style={{ fontSize: 11, marginTop: 4, display: 'block' }}>
          {bars.length} muestras · pico: {maxRt}ms · rango: {rangeLabel}
        </Text>
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
        <Descriptions.Item label={`Uptime ${rangeLabel}`}>{uptime !== null ? `${uptime}%` : '—'}</Descriptions.Item>
        {monitor.config_json && Object.keys(monitor.config_json).length > 0 && (
          <Descriptions.Item label="Config">
            <Typography.Text code>{JSON.stringify(monitor.config_json)}</Typography.Text>
          </Descriptions.Item>
        )}
      </Descriptions>

      {/* ── Range selector ── */}
      <Card style={{ marginTop: 16 }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 12 }}>
          <Text strong><ClockCircleOutlined /> Timeline</Text>
          <Space size={4}>
            {RANGE_OPTIONS.map(opt => {
              const isActive = getRangeHours(opt.value) === getRangeHours(range) ||
                getRangeDays(opt.value) === getRangeDays(range);
              return (
                <Button
                  key={opt.label}
                  size="small"
                  type={isActive ? 'primary' : 'default'}
                  onClick={() => setRange(opt.value)}
                >
                  {opt.label}
                </Button>
              );
            })}
          </Space>
        </div>
        {renderTimelineBars()}
      </Card>

      {/* ── Latency chart ── */}
      <Card title={`Latencia — ${rangeLabel}`} style={{ marginTop: 16 }}>
        {renderLatencyChart()}
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