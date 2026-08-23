import { useEffect, useState } from 'react';
import {
  Table, Button, Modal, Form, Input, InputNumber, Select, Switch, Typography, Space, Tag, message, Popconfirm,
} from 'antd';
import { PlusOutlined, ReloadOutlined, PlayCircleOutlined } from '@ant-design/icons';
import {
  fetchMonitors, createMonitor, updateMonitor, deleteMonitor, toggleMonitor, runCheck,
  type Monitor,
} from '../api/http';

const { Title } = Typography;

const MONITOR_TYPES = [
  { value: 'http', label: 'HTTP(S)' },
  { value: 'tcp', label: 'TCP' },
  { value: 'ping', label: 'Ping' },
];

const STATUS_COLORS: Record<string, string> = {
  up: '#22c55e',
  down: '#ef4444',
  error: '#f59e0b',
};

export default function Monitors() {
  const [monitors, setMonitors] = useState<Monitor[]>([]);
  const [loading, setLoading] = useState(true);
  const [modalOpen, setModalOpen] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [form] = Form.useForm();

  const load = () => {
    setLoading(true);
    fetchMonitors()
      .then(data => setMonitors(data.monitors))
      .catch(err => message.error(err.message))
      .finally(() => setLoading(false));
  };

  useEffect(() => { load(); }, []);

  const handleCreate = () => {
    setEditingId(null);
    form.resetFields();
    form.setFieldsValue({ type: 'http', interval_seconds: 300, timeout_seconds: 30, enabled: true });
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
    });
    setModalOpen(true);
  };

  const handleSave = async () => {
    try {
      const values = await form.validateFields();
      if (editingId) {
        await updateMonitor(editingId, values);
        message.success('Monitor actualizado');
      } else {
        await createMonitor(values);
        message.success('Monitor creado');
      }
      setModalOpen(false);
      load();
    } catch (err: unknown) {
      if (err && typeof err === 'object' && 'errorFields' in err) return; // validation error
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
    { title: 'Nombre', dataIndex: 'name', key: 'name',
      render: (name: string, record: Monitor) => (
        <a href={`#/monitors/${record.id}`}>{name}</a>
      ),
    },
    { title: 'Tipo', dataIndex: 'type', key: 'type', width: 80 },
    { title: 'Target', dataIndex: 'target', key: 'target', ellipsis: true },
    { title: 'Intervalo', dataIndex: 'interval_seconds', key: 'interval',
      render: (v: number) => `${v}s`,
    },
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
      >
        <Form form={form} layout="vertical">
          <Form.Item name="name" label="Nombre" rules={[{ required: true }]}>
            <Input />
          </Form.Item>
          <Form.Item name="type" label="Tipo" rules={[{ required: true }]}>
            <Select options={MONITOR_TYPES} />
          </Form.Item>
          <Form.Item name="target" label="Target" rules={[{ required: true }]}
            extra="URL, host:puerto, o IP para ping"
          >
            <Input placeholder="https://ejemplo.com" />
          </Form.Item>
          <Form.Item name="interval_seconds" label="Intervalo (segundos)">
            <InputNumber min={10} max={86400} style={{ width: '100%' }} />
          </Form.Item>
          <Form.Item name="timeout_seconds" label="Timeout (segundos)">
            <InputNumber min={1} max={120} style={{ width: '100%' }} />
          </Form.Item>
          <Form.Item name="enabled" label="Habilitado" valuePropName="checked">
            <Switch />
          </Form.Item>
        </Form>
      </Modal>
    </div>
  );
}