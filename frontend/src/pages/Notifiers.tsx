import { useEffect, useState } from 'react';
import {
  Table, Button, Modal, Form, Input, Select, Switch, Typography, Space, message, Popconfirm,
} from 'antd';
import { PlusOutlined, ReloadOutlined } from '@ant-design/icons';
import {
  fetchNotifiers, createNotifier, updateNotifier, deleteNotifier, testNotifier,
  type Notifier,
} from '../api/http';

const { Title } = Typography;

const NOTIFIER_TYPES = [
  { value: 'telegram', label: 'Telegram' },
];

export default function Notifiers() {
  const [notifiers, setNotifiers] = useState<Notifier[]>([]);
  const [loading, setLoading] = useState(true);
  const [modalOpen, setModalOpen] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
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
    form.resetFields();
    form.setFieldsValue({ type: 'telegram', enabled: true });
    setModalOpen(true);
  };

  const handleEdit = (n: Notifier) => {
    setEditingId(n.id);
    form.setFieldsValue({
      name: n.name,
      type: n.type,
      bot_token: n.config_json?.bot_token ?? '',
      chat_id: n.config_json?.chat_id ?? '',
      enabled: n.enabled,
    });
    setModalOpen(true);
  };

  const handleSave = async () => {
    try {
      const values = await form.validateFields();
      const payload = {
        name: values.name,
        type: values.type,
        config: { bot_token: values.bot_token, chat_id: values.chat_id },
        enabled: values.enabled,
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
    { title: 'Chat ID', key: 'chat_id',
      render: (_: unknown, r: Notifier) => r.config_json?.chat_id ?? '—',
    },
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
      >
        <Form form={form} layout="vertical">
          <Form.Item name="name" label="Nombre" rules={[{ required: true }]}>
            <Input />
          </Form.Item>
          <Form.Item name="type" label="Tipo">
            <Select options={NOTIFIER_TYPES} disabled />
          </Form.Item>
          <Form.Item name="bot_token" label="Bot Token" rules={[{ required: true }]}>
            <Input.Password />
          </Form.Item>
          <Form.Item name="chat_id" label="Chat ID" rules={[{ required: true }]}>
            <Input />
          </Form.Item>
          <Form.Item name="enabled" label="Habilitado" valuePropName="checked">
            <Switch />
          </Form.Item>
        </Form>
      </Modal>
    </div>
  );
}