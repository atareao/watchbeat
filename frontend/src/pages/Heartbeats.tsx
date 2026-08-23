import { useEffect, useState } from 'react';
import {
  Table, Button, Modal, Form, Input, InputNumber, Select, Typography, Space, message, Popconfirm, Tag,
} from 'antd';
import { PlusOutlined, ReloadOutlined, CopyOutlined } from '@ant-design/icons';
import {
  fetchHeartbeats, createHeartbeat, updateHeartbeat, deleteHeartbeat, fetchNotifiers,
  type Heartbeat,
} from '../api/http';

const { Title } = Typography;

export default function Heartbeats() {
  const [hbs, setHbs] = useState<Heartbeat[]>([]);
  const [notifiers, setNotifiers] = useState<{ id: string; name: string }[]>([]);
  const [loading, setLoading] = useState(true);
  const [modalOpen, setModalOpen] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [form] = Form.useForm();

  const load = () => {
    setLoading(true);
    Promise.all([fetchHeartbeats(), fetchNotifiers()])
      .then(([hData, nData]) => {
        setHbs(hData.heartbeats);
        setNotifiers(nData.notifiers.map(n => ({ id: n.id, name: n.name })));
      })
      .catch(err => message.error(err.message))
      .finally(() => setLoading(false));
  };

  useEffect(() => { load(); }, []);

  const handleCreate = () => {
    setEditingId(null);
    form.resetFields();
    form.setFieldsValue({ grace_seconds: 3600 });
    setModalOpen(true);
  };

  const handleEdit = (hb: Heartbeat) => {
    setEditingId(hb.id);
    form.setFieldsValue({
      name: hb.name,
      grace_seconds: hb.grace_seconds,
      notifier_id: hb.notifier_id,
    });
    setModalOpen(true);
  };

  const handleSave = async () => {
    try {
      const values = await form.validateFields();
      const payload = {
        name: values.name,
        grace_seconds: values.grace_seconds ?? 3600,
        notifier_id: values.notifier_id || null,
      };
      if (editingId) {
        await updateHeartbeat(editingId, payload);
        message.success('Heartbeat actualizado');
      } else {
        await createHeartbeat(payload);
        message.success('Heartbeat creado');
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
      await deleteHeartbeat(id);
      message.success('Heartbeat eliminado');
      load();
    } catch { message.error('Error al eliminar'); }
  };

  const statusColor: Record<string, string> = {
    ok: 'green', pending: 'gold', missing: 'red',
  };

  const columns = [
    { title: 'Nombre', dataIndex: 'name', key: 'name' },
    {
      title: 'Estado', dataIndex: 'status', key: 'status', width: 100,
      render: (s: string) => <Tag color={statusColor[s] || 'default'}>{s}</Tag>,
    },
    { title: 'Grace (s)', dataIndex: 'grace_seconds', key: 'grace_seconds', width: 100 },
    {
      title: 'Último pulso', dataIndex: 'last_seen_at', key: 'last_seen_at', width: 200,
      render: (v: string | null) => v ? new Date(v).toLocaleString() : '—',
    },
    {
      title: 'Token', key: 'token', width: 260,
      render: (_: unknown, r: Heartbeat) => (
        <Space>
          <code style={{ fontSize: 12 }}>{r.token?.substring(0, 16)}…</code>
          <Button size="small" icon={<CopyOutlined />} onClick={() => {
            navigator.clipboard.writeText(r.token);
            message.success('Token copiado');
          }} />
        </Space>
      ),
    },
    { title: 'Notificador', dataIndex: 'notifier_id', key: 'notifier_id', render: (v: string | null) => v ? notifiers.find(n => n.id === v)?.name ?? v : '—' },
    {
      title: 'Acciones', key: 'actions', width: 160,
      render: (_: unknown, r: Heartbeat) => (
        <Space>
          <Button size="small" onClick={() => handleEdit(r)}>Editar</Button>
          <Popconfirm title="¿Eliminar?" onConfirm={() => handleDelete(r.id)}>
            <Button size="small" danger>Eliminar</Button>
          </Popconfirm>
        </Space>
      ),
    },
  ];

  return (
    <div className="fade-in-up">
      <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 16 }}>
        <Title level={3} style={{ margin: 0 }}>Heartbeats</Title>
        <Space>
          <Button icon={<ReloadOutlined />} onClick={load}>Recargar</Button>
          <Button type="primary" icon={<PlusOutlined />} onClick={handleCreate}>Crear</Button>
        </Space>
      </div>

      <Table dataSource={hbs} columns={columns} rowKey="id" loading={loading} pagination={false} />

      <Modal
        title={editingId ? 'Editar heartbeat' : 'Nuevo heartbeat'}
        open={modalOpen}
        onOk={handleSave}
        onCancel={() => setModalOpen(false)}
        width={500}
      >
        <Form form={form} layout="vertical">
          <Form.Item name="name" label="Nombre" rules={[{ required: true }]}>
            <Input placeholder="Ej: backup-diario" />
          </Form.Item>
          <Form.Item name="grace_seconds" label="Grace period (segundos)" rules={[{ required: true }]}>
            <InputNumber min={60} max={2592000} style={{ width: '100%' }} />
          </Form.Item>
          <Form.Item name="notifier_id" label="Notificador para alertas">
            <Select allowClear placeholder="Ninguno" options={notifiers.map(n => ({ value: n.id, label: n.name }))} />
          </Form.Item>
        </Form>
        {editingId && (
          <div style={{ marginTop: 8 }}>
            <p><strong>URL de pulso:</strong></p>
            <Input value={`${window.location.origin}/api/heartbeat/${hbs.find(h => h.id === editingId)?.token ?? ''}`} readOnly />
          </div>
        )}
      </Modal>
    </div>
  );
}