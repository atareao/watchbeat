import { useEffect, useState } from 'react';
import { Link } from 'react-router';
import {
  Table, Button, Modal, Form, Input, InputNumber, Select, Switch, Tabs, Typography, Space, Tag, message, Popconfirm,
} from 'antd';
import { PlusOutlined, ReloadOutlined, PlayCircleOutlined } from '@ant-design/icons';
import {
  fetchMonitors, createMonitor, updateMonitor, deleteMonitor, toggleMonitor, runCheck,
  fetchNotifiers, type Monitor,
} from '../api/http';

const { Title } = Typography;

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
  ],
  tls: [
    { name: 'expiry_days', label: 'Días para expiry', type: 'number', defaultValue: 14 },
  ],
  tcp: [],
  ping: [],
};

export default function Monitors() {
  const [monitors, setMonitors] = useState<Monitor[]>([]);
  const [notifiers, setNotifiers] = useState<{ id: string; name: string }[]>([]);
  const [loading, setLoading] = useState(true);
  const [modalOpen, setModalOpen] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [selectedType, setSelectedType] = useState<string>('http');
  const [form] = Form.useForm();

  const load = () => {
    setLoading(true);
    Promise.all([fetchMonitors(), fetchNotifiers()])
      .then(([mData, nData]) => {
        setMonitors(mData.monitors);
        setNotifiers(nData.notifiers.map(n => ({ id: n.id, name: n.name })));
      })
      .catch(err => message.error(err.message))
      .finally(() => setLoading(false));
  };

  useEffect(() => { load(); }, []);

  const handleCreate = () => {
    setEditingId(null);
    form.resetFields();
    form.setFieldsValue({ type: 'http', interval_seconds: 300, timeout_seconds: 30, enabled: true, confirmations_required: 0, config: {} });
    setSelectedType('http');
    setModalOpen(true);
  };

  const handleEdit = (m: Monitor) => {
    setEditingId(m.id);
    form.setFieldsValue({
      name: m.name,
      type: m.type,
      target: m.target,
      interval_seconds: m.interval_seconds,
      timeout_seconds: m.timeout_seconds,
      enabled: m.enabled,
      notifier_id: m.notifier_id ?? null,
      confirmations_required: (m as any).confirmations_required ?? 0,
      config: m.config_json ?? {},
    });
    setSelectedType(m.type);
    setModalOpen(true);
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
        config_json: values.config ?? {},
      };
      if (editingId) {
        await updateMonitor(editingId, payload);
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

  const handleToggle = async (id: string) => {
    try {
      await toggleMonitor(id);
      load();
    } catch { message.error('Error al cambiar estado'); }
  };

  const handleCheck = async (id: string) => {
    try {
      const result = await runCheck(id);
      message.success(`Check: ${result.status} (${result.response_time_ms}ms)`);
      load();
    } catch { message.error('Error al ejecutar check'); }
  };

  const columns = [
    {
      title: 'Nombre', dataIndex: 'name', key: 'name',
      render: (name: string, record: Monitor) => (
        <Link to={`/monitors/${record.id}`}>{name}</Link>
      ),
    },
    { title: 'Tipo', dataIndex: 'type', key: 'type', width: 80 },
    { title: 'Target', dataIndex: 'target', key: 'target', ellipsis: true },
    { title: 'Intervalo', dataIndex: 'interval_seconds', key: 'interval', render: (v: number) => `${v}s` },
    {
      title: 'Activo', dataIndex: 'enabled', key: 'enabled', width: 80,
      render: (enabled: boolean, record: Monitor) => (
        <Switch checked={enabled} onChange={() => handleToggle(record.id)} size="small" />
      ),
    },
    {
      title: 'Acciones', key: 'actions', width: 200,
      render: (_: unknown, record: Monitor) => (
        <Space>
          <Button size="small" icon={<PlayCircleOutlined />} onClick={() => handleCheck(record.id)} />
          <Button size="small" onClick={() => handleEdit(record)}>Editar</Button>
          <Popconfirm title="¿Eliminar?" onConfirm={() => handleDelete(record.id)}>
            <Button size="small" danger>Eliminar</Button>
          </Popconfirm>
        </Space>
      ),
    },
  ];

  return (
    <div className="fade-in-up">
      <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 16 }}>
        <Title level={3} style={{ margin: 0 }}>Monitores</Title>
        <Space>
          <Button icon={<ReloadOutlined />} onClick={load}>Recargar</Button>
          <Button type="primary" icon={<PlusOutlined />} onClick={handleCreate}>Añadir</Button>
        </Space>
      </div>

      <Table dataSource={monitors} columns={columns} rowKey="id" loading={loading} pagination={false} />

      <Modal
        title={editingId ? 'Editar monitor' : 'Nuevo monitor'}
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
          </Tabs>
        </Form>
      </Modal>
    </div>
  );
}