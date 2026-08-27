import { useEffect, useState } from 'react';
import {
  Card, Typography, InputNumber, Input, Button, message, Space, Tabs, Table, Modal, Form, Select, Switch, Popconfirm,
} from 'antd';
import {
  SettingOutlined, DownloadOutlined, FileTextOutlined, SaveOutlined, WarningOutlined,
  BellOutlined, ControlOutlined, PlusOutlined, ReloadOutlined, LinkOutlined,
} from '@ant-design/icons';
import {
  fetchStatus, fetchSetting, saveSetting, createBackup,
  fetchNotifiers, createNotifier, updateNotifier, deleteNotifier, testNotifier,
  fetchStatusPages, createStatusPage, updateStatusPage, deleteStatusPage, fetchMonitors,
  type Notifier, type StatusPage,
} from '../api/http';

const { Title, Text, Paragraph } = Typography;

// ─── Notifier config ───

const NOTIFIER_TYPES = [
  { value: 'telegram', label: 'Telegram' },
  { value: 'matrix', label: 'Matrix' },
  { value: 'ntfy', label: 'ntfy' },
  { value: 'webhook', label: 'Webhook' },
  { value: 'slack', label: 'Slack' },
  { value: 'discord', label: 'Discord' },
  { value: 'email', label: 'Email (SMTP)' },
  { value: 'gotify', label: 'Gotify' },
];

type ConfigField = {
  name: string;
  label: string;
  type: 'text' | 'password' | 'number';
  required: boolean;
  defaultValue?: string | number;
};

const CONFIG_FIELDS: Record<string, ConfigField[]> = {
  telegram: [
    { name: 'bot_token', label: 'Bot Token', type: 'password', required: true },
    { name: 'chat_id', label: 'Chat ID', type: 'text', required: true },
  ],
  matrix: [
    { name: 'homeserver_url', label: 'Homeserver URL', type: 'text', required: true, defaultValue: 'https://matrix.example.com' },
    { name: 'access_token', label: 'Access Token', type: 'password', required: true },
    { name: 'room_id', label: 'Room ID', type: 'text', required: true },
  ],
  ntfy: [
    { name: 'topic', label: 'Topic', type: 'text', required: true },
    { name: 'server_url', label: 'Server URL', type: 'text', required: false, defaultValue: 'https://ntfy.sh' },
    { name: 'token', label: 'Token (opcional)', type: 'password', required: false },
  ],
  webhook: [
    { name: 'url', label: 'Webhook URL', type: 'text', required: true },
    { name: 'method', label: 'Método HTTP', type: 'text', required: false, defaultValue: 'POST' },
    { name: 'headers', label: 'Headers extra (JSON)', type: 'text', required: false },
  ],
  slack: [
    { name: 'webhook_url', label: 'Webhook URL', type: 'text', required: true },
  ],
  discord: [
    { name: 'webhook_url', label: 'Webhook URL', type: 'text', required: true },
  ],
  email: [
    { name: 'smtp_host', label: 'SMTP Host', type: 'text', required: true },
    { name: 'smtp_port', label: 'SMTP Port', type: 'number', required: false, defaultValue: 587 },
    { name: 'username', label: 'Usuario', type: 'text', required: true },
    { name: 'password', label: 'Contraseña', type: 'password', required: true },
    { name: 'from', label: 'From (email)', type: 'text', required: true },
    { name: 'to', label: 'To (email)', type: 'text', required: true },
  ],
  gotify: [
    { name: 'server_url', label: 'Server URL', type: 'text', required: false, defaultValue: 'http://localhost:8080' },
    { name: 'app_token', label: 'App Token', type: 'password', required: true },
    { name: 'priority', label: 'Prioridad', type: 'number', required: false, defaultValue: 5 },
  ],
};

