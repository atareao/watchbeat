import { useEffect, useState, useCallback } from 'react';
import { useParams, useNavigate } from 'react-router';
import {
  Card, Typography, Spin, Table, Tag, Button, Descriptions, Space, Tooltip, message, Statistic, Row, Col,
  Modal, Form, Input, InputNumber, Select, Switch, Tabs,
} from 'antd';
import {
  ReloadOutlined, BarChartOutlined,
  ClockCircleOutlined, EditOutlined, SettingOutlined,
} from '@ant-design/icons';
import {
  fetchMonitor, fetchChecks, fetchTimelineBuckets, fetchNotifiers,
  updateMonitor,
  type Monitor, type CheckResult, type TimelineBucket,
} from '../api/http';
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

const MONITOR_TYPES = [
  { value: 'http', label: 'HTTP(S)' },
  { value: 'tcp', label: 'TCP' },
  { value: 'ping', label: 'Ping' },
  { value: 'tls', label: 'TLS/SSL' },
];

const CONFIG_FIELDS: Record<string, { name: string; label: string; type: string; defaultValue?: unknown }[]> = {
  http: [
    { name: 'method', label: 'Método HTTP', type: 'select', defaultValue: 'GET' },
    { name: 'expected_status', label: 'Status esperado', type: 'number', defaultValue: 200 },
    { name: 'expected_body', label: 'Body esperado', type: 'text' },
    { name: 'body_is_regex', label: 'Body es regex', type: 'boolean', defaultValue: false },
    { name: 'expiry_days', label: 'Días para expiry del certificado', type: 'number', defaultValue: 14 },
  ],
  tls: [
    { name: 'expiry_days', label: 'Días para expiry', type: 'number', defaultValue: 14 },
  ],
  tcp: [],
  ping: [],
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
  const navigate = useNavigate();
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

  // Edit modal state
  const [editModalOpen, setEditModalOpen] = useState(false);
  const [selectedType, setSelectedType] = useState<string>('http');
  const [notifiers, setNotifiers] = useState<{ id: string; name: string }[]>([]);
  const [editForm] = Form.useForm();

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

  // ── Load timeline buckets ──
  useEffect(() => {
    if (!id) return;
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
  }, [id, rangeKey, range.bucketSeconds, range.hours, range.days]);

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

  // ── Edit handlers ──
  const openEdit = () => {
    if (!monitor) return;
    fetchNotifiers().then(nData => {
      setNotifiers(nData.notifiers.map(n => ({ id: n.id, name: n.name })));
    }).catch(() => {});
    setSelectedType(monitor.type);
    editForm.setFieldsValue({
      name: monitor.name,
      type: monitor.type,
      target: monitor.target,
      interval_seconds: monitor.interval_seconds,
      timeout_seconds: monitor.timeout_seconds,
      enabled: monitor.enabled,
      notifier_id: monitor.notifier_id ?? null,
      confirmations_required: (monitor as any).confirmations_required ?? 0,
      config: monitor.config_json ?? {},
      latency_threshold_ms: monitor.latency_threshold_ms,
      message_template_down: monitor.message_template_down,
      message_template_latency: monitor.message_template_latency,
      message_template_up: monitor.message_template_up,
      message_template_expiry: monitor.message_template_expiry,
    });
    setEditModalOpen(true);
  };

  const handleSave = async () => {
    if (!id) return;
    try {
      const values = await editForm.validateFields();
      const payload = {
        name: values.name,
        type: values.type,
        target: values.target,
        interval_seconds: values.interval_seconds,
        timeout_seconds: values.timeout_seconds,
        enabled: values.enabled,
        notifier_id: values.notifier_id || null,
        confirmations_required: values.confirmations_required ?? 0,
        config: (values.config ?? {}) as any,
        latency_threshold_ms: values.latency_threshold_ms ?? null,
        message_template_down: values.message_template_down || null,
        message_template_latency: values.message_template_latency || null,
        message_template_up: values.message_template_up || null,
        message_template_expiry: values.message_template_expiry || null,
      };
      await updateMonitor(id, payload);
      message.success('Monitor actualizado');
      setEditModalOpen(false);
      // Reload monitor data
      const updated = await fetchMonitor(id);
      setMonitor(updated);
    } catch (err: unknown) {
      if (err && typeof err === 'object' && 'errorFields' in err) return;
      message.error('Error al guardar');
    }
  };

  if (monitorLoading) return <div style={{ textAlign: 'center', padding: 40 }}><Spin size="large" /></div>;
  if (!monitor) return <Typography.Text type="danger">Monitor no encontrado</Typography.Text>;

  // ── Common view for all monitor types (including heartbeat) ──

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
          <Button icon={<EditOutlined />} onClick={openEdit}>Editar</Button>
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

      {/* Edit modal */}
      <Modal
        title="Editar monitor"
        open={editModalOpen}
        onOk={handleSave}
        onCancel={() => setEditModalOpen(false)}
        width={600}
      >
        <Form form={editForm} layout="vertical" onValuesChange={(changedValues) => {
          if ('type' in changedValues) {
            setSelectedType(changedValues.type);
          }
        }}>
          <Tabs>
            <Tabs.TabPane tab="General" key="general">
              <Form.Item name="name" label="Nombre" rules={[{ required: true }]}>
                <Input />
              </Form.Item>
              <Form.Item name="type" label="Tipo" rules={[{ required: true }]}>
                <Select options={MONITOR_TYPES} />
              </Form.Item>
              <Form.Item name="target" label="Target" rules={[{ required: true }]}
                extra={(() => {
                  if (selectedType === 'http') return 'URL completa, ej: https://ejemplo.com';
                  if (selectedType === 'tls') return 'host o host:puerto (sin https://), ej: atareao.es';
                  if (selectedType === 'tcp') return 'host:puerto, ej: ejemplo.com:443';
                  if (selectedType === 'ping') return 'IP o dominio, ej: 8.8.8.8';
                  return 'URL, host:puerto, o IP';
                })()}
              >
                <Input placeholder={(() => {
                  if (selectedType === 'tls') return 'atareao.es';
                  if (selectedType === 'tcp') return 'ejemplo.com:443';
                  if (selectedType === 'ping') return '8.8.8.8';
                  return 'https://ejemplo.com';
                })()} />
              </Form.Item>
              <Space style={{ width: '100%' }} size="large">
                <Form.Item name="interval_seconds" label="Intervalo (s)">
                  <InputNumber min={10} max={86400} />
                </Form.Item>
                <Form.Item name="timeout_seconds" label="Timeout (s)">
                  <InputNumber min={1} max={120} />
                </Form.Item>
                <Form.Item name="confirmations_required" label="Confirmaciones">
                  <InputNumber min={0} max={10} />
                </Form.Item>
              </Space>
              <Space style={{ width: '100%' }} size="large">
                <Form.Item name="enabled" label="Habilitado" valuePropName="checked">
                  <Switch />
                </Form.Item>
                <Form.Item name="notifier_id" label="Notificador" style={{ minWidth: 200 }}>
                  <Select allowClear placeholder="Ninguno" options={notifiers.map(n => ({ value: n.id, label: n.name }))} />
                </Form.Item>
              </Space>
              <Form.Item name="latency_threshold_ms" label="Umbral de latencia (ms)"
                tooltip="Si la latencia supera este valor estando UP, se envía una notificación de latencia alta">
                <InputNumber min={0} max={60000} style={{ width: '100%' }} placeholder="Ej: 500" />
              </Form.Item>
            </Tabs.TabPane>
            <Tabs.TabPane tab="Específico" key="specific">
              {CONFIG_FIELDS[selectedType]?.length > 0 ? (
                CONFIG_FIELDS[selectedType].map(field => (
                  <Form.Item key={field.name} name={['config', field.name]} label={field.label} valuePropName={field.type === 'boolean' ? 'checked' : undefined}>
                    {field.type === 'select' ? (
                      <Select options={['GET', 'HEAD', 'POST'].map(v => ({ value: v, label: v }))} />
                    ) : field.type === 'number' ? (
                      <InputNumber style={{ width: '100%' }} />
                    ) : field.type === 'boolean' ? (
                      <Switch />
                    ) : (
                      <Input />
                    )}
                  </Form.Item>
                ))
              ) : (
                <Typography.Text type="secondary">No hay opciones específicas para este tipo de monitor.</Typography.Text>
              )}
            </Tabs.TabPane>
            <Tabs.TabPane tab="Plantillas" key="templates">
              <div style={{ marginBottom: 16 }}>
                <Typography.Text type="secondary">
                  Las plantillas usan sintaxis Jinja2. Variables disponibles:{' '}
                  <code>{'{{ monitor_name }}'}</code>, <code>{'{{ target }}'}</code>,{' '}
                  <code>{'{{ response_time_ms }}'}</code>, <code>{'{{ error_message }}'}</code>,{' '}
                  <code>{'{{ status_code }}'}</code>, <code>{'{{ days_left }}'}</code>,{' '}
                  <code>{'{{ expiry_threshold_days }}'}</code>
                </Typography.Text>
                <br />
                <Button type="link" icon={<SettingOutlined />} onClick={() => navigate('/settings')}>
                  Configurar plantillas por defecto
                </Button>
              </div>
              <Tabs>
                <Tabs.TabPane tab="DOWN" key="template-down">
                  <Form.Item name="message_template_down" label="Plantilla DOWN">
                    <Input.TextArea rows={6} placeholder={'⚠️ DOWN: {{ monitor_name }} — {{ target }} — Error: {{ error_message }}'} />
                  </Form.Item>
                </Tabs.TabPane>
                <Tabs.TabPane tab="LATENCIA" key="template-latency">
                  <Form.Item name="message_template_latency" label="Plantilla LATENCIA">
                    <Input.TextArea rows={6} placeholder={'⚠️ Latencia alta: {{ monitor_name }} — {{ response_time_ms }}ms (umbral: {{ latency_threshold_ms }}ms)'} />
                  </Form.Item>
                </Tabs.TabPane>
                <Tabs.TabPane tab="UP" key="template-up">
                  <Form.Item name="message_template_up" label="Plantilla UP">
                    <Input.TextArea rows={6} placeholder={'✅ UP: {{ monitor_name }} — {{ target }} — {{ response_time_ms }}ms'} />
                  </Form.Item>
                </Tabs.TabPane>
                <Tabs.TabPane tab="EXPIRACIÓN" key="template-expiry">
                  <Form.Item name="message_template_expiry" label="Plantilla EXPIRACIÓN">
                    <Input.TextArea
                      rows={6}
                      placeholder={'🟡 {{ monitor_name }} — {{ target }}\nCertificate expires in {{ days_left }} days (threshold: {{ expiry_threshold_days }} days)'}
                    />
                  </Form.Item>
                </Tabs.TabPane>
              </Tabs>
            </Tabs.TabPane>
          </Tabs>
        </Form>
      </Modal>
    </div>
  );
}
