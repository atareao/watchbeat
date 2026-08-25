import { useEffect, useState } from 'react';
import {
  Table, Button, Modal, Form, Input, InputNumber, Select, Switch, Typography, Space, message, Popconfirm,
} from 'antd';
import { PlusOutlined, ReloadOutlined } from '@ant-design/icons';
import {
  fetchNotifiers, createNotifier, updateNotifier, deleteNotifier, testNotifier,
  type Notifier,
} from '../api/http';

const { Title } = Typography;

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

function configToFields(cfg: Record<string, unknown>): Record<string, unknown> {
  return Object.fromEntries(
    Object.entries(cfg).map(([k, v]) => [k, typeof v === 'object' ? JSON.stringify(v) : v])
  );
}

function fieldsToConfig(values: Record<string, unknown>, fields: ConfigField[]): Record<string, unknown> {
  const config: Record<string, unknown> = {};
  for (const f of fields) {
    const val = values[f.name];
    if (val !== undefined && val !== '') {
      if (f.type === 'number') {
        config[f.name] = Number(val);
      } else {
        config[f.name] = val;
      }
    }
  }
  return config;
}

export default function Notifiers() {
  const [notifiers, setNotifiers] = useState<Notifier[]>([]);
  const [loading, setLoading] = useState(true);
  const [modalOpen, setModalOpen] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [notifierType, setNotifierType] = useState('telegram');
  const [form] = Form.useForm();

  const load = () => {
    setLoading(true);
    fetchNotifiers()
      .then(data => setNotifiers(data.notifiers))
      .catch(err => message.error(err.message))
      .finally(() => setLoading(false));
  };

  useEffect(() => { load(); }, []);

  const handleCreate = () => {
    setEditingId(null);
    setNotifierType('telegram');
    form.resetFields();
    form.setFieldsValue({ type: 'telegram', enabled: true });
    setModalOpen(true);
  };

  const handleEdit = (n: Notifier) => {
    setEditingId(n.id);
    setNotifierType(n.type);
    const fields = CONFIG_FIELDS[n.type] || [];
    const defaults: Record<string, unknown> = {};
    for (const f of fields) {
      if (f.defaultValue !== undefined) defaults[f.name] = f.defaultValue;
    }
    form.setFieldsValue({
      name: n.name,
      type: n.type,
      enabled: n.enabled,
      ...defaults,
      ...configToFields(n.config_json),
    });
    setModalOpen(true);
  };

  const handleSave = async () => {
    try {
      const values = await form.validateFields();
      const fields = CONFIG_FIELDS[values.type] || [];
      const payload = {
        name: values.name,
        type: values.type,
        config: fieldsToConfig(values, fields),
        enabled: values.enabled ?? true,
      };
      if (editingId) {
        await updateNotifier(editingId, payload);
        message.success('Notificador actualizado');
      } else {
        await createNotifier(payload);
        message.success('Notificador creado');
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
      await deleteNotifier(id);
      message.success('Notificador eliminado');
      load();
    } catch { message.error('Error al eliminar'); }
  };

  const handleTest = async (id: string) => {
    try {
      await testNotifier(id);
      message.success('Notificación de prueba enviada');
    } catch { message.error('Error al enviar prueba'); }
  };

  const columns = [
    { title: 'Nombre', dataIndex: 'name', key: 'name' },
    { title: 'Tipo', dataIndex: 'type', key: 'type', width: 100 },
    {
      title: 'Activo', dataIndex: 'enabled', key: 'enabled', width: 80,
      render: (enabled: boolean) => <Switch checked={enabled} disabled size="small" />,
    },
    {
      title: 'Acciones', key: 'actions', width: 220,
      render: (_: unknown, r: Notifier) => (
        <Space>
          <Button size="small" onClick={() => handleTest(r.id)}>Probar</Button>
          <Button size="small" onClick={() => handleEdit(r)}>Editar</Button>
          <Popconfirm title="¿Eliminar?" onConfirm={() => handleDelete(r.id)}>
            <Button size="small" danger>Eliminar</Button>
          </Popconfirm>
        </Space>
      ),
    },
  ];

  const currentFields = CONFIG_FIELDS[notifierType] || [];

  return (
    <div className="fade-in-up">
      <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 16 }}>
        <Title level={3} style={{ margin: 0 }}>Notificadores</Title>
        <Space>
          <Button icon={<ReloadOutlined />} onClick={load}>Recargar</Button>
          <Button type="primary" icon={<PlusOutlined />} onClick={handleCreate}>Añadir</Button>
        </Space>
      </div>

      <Table dataSource={notifiers} columns={columns} rowKey="id" loading={loading} pagination={false} />

      <Modal
        title={editingId ? 'Editar notificador' : 'Nuevo notificador'}
        open={modalOpen}
        onOk={handleSave}
        onCancel={() => setModalOpen(false)}
        width={550}
      >
        <Form form={form} layout="vertical">
          <Form.Item name="name" label="Nombre" rules={[{ required: true }]}>
            <Input />
          </Form.Item>
          <Form.Item name="type" label="Tipo" rules={[{ required: true }]}>
            <Select
              options={NOTIFIER_TYPES}
              onChange={(val) => {
                setNotifierType(val);
                // Reset config fields when type changes
                const fields = CONFIG_FIELDS[val] || [];
                const defaults: Record<string, unknown> = { type: val, enabled: true };
                for (const f of fields) {
                  if (f.defaultValue !== undefined) defaults[f.name] = f.defaultValue;
                }
                form.setFieldsValue(defaults);
              }}
            />
          </Form.Item>
          {currentFields.map(f => (
            <Form.Item
              key={f.name}
              name={f.name}
              label={f.label}
              rules={f.required ? [{ required: true, message: `${f.label} es obligatorio` }] : []}
            >
              {f.type === 'password' ? <Input.Password /> :
               f.type === 'number' ? <InputNumber style={{ width: '100%' }} /> :
               <Input />}
            </Form.Item>
          ))}
          <Form.Item name="enabled" label="Habilitado" valuePropName="checked">
            <Switch />
          </Form.Item>
        </Form>
      </Modal>
    </div>
  );
}