export default function Settings() {
  // ── General state ──
  const [retentionDays, setRetentionDays] = useState(30);
  const [saving, setSaving] = useState(false);
  const [backingUp, setBackingUp] = useState(false);
  const [defaultTemplates, setDefaultTemplates] = useState({ down: '', latency: '', up: '', expiry: '' });
  const [savingTemplates, setSavingTemplates] = useState(false);

  // ── Notifier state ──
  const [notifiers, setNotifiers] = useState<Notifier[]>([]);
  const [notifLoading, setNotifLoading] = useState(true);
  const [notifModal, setNotifModal] = useState(false);
  const [editingNotifId, setEditingNotifId] = useState<string | null>(null);
  const [notifType, setNotifType] = useState('telegram');
  const [notifForm] = Form.useForm();

  // ── Status page state ──
  const [pages, setPages] = useState<StatusPage[]>([]);
  const [monitorOptions, setMonitorOptions] = useState<{ id: string; name: string }[]>([]);
  const [spLoading, setSpLoading] = useState(true);
  const [spModal, setSpModal] = useState(false);
  const [editingSpId, setEditingSpId] = useState<string | null>(null);
  const [spForm] = Form.useForm();

  // ── Init ──
  useEffect(() => {
    fetchSetting('retention_days')
      .then(d => { if (d.value) setRetentionDays(parseInt(d.value, 10)); })
      .catch(() => {});
    Promise.all([
      fetchSetting('default_template_down'),
      fetchSetting('default_template_latency'),
      fetchSetting('default_template_up'),
      fetchSetting('default_template_expiry'),
    ]).then(([down, latency, up, expiry]) => {
      setDefaultTemplates({ down: down.value || '', latency: latency.value || '', up: up.value || '', expiry: expiry.value || '' });
    }).catch(() => {});
  }, []);

  // ── Handlers: general ──
  const saveRetention = async () => {
    setSaving(true);
    try { await saveSetting('retention_days', String(retentionDays)); message.success('Retención guardada'); } catch (e: any) { message.error(e.message); } finally { setSaving(false); }
  };

  const doBackup = async () => {
    setBackingUp(true);
    try { const data = await createBackup(); message.success(`Backup: ${data.path}`); } catch (e: any) { message.error(e.message); } finally { setBackingUp(false); }
  };

  const saveDefaultTemplates = async () => {
    setSavingTemplates(true);
    try {
      await Promise.all([
        saveSetting('default_template_down', defaultTemplates.down),
        saveSetting('default_template_latency', defaultTemplates.latency),
        saveSetting('default_template_up', defaultTemplates.up),
        saveSetting('default_template_expiry', defaultTemplates.expiry),
      ]);
      message.success('Plantillas guardadas');
    } catch (e: any) { message.error(e.message); } finally { setSavingTemplates(false); }
  };

  // ── Handlers: notifiers ──
  const loadNotifiers = () => {
    setNotifLoading(true);
    fetchNotifiers().then(d => setNotifiers(d.notifiers)).catch(() => {}).finally(() => setNotifLoading(false));
  };
  useEffect(() => { loadNotifiers(); }, []);

  const openNotifCreate = () => {
    setEditingNotifId(null); setNotifType('telegram'); notifForm.resetFields(); notifForm.setFieldsValue({ type: 'telegram', enabled: true }); setNotifModal(true);
  };

  const openNotifEdit = (n: Notifier) => {
    setEditingNotifId(n.id); setNotifType(n.type);
    const fields = CONFIG_FIELDS[n.type] || [];
    const defaults: Record<string, unknown> = {};
    for (const f of fields) { if (f.defaultValue !== undefined) defaults[f.name] = f.defaultValue; }
    notifForm.setFieldsValue({ name: n.name, type: n.type, enabled: n.enabled, ...defaults, ...Object.fromEntries(Object.entries(n.config_json).map(([k, v]) => [k, typeof v === 'object' ? JSON.stringify(v) : v])) });
    setNotifModal(true);
  };

  const saveNotifier = async () => {
    try {
      const values = await notifForm.validateFields();
      const fields = CONFIG_FIELDS[values.type] || [];
      const config: Record<string, unknown> = {};
      for (const f of fields) { const val = values[f.name]; if (val !== undefined && val !== '') { config[f.name] = f.type === 'number' ? Number(val) : val; } }
      const payload = { name: values.name, type: values.type, config, enabled: values.enabled ?? true };
      if (editingNotifId) { await updateNotifier(editingNotifId, payload); message.success('Notificador actualizado'); } else { await createNotifier(payload); message.success('Notificador creado'); }
      setNotifModal(false); loadNotifiers();
    } catch (err: any) { if (err?.errorFields) return; message.error('Error al guardar'); }
  };

  const deleteNotifier = async (id: string) => { try { await deleteNotifier(id); message.success('Eliminado'); loadNotifiers(); } catch { message.error('Error'); } };
  const testNotifierAction = async (id: string) => { try { await testNotifier(id); message.success('Prueba enviada'); } catch { message.error('Error'); } };

  // ── Handlers: status pages ──
  const loadStatusPages = () => {
    setSpLoading(true);
    Promise.all([fetchStatusPages(), fetchMonitors()])
      .then(([pData, mData]) => { setPages(pData.status_pages); setMonitorOptions(mData.monitors.map(m => ({ id: m.id, name: m.name }))); })
      .catch(() => {}).finally(() => setSpLoading(false));
  };
  useEffect(() => { loadStatusPages(); }, []);

  const openSpCreate = () => { setEditingSpId(null); spForm.resetFields(); spForm.setFieldsValue({ public: true, slug: '', description: '', monitors: [] }); setSpModal(true); };
  const openSpEdit = (p: StatusPage) => { setEditingSpId(p.id); spForm.setFieldsValue({ slug: p.slug, title: p.title, description: p.description ?? '', monitors: p.monitors, public: p.public }); setSpModal(true); };

  const saveStatusPage = async () => {
    try {
      const values = await spForm.validateFields();
      const payload = { slug: values.slug, title: values.title, description: values.description || null, monitors: values.monitors || [], public: values.public ?? true };
      if (editingSpId) { await updateStatusPage(editingSpId, payload); message.success('Status page actualizada'); } else { await createStatusPage(payload); message.success('Status page creada'); }
      setSpModal(false); loadStatusPages();
    } catch (err: any) { if (err?.errorFields) return; message.error('Error al guardar'); }
  };

  const deleteStatusPage = async (id: string) => { try { await deleteStatusPage(id); message.success('Eliminada'); loadStatusPages(); } catch { message.error('Error'); } };

  // ── Tab items (single level) ──

  const tabItems = [
    {
      key: 'retention',
      label: 'Retención de datos',
      children: (
        <Card>
          <Paragraph>Los checks antiguos se eliminan automáticamente. El cambio se aplica en el siguiente ciclo del scheduler (~15s).</Paragraph>
          <Space>
            <InputNumber min={1} max={365} value={retentionDays} onChange={(v) => setRetentionDays(v ?? 30)} addonAfter="días" style={{ width: 200 }} />
            <Button type="primary" icon={<SaveOutlined />} onClick={saveRetention} loading={saving}>Guardar</Button>
          </Space>
        </Card>
      ),
    },
    {
      key: 'templates',
      label: 'Plantillas',
      children: (
        <Card>
          <Paragraph>Se usan cuando un monitor no tiene plantilla personalizada. Déjalas en blanco para usar las internas.</Paragraph>
          <Space direction="vertical" style={{ width: '100%' }} size="middle">
            <div><Text strong>DOWN</Text><Input.TextArea rows={2} value={defaultTemplates.down} onChange={(e) => setDefaultTemplates(p => ({ ...p, down: e.target.value }))} placeholder="🔴 {{ monitor_name }} — {{ target }}\nError: {{ error_message }}" /></div>
            <div><Text strong>LATENCIA</Text><Input.TextArea rows={2} value={defaultTemplates.latency} onChange={(e) => setDefaultTemplates(p => ({ ...p, latency: e.target.value }))} placeholder="🟡 {{ monitor_name }} — {{ response_time_ms }}ms" /></div>
            <div><Text strong>UP</Text><Input.TextArea rows={2} value={defaultTemplates.up} onChange={(e) => setDefaultTemplates(p => ({ ...p, up: e.target.value }))} placeholder="🟢 {{ monitor_name }} — {{ response_time_ms }}ms" /></div>
            <div><Text strong>EXPIRACIÓN</Text><Input.TextArea rows={2} value={defaultTemplates.expiry} onChange={(e) => setDefaultTemplates(p => ({ ...p, expiry: e.target.value }))} placeholder="🟡 {{ monitor_name }} — {{ days_left }} días" /></div>
            <Button type="primary" icon={<SaveOutlined />} onClick={saveDefaultTemplates} loading={savingTemplates}>Guardar plantillas</Button>
          </Space>
        </Card>
      ),
    },
    {
      key: 'backup',
      label: 'Backup',
      children: (
        <Card>
          <Paragraph>Crea una copia del archivo SQLite con checkpoint WAL.</Paragraph>
          <Button icon={<WarningOutlined />} onClick={doBackup} loading={backingUp}>Crear backup ahora</Button>
        </Card>
      ),
    },
    {
      key: 'export',
      label: 'Exportar',
      children: (
        <Card>
          <Paragraph>Descarga el histórico de checks en CSV o JSON (desde la vista de detalle de un monitor).</Paragraph>
        </Card>
      ),
    },
    {
      key: 'env',
      label: 'Variables de entorno',
      children: (
        <Card>
          <Paragraph>Configuración principal mediante variables. Consulta <code>watchbeat.env.example</code>.</Paragraph>
          <table style={{ width: '100%', borderCollapse: 'collapse' }}>
            <thead><tr style={{ borderBottom: '1px solid #eee' }}><th style={{ padding: 8, textAlign: 'left' }}>Variable</th><th style={{ padding: 8, textAlign: 'left' }}>Descripción</th></tr></thead>
            <tbody>
              {[['HOST', 'Host de escucha (0.0.0.0)'], ['PORT', 'Puerto (3055)'], ['DATA_DIR', 'Directorio de datos'], ['DATABASE_URL', 'Ruta a SQLite'], ['OIDC_ISSUER_URL', 'URL del issuer OIDC'], ['OIDC_CLIENT_ID', 'Client ID OIDC'], ['OIDC_CLIENT_SECRET', 'Client Secret OIDC'], ['TIMEZONE', 'Zona horaria (Europe/Madrid)'], ['RUST_LOG', 'Nivel de log (info)']].map(([v, d]) => (
                <tr key={v}><td style={{ padding: 8 }}>{v}</td><td style={{ padding: 8 }}>{d}</td></tr>
              ))}
            </tbody>
          </table>
        </Card>
      ),
    },
    {
      key: 'notifiers',
      label: <><BellOutlined /> Notificadores</>,
      children: (
        <Card>
          <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 16 }}>
            <Title level={4} style={{ margin: 0 }}>Notificadores</Title>
            <Space>
              <Button icon={<ReloadOutlined />} onClick={loadNotifiers}>Recargar</Button>
              <Button type="primary" icon={<PlusOutlined />} onClick={openNotifCreate}>Añadir</Button>
            </Space>
          </div>
          <Table dataSource={notifiers} columns={[
            { title: 'Nombre', dataIndex: 'name', key: 'name' },
            { title: 'Tipo', dataIndex: 'type', key: 'type', width: 100 },
            { title: 'Activo', dataIndex: 'enabled', key: 'enabled', width: 80, render: (e: boolean) => <Switch checked={e} disabled size="small" /> },
            { title: 'Acciones', key: 'actions', width: 220, render: (_: any, r: Notifier) => (
              <Space>
                <Button size="small" onClick={() => testNotifierAction(r.id)}>Probar</Button>
                <Button size="small" onClick={() => openNotifEdit(r)}>Editar</Button>
                <Popconfirm title="¿Eliminar?" onConfirm={() => deleteNotifier(r.id)}><Button size="small" danger>Eliminar</Button></Popconfirm>
              </Space>
            )},
          ]} rowKey="id" loading={notifLoading} pagination={false} />
        </Card>
      ),
    },
    {
      key: 'status-pages',
      label: <><ControlOutlined /> Status Pages</>,
      children: (
        <Card>
          <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 16 }}>
            <Title level={4} style={{ margin: 0 }}>Status Pages</Title>
            <Space>
              <Button icon={<ReloadOutlined />} onClick={loadStatusPages}>Recargar</Button>
              <Button type="primary" icon={<PlusOutlined />} onClick={openSpCreate}>Crear</Button>
            </Space>
          </div>
          <Table dataSource={pages} columns={[
            { title: 'Título', dataIndex: 'title', key: 'title' },
            { title: 'Slug', dataIndex: 'slug', key: 'slug' },
            { title: 'Pública', dataIndex: 'public', key: 'public', width: 80, render: (p: boolean) => p ? '✅' : '❌' },
            { title: 'Monitores', key: 'monitors', render: (_: any, r: StatusPage) => r.monitors?.length ?? 0 },
            { title: 'URL', key: 'url', width: 300, render: (_: any, r: StatusPage) => r.public ? <Text copyable={{ text: `${window.location.origin}/status/${r.slug}` }}><LinkOutlined /> /status/{r.slug}</Text> : '—' },
            { title: 'Acciones', key: 'actions', width: 160, render: (_: any, r: StatusPage) => (
              <Space>
                <Button size="small" onClick={() => openSpEdit(r)}>Editar</Button>
                <Popconfirm title="¿Eliminar?" onConfirm={() => deleteStatusPage(r.id)}><Button size="small" danger>Eliminar</Button></Popconfirm>
              </Space>
            )},
          ]} rowKey="id" loading={spLoading} pagination={false} />
        </Card>
      ),
    },
  ];

  const currentNotifFields = CONFIG_FIELDS[notifType] || [];

  return (
    <div className="fade-in-up">
      <Title level={3}><SettingOutlined /> Ajustes</Title>
      <Tabs items={tabItems} />

      {/* Notifier modal */}
      <Modal title={editingNotifId ? 'Editar notificador' : 'Nuevo notificador'} open={notifModal} onOk={saveNotifier} onCancel={() => setNotifModal(false)} width={550} destroyOnClose>
        <Form form={notifForm} layout="vertical">
          <Form.Item name="name" label="Nombre" rules={[{ required: true }]}><Input /></Form.Item>
          <Form.Item name="type" label="Tipo" rules={[{ required: true }]}>
            <Select options={NOTIFIER_TYPES} onChange={(val) => { setNotifType(val); const fields = CONFIG_FIELDS[val] || []; const d: Record<string, unknown> = { type: val, enabled: true }; for (const f of fields) { if (f.defaultValue !== undefined) d[f.name] = f.defaultValue; } notifForm.setFieldsValue(d); }} />
          </Form.Item>
          {currentNotifFields.map(f => (
            <Form.Item key={f.name} name={f.name} label={f.label} rules={f.required ? [{ required: true, message: `${f.label} es obligatorio` }] : []}>
              {f.type === 'password' ? <Input.Password /> : f.type === 'number' ? <InputNumber style={{ width: '100%' }} /> : <Input />}
            </Form.Item>
          ))}
          <Form.Item name="enabled" label="Habilitado" valuePropName="checked"><Switch /></Form.Item>
        </Form>
      </Modal>

      {/* Status page modal */}
      <Modal title={editingSpId ? 'Editar status page' : 'Nueva status page'} open={spModal} onOk={saveStatusPage} onCancel={() => setSpModal(false)} width={600} destroyOnClose>
        <Form form={spForm} layout="vertical">
          <Form.Item name="title" label="Título" rules={[{ required: true }]}><Input /></Form.Item>
          <Form.Item name="slug" label="Slug" rules={[{ required: true, pattern: /^[a-z0-9-]+$/ }]} extra="solo minúsculas, números y guiones"><Input /></Form.Item>
          <Form.Item name="description" label="Descripción"><Input.TextArea rows={2} /></Form.Item>
          <Form.Item name="monitors" label="Monitores"><Select mode="multiple" options={monitorOptions.map(m => ({ value: m.id, label: m.name }))} placeholder="Selecciona monitores" /></Form.Item>
          <Form.Item name="public" label="Pública" valuePropName="checked"><Switch /></Form.Item>
        </Form>
      </Modal>
    </div>
  );
}