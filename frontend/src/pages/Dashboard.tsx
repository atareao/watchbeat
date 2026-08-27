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
  fetchMonitors, createMonitor, updateMonitor, deleteMonitor, fetchNotifiers, fetchHeartbeats,
  createHeartbeat, updateHeartbeat, deleteHeartbeat,
  type DashboardStatus, type MonitorSummary, type UnifiedDashboardResponse, type Heartbeat,
  type DashboardItem,
} from '../api/http';
import MonitorCard from '../components/MonitorCard';
import dayjs from 'dayjs';
import relativeTime from 'dayjs/plugin/relativeTime';
import 'dayjs/locale/es';

dayjs.extend(relativeTime);
dayjs.locale('es');

const { Title } = Typography;

const ALL_TYPES = [
  { value: 'http', label: 'HTTP(S)' },
  { value: 'tcp', label: 'TCP' },
  { value: 'ping', label: 'Ping' },
  { value: 'tls', label: 'TLS/SSL' },
  { value: 'heartbeat', label: 'Heartbeat' },
];

const TYPE_FILTER_OPTIONS = [
  { value: '', label: 'Todos los tipos' },
  ...ALL_TYPES,
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
  const [heartbeats, setHeartbeats] = useState<Heartbeat[]>([]);
  const [scheduler, setScheduler] = useState<UnifiedDashboardResponse['scheduler']>({ last_run_at: null, next_run_at: null, last_monitors_checked: 0 });
  const [loading, setLoading] = useState(true);
  const [hbLoading, setHbLoading] = useState(true);
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

  // Modal state — single modal for both monitors and heartbeats
  const [modalOpen, setModalOpen] = useState(false);
  const [editingItem, setEditingItem] = useState<DashboardItem | null>(null);
  const [selectedType, setSelectedType] = useState<string>('http');
  const [notifiers, setNotifiers] = useState<{ id: string; name: string }[]>([]);
  const [form] = Form.useForm();

  // Track if any modal is open for auto-refresh pausing
  const anyModalOpen = modalOpen;

  // ── Derived data: combined items sorted by name ──
  const items: DashboardItem[] = [
    ...monitors.map(m => ({ ...m, kind: 'monitor' as const })),
    ...heartbeats.map(h => ({ ...h, kind: 'heartbeat' as const })),
  ].sort((a, b) => a.name.localeCompare(b.name));

  // ── Debounce search ──
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

  // ── Load data ──
  const loadMonitors = useCallback((p?: number, pp?: number) => {
    const currentPage = p ?? page;
    const currentPerPage = pp ?? perPage;
    const isHeartbeatFilter = typeFilter === 'heartbeat';

    if (isHeartbeatFilter) {
      // When filtering by heartbeat, no monitors to fetch
      setStatus(null);
      setMonitors([]);
      setScheduler({ last_run_at: null, next_run_at: null, last_monitors_checked: 0 });
      setTotal(0);
      setPage(1);
      setPerPage(currentPerPage);
      setTotalPages(0);
      setLoading(false);
      return;
    }

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

  const loadHeartbeats = useCallback(() => {
    fetchHeartbeats()
      .then(hData => { setHeartbeats(hData.heartbeats); setHbLoading(false); })
      .catch(() => setHbLoading(false));
  }, []);

  // Load notifiers and heartbeats once
  useEffect(() => {
    fetchNotifiers()
      .then(nData => setNotifiers(nData.notifiers.map(n => ({ id: n.id, name: n.name }))))
      .catch(() => {});
    loadHeartbeats();
  }, [loadHeartbeats]);

  // Initial load & reload on filter/pagination change
  useEffect(() => {
    loadMonitors();
  }, [loadMonitors]);

  // Auto-refresh every 30 seconds, paused when modal is open
  useEffect(() => {
    if (anyModalOpen) return;
    const interval = setInterval(() => {
      loadMonitors();
      loadHeartbeats();
    }, 30_000);
    return () => clearInterval(interval);
  }, [anyModalOpen, loadMonitors, loadHeartbeats]);

  // ── Unified stats ──
  const totalItems = (status?.total_monitors ?? 0) + heartbeats.length;
  const healthyItems = (status?.up_monitors ?? 0) + heartbeats.filter(h => h.status === 'ok').length;
  const problemItems = (status?.down_monitors ?? 0) + heartbeats.filter(h => h.status === 'missing').length;

  // ── Modal handlers ──

  const openCreateModal = () => {
    setEditingItem(null);
    form.resetFields();
    form.setFieldsValue({ type: 'http', interval_seconds: 300, timeout_seconds: 30, enabled: true, confirmations_required: 0, config: {} });
    setSelectedType('http');
    setModalOpen(true);
  };

  const handleEdit = (item: DashboardItem) => {
    setEditingItem(item);
    form.resetFields();

    if (item.kind === 'heartbeat') {
      // Edit heartbeat — data is already in the item
      form.setFieldsValue({
        name: item.name,
        type: 'heartbeat',
        grace_seconds: item.grace_seconds,
        notifier_id: item.notifier_id,
      });
      setSelectedType('heartbeat');
      setModalOpen(true);
    } else {
      // Edit monitor — fetch full data
      import('../api/http').then(({ fetchMonitor }) => {
        fetchMonitor(item.id).then(full => {
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
    }
  };

  const handleDelete = async (id: string) => {
    // We don't know if it's a monitor or heartbeat, try both
    // The caller (MonitorCard) already knows which API to call for heartbeats
    // For monitors, we handle it here
    try {
      await deleteMonitor(id);
      message.success('Monitor eliminado');
      loadMonitors();
    } catch {
      message.error('Error al eliminar');
    }
  };

  const handleSave = async () => {
    try {
      const values = await form.validateFields();
      const type = values.type;

      if (type === 'heartbeat') {
        // ── Save as heartbeat ──
        const payload = {
          name: values.name,
          grace_seconds: values.grace_seconds ?? 3600,
          notifier_id: values.notifier_id || null,
        };
        if (editingItem?.kind === 'heartbeat') {
          await updateHeartbeat(editingItem.id, payload);
          message.success('Heartbeat actualizado');
        } else {
          await createHeartbeat(payload);
          message.success('Heartbeat creado');
        }
        setModalOpen(false);
        loadHeartbeats();
      } else {
        // ── Save as monitor ──
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
        if (editingItem?.kind === 'monitor') {
          await updateMonitor(editingItem.id, payload);
          message.success('Monitor actualizado');
        } else {
          await createMonitor(payload);
          message.success('Monitor creado');
        }
        setModalOpen(false);
        loadMonitors();
      }
    } catch (err: unknown) {
      if (err && typeof err === 'object' && 'errorFields' in err) return;
      message.error('Error al guardar');
    }
  };

  const handleRefresh = () => {
    loadMonitors();
    loadHeartbeats();
  };

  const isHeartbeatSelected = selectedType === 'heartbeat';

  // ── Render ──

  if (loading && !status && heartbeats.length === 0) {
    return <div style={{ textAlign: 'center', padding: 40 }}><Spin size="large" /></div>;
  }

  if (error && !status && heartbeats.length === 0) {
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
            placeholder="Buscar..."
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
          <Button type="primary" icon={<PlusOutlined />} onClick={openCreateModal}>Añadir</Button>
        </Space>
      </div>

      {/* Unified stats row */}
      <Row gutter={[16, 16]} style={{ marginBottom: 16 }}>
        <Col xs={12} sm={12} lg={6}>
          <Card>
            <Statistic
              title="Total"
              value={totalItems}
              prefix={<RocketOutlined style={{ color: '#1677ff' }} />}
            />
          </Card>
        </Col>
        <Col xs={12} sm={12} lg={6}>
          <Card>
            <Statistic
              title="En línea"
              value={healthyItems}
              prefix={<CheckCircleOutlined style={{ color: '#22c55e' }} />}
              valueStyle={{ color: '#22c55e' }}
            />
          </Card>
        </Col>
        <Col xs={12} sm={12} lg={6}>
          <Card>
            <Statistic
              title="Con problemas"
              value={problemItems}
              prefix={<CloseCircleOutlined style={{ color: '#ef4444' }} />}
              valueStyle={{ color: '#ef4444' }}
            />
          </Card>
        </Col>
        <Col xs={12} sm={12} lg={6}>
          <Card>
            <Statistic
              title="Latencia media"
              value={status?.avg_response_time_24h ?? 0}
              suffix="ms"
              prefix={<FieldTimeOutlined style={{ color: '#a78bfa' }} />}
            />
          </Card>
        </Col>
      </Row>

      {/* Scheduler info */}
      {scheduler.last_run_at && (
        <div style={{ marginBottom: 16, display: 'flex', gap: 16, flexWrap: 'wrap' }}>
          <Typography.Text type="secondary" style={{ fontSize: 12 }}>
            Última ejecución: {dayjs(scheduler.last_run_at).fromNow()}
          </Typography.Text>
          <Typography.Text type="secondary" style={{ fontSize: 12 }}>
            Próxima ejecución: {scheduler.next_run_at ? dayjs(scheduler.next_run_at).fromNow() : '—'}
          </Typography.Text>
          <Typography.Text type="secondary" style={{ fontSize: 12 }}>
            Monitores verificados: {scheduler.last_monitors_checked}
          </Typography.Text>
        </div>
      )}

      {/* Loading overlay */}
      {loading && (
        <div style={{ textAlign: 'center', padding: 16 }}>
          <Spin />
        </div>
      )}

      {/* Unified grid — monitors + heartbeats mixed */}
      {items.length > 0 ? (
        <>
          <Row gutter={[16, 16]}>
            {items.map(item => (
              <Col xs={24} sm={12} lg={8} xl={6} key={`${item.kind}-${item.id}`}>
                <MonitorCard
                  item={item}
                  onEdit={handleEdit}
                  onDelete={handleDelete}
                  onRefresh={() => {
                    loadMonitors();
                    loadHeartbeats();
                  }}
                />
              </Col>
            ))}
          </Row>

          {/* Pagination — only for monitors */}
          {total > 0 && monitors.length > 0 && (
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
        </>
      ) : (
        !loading && (
          <Card style={{ marginTop: 16 }}>
            <Typography.Text type="secondary">
              {debouncedSearch || typeFilter || statusFilter
                ? 'No hay elementos que coincidan con los filtros'
                : 'No hay elementos configurados. Crea tu primer monitor o heartbeat.'}
            </Typography.Text>
          </Card>
        )
      )}

      {/* ── Unified Create/Edit Modal ── */}
      <Modal
        title={editingItem ? 'Editar' : 'Nuevo'}
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
          {/* Common fields */}
          <Form.Item name="name" label="Nombre" rules={[{ required: true }]}>
            <Input placeholder="Ej: Mi monitor" />
          </Form.Item>
          <Form.Item name="type" label="Tipo" rules={[{ required: true }]}>
            <Select options={ALL_TYPES} />
          </Form.Item>

          {/* Heartbeat-specific fields */}
          {isHeartbeatSelected && (
            <>
              <Form.Item name="grace_seconds" label="Grace period (segundos)" rules={[{ required: true }]}>
                <InputNumber min={60} max={2592000} style={{ width: '100%' }} />
              </Form.Item>
              <Form.Item name="notifier_id" label="Notificador para alertas">
                <Select allowClear placeholder="Ninguno" options={notifiers.map(n => ({ value: n.id, label: n.name }))} />
              </Form.Item>
              {editingItem?.kind === 'heartbeat' && (
                <div style={{ marginTop: 8, marginBottom: 16 }}>
                  <Typography.Text strong>URL de pulso:</Typography.Text>
                  <Input
                    value={`${window.location.origin}/api/heartbeat/${editingItem.token}`}
                    readOnly
                    style={{ marginTop: 4 }}
                  />
                </div>
              )}
            </>
          )}

          {/* Monitor-specific fields */}
          {!isHeartbeatSelected && (
            <Tabs>
              <Tabs.TabPane tab="General" key="general">
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
          )}
        </Form>
      </Modal>
    </div>
  );
}