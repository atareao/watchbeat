import { useEffect, useState, useRef, useCallback } from 'react';
import {
  Card, Col, Row, Statistic, Typography, Spin, Tag, Button, Input, Select, Pagination, Space, Modal, Form, InputNumber, Switch, Tabs, message,
} from 'antd';
import {
  RocketOutlined, CheckCircleOutlined, CloseCircleOutlined,
  FieldTimeOutlined, DashboardOutlined, PlusOutlined, ReloadOutlined, SearchOutlined, SettingOutlined,
} from '@ant-design/icons';
import { useNavigate } from 'react-router';
import {
  fetchMonitors, createMonitor, updateMonitor, deleteMonitor, fetchNotifiers,
  type DashboardStatus, type MonitorSummary, type UnifiedDashboardResponse,
} from '../api/http';
import MonitorCard from '../components/MonitorCard';
import dayjs from 'dayjs';
import relativeTime from 'dayjs/plugin/relativeTime';
import 'dayjs/locale/es';

dayjs.extend(relativeTime);
dayjs.locale('es');

const { Title } = Typography;

const MONITOR_TYPES = [
  { value: 'http', label: 'HTTP(S)' },
  { value: 'tcp', label: 'TCP' },
  { value: 'ping', label: 'Ping' },
  { value: 'tls', label: 'TLS/SSL' },
];

const TYPE_FILTER_OPTIONS = [
  { value: '', label: 'Todos los tipos' },
  ...MONITOR_TYPES,
];

const STATUS_FILTER_OPTIONS = [
  { value: '', label: 'Todos los estados' },
  { value: 'up', label: 'UP' },
  { value: 'down', label: 'DOWN' },
  { value: 'error', label: 'Error' },
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

export default function Dashboard() {
  const navigate = useNavigate();

  // Data state
  const [status, setStatus] = useState<DashboardStatus | null>(null);
  const [monitors, setMonitors] = useState<MonitorSummary[]>([]);
  const [scheduler, setScheduler] = useState<UnifiedDashboardResponse['scheduler']>({ last_run_at: null, next_run_at: null, last_monitors_checked: 0 });
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Pagination state
  const [page, setPage] = useState(1);
  const [perPage, setPerPage] = useState(20);
  const [total, setTotal] = useState(0);
  const [totalPages, setTotalPages] = useState(0);

  // Filter state
  const [searchQuery, setSearchQuery] = useState('');
  const [typeFilter, setTypeFilter] = useState('');
  const [statusFilter, setStatusFilter] = useState('');
  const [debouncedSearch, setDebouncedSearch] = useState('');

  // Modal state
  const [modalOpen, setModalOpen] = useState(false);
  const [editingMonitor, setEditingMonitor] = useState<MonitorSummary | null>(null);
  const [selectedType, setSelectedType] = useState<string>('http');
  const [notifiers, setNotifiers] = useState<{ id: string; name: string }[]>([]);
  const [form] = Form.useForm();

  // Track if any modal is open for auto-refresh pausing
  const anyModalOpen = modalOpen;

  // Debounce search
  const debounceRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  useEffect(() => {
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => {
      setDebouncedSearch(searchQuery);
      setPage(1);
    }, 300);
    return () => { if (debounceRef.current) clearTimeout(debounceRef.current); };
  }, [searchQuery]);

  // Reset page when filters change
  useEffect(() => { setPage(1); }, [typeFilter, statusFilter]);

  // Load data
  const load = useCallback((p?: number, pp?: number) => {
    const currentPage = p ?? page;
    const currentPerPage = pp ?? perPage;
    setLoading(true);
    setError(null);
    fetchMonitors({
      page: currentPage,
      perPage: currentPerPage,
      q: debouncedSearch || undefined,
      type: typeFilter || undefined,
      status: statusFilter || undefined,
    })
      .then(data => {
        setStatus(data.status);
        setMonitors(data.monitors);
        setScheduler(data.scheduler);
        setTotal(data.total);
        setPage(data.page);
        setPerPage(data.per_page);
        setTotalPages(data.total_pages);
      })
      .catch(err => {
        setError(err.message);
        message.error(err.message);
      })
      .finally(() => setLoading(false));
  }, [debouncedSearch, typeFilter, statusFilter, page, perPage]);

  // Load notifiers once
  useEffect(() => {
    fetchNotifiers()
      .then(nData => setNotifiers(nData.notifiers.map(n => ({ id: n.id, name: n.name }))))
      .catch(() => {});
  }, []);

  // Initial load & reload on filter/pagination change
  useEffect(() => {
    load();
  }, [debouncedSearch, typeFilter, statusFilter, page, perPage]);

  // Auto-refresh every 30 seconds, paused when modal is open
  useEffect(() => {
    if (anyModalOpen) return;
    const interval = setInterval(() => load(), 30_000);
    return () => clearInterval(interval);
  }, [anyModalOpen, load]);

  // ── Modal handlers ──

  const handleCreate = () => {
    setEditingMonitor(null);
    form.resetFields();
    form.setFieldsValue({ type: 'http', interval_seconds: 300, timeout_seconds: 30, enabled: true, confirmations_required: 0, config: {} });
    setSelectedType('http');
    setModalOpen(true);
  };

  const handleEdit = (m: MonitorSummary) => {
    setEditingMonitor(m);
    form.resetFields();
    // We need to fetch the full monitor data for editing fields
    // Since MonitorSummary doesn't have all fields, we'll do a quick fetch
    import('../api/http').then(({ fetchMonitor }) => {
      fetchMonitor(m.id).then(full => {
        form.setFieldsValue({
          name: full.name,
          type: full.type,
          target: full.target,
          interval_seconds: full.interval_seconds,
          timeout_seconds: full.timeout_seconds,
          enabled: full.enabled,
          notifier_id: full.notifier_id ?? null,
          confirmations_required: (full as any).confirmations_required ?? 0,
          config: full.config_json ?? {},
          latency_threshold_ms: full.latency_threshold_ms,
          message_template_down: full.message_template_down,
          message_template_latency: full.message_template_latency,
          message_template_up: full.message_template_up,
          message_template_expiry: full.message_template_expiry,
        });
        setSelectedType(full.type);
        setModalOpen(true);
      }).catch(() => message.error('Error al cargar monitor'));
    }).catch(() => message.error('Error al cargar monitor'));
  };

  const handleSave = async () => {
    try {
      const values = await form.validateFields();
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
      if (editingMonitor) {
        await updateMonitor(editingMonitor.id, payload);
        message.success('Monitor actualizado');
      } else {
        await createMonitor(payload);
        message.success('Monitor creado');
      }
      setModalOpen(false);
      load();
    } catch (err: unknown) {
      if (err && typeof err === 'object' && 'errorFields' in err) return;
      message.error('Error al guardar');
    }
  };

  const handleDelete = async (id: string) => {
    try {
      await deleteMonitor(id);
      message.success('Monitor eliminado');
      load();
    } catch { message.error('Error al eliminar'); }
  };

  const handleRefresh = () => {
    load();
  };

  // ── Render ──

  if (loading && !status) {
    return <div style={{ textAlign: 'center', padding: 40 }}><Spin size="large" /></div>;
  }

  if (error && !status) {
    return (
      <div style={{ textAlign: 'center', padding: 40 }}>
        <Typography.Text type="danger">Error al cargar: {error}</Typography.Text>
        <br />
        <Button onClick={handleRefresh} style={{ marginTop: 16 }}>Reintentar</Button>
      </div>
    );
  }

  return (
    <div className="fade-in-up">
      {/* Header */}
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 16, flexWrap: 'wrap', gap: 8 }}>
        <Title level={3} style={{ margin: 0 }}><DashboardOutlined /> Dashboard</Title>
        <Space wrap>
          <Input
            placeholder="Buscar monitores..."
            prefix={<SearchOutlined />}
            value={searchQuery}
            onChange={e => setSearchQuery(e.target.value)}
            style={{ width: 200 }}
            allowClear
          />
          <Select
            options={TYPE_FILTER_OPTIONS}
            value={typeFilter}
            onChange={setTypeFilter}
            style={{ width: 150 }}
          />
          <Select
            options={STATUS_FILTER_OPTIONS}
            value={statusFilter}
            onChange={setStatusFilter}
            style={{ width: 150 }}
          />
          <Button icon={<ReloadOutlined />} onClick={handleRefresh} loading={loading}>Recargar</Button>
          <Button type="primary" icon={<PlusOutlined />} onClick={handleCreate}>Añadir monitor</Button>
        </Space>
      </div>

      {/* Stats row */}
      {status && (
        <Row gutter={[16, 16]} style={{ marginBottom: 16 }}>
          <Col xs={12} sm={12} lg={6}>
            <Card><Statistic title="Monitores" value={status.total_monitors} prefix={<RocketOutlined style={{ color: '#1677ff' }} />} /></Card>
          </Col>
          <Col xs={12} sm={12} lg={6}>
            <Card><Statistic title="UP" value={status.up_monitors} prefix={<CheckCircleOutlined style={{ color: '#22c55e' }} />} suffix={`/ ${status.enabled_monitors}`} /></Card>
          </Col>
          <Col xs={12} sm={12} lg={6}>
            <Card><Statistic title="DOWN" value={status.down_monitors} prefix={<CloseCircleOutlined style={{ color: '#ef4444' }} />} /></Card>
          </Col>
          <Col xs={12} sm={12} lg={6}>
            <Card><Statistic title="Latencia media" value={status.avg_response_time_24h ?? 0} suffix="ms" prefix={<FieldTimeOutlined style={{ color: '#a78bfa' }} />} /></Card>
          </Col>
        </Row>
      )}

      {/* Scheduler info */}
      <div style={{ marginBottom: 16, display: 'flex', gap: 16, flexWrap: 'wrap' }}>
        <Typography.Text type="secondary" style={{ fontSize: 12 }}>
          Última ejecución: {scheduler.last_run_at ? dayjs(scheduler.last_run_at).fromNow() : '—'}
        </Typography.Text>
        <Typography.Text type="secondary" style={{ fontSize: 12 }}>
          Próxima ejecución: {scheduler.next_run_at ? dayjs(scheduler.next_run_at).fromNow() : '—'}
        </Typography.Text>
        <Typography.Text type="secondary" style={{ fontSize: 12 }}>
          Monitores verificados: {scheduler.last_monitors_checked}
        </Typography.Text>
      </div>

      {/* Loading overlay for subsequent loads */}
      {loading && (
        <div style={{ textAlign: 'center', padding: 16 }}>
          <Spin />
        </div>
      )}

      {/* Monitors grid */}
      <Row gutter={[16, 16]}>
        {monitors.map(m => (
          <Col xs={24} sm={12} lg={8} xl={6} key={m.id}>
            <MonitorCard
              monitor={m}
              onToggle={load}
              onEdit={handleEdit}
              onDelete={handleDelete}
              onCheck={load}
            />
          </Col>
        ))}
      </Row>

      {/* Empty state */}
      {!loading && monitors.length === 0 && (
        <Card style={{ marginTop: 16 }}>
          <Typography.Text type="secondary">
            {debouncedSearch || typeFilter || statusFilter
              ? 'No hay monitores que coincidan con los filtros'
              : 'No hay monitores configurados. Crea tu primer monitor.'}
          </Typography.Text>
        </Card>
      )}

      {/* Pagination */}
      {total > 0 && (
        <div style={{ marginTop: 16, display: 'flex', justifyContent: 'center', flexDirection: 'column', alignItems: 'center' }}>
          <Pagination
            current={page}
            pageSize={perPage}
            total={total}
            showSizeChanger
            pageSizeOptions={['10', '20', '50', '100']}
            showTotal={(total, range) => `${range[0]}-${range[1]} de ${total} monitores (pág. ${page} de ${totalPages})`}
            onChange={(p, ps) => {
              setPage(p);
              setPerPage(ps);
            }}
          />
        </div>
      )}

      {/* Create/Edit Modal */}
      <Modal
        title={editingMonitor ? 'Editar monitor' : 'Nuevo monitor'}
        open={modalOpen}
        onOk={handleSave}
        onCancel={() => setModalOpen(false)}
        width={600}
      >
        <Form form={form} layout="vertical" onValuesChange={(changedValues) => {
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