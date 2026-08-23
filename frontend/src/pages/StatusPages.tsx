import { useEffect, useState } from 'react';
import {
  Table, Button, Modal, Form, Input, InputNumber, Select, Switch, Typography, Space, message, Popconfirm,
} from 'antd';
import { PlusOutlined, ReloadOutlined, LinkOutlined } from '@ant-design/icons';
import {
  fetchStatusPages, createStatusPage, updateStatusPage, deleteStatusPage, fetchMonitors,
  type StatusPage,
} from '../api/http';

const { Title, Text } = Typography;

export default function StatusPages() {
  const [pages, setPages] = useState<StatusPage[]>([]);
  const [monitors, setMonitors] = useState<{ id: string; name: string }[]>([]);
  const [loading, setLoading] = useState(true);
  const [modalOpen, setModalOpen] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [form] = Form.useForm();

  const load = () => {
    setLoading(true);
    Promise.all([
      fetchStatusPages(),
      fetchMonitors(),
    ])
      .then(([pData, mData]) => {
        setPages(pData.status_pages);
        setMonitors(mData.monitors.map(m => ({ id: m.id, name: m.name })));
      })
      .catch(err => message.error(err.message))
      .finally(() => setLoading(false));
  };

  useEffect(() => { load(); }, []);

  const handleCreate = () => {
    setEditingId(null);
    form.resetFields();
    form.setFieldsValue({ public: true, slug: '', description: '', monitors: [] });
    setModalOpen(true);
  };

  const handleEdit = (p: StatusPage) => {
    setEditingId(p.id);
    form.setFieldsValue({
      slug: p.slug,
      title: p.title,
      description: p.description ?? '',
      monitors: p.monitors,
      public: p.public,
    });
    setModalOpen(true);
  };

  const handleSave = async () => {
    try {
      const values = await form.validateFields();
      const payload = {
        slug: values.slug,
        title: values.title,
        description: values.description || null,
        monitors: values.monitors || [],
        public: values.public ?? true,
      };
      if (editingId) {
        await updateStatusPage(editingId, payload);
        message.success('Status page actualizada');
      } else {
        await createStatusPage(payload);
        message.success('Status page creada');
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
      await deleteStatusPage(id);
      message.success('Status page eliminada');
      load();
    } catch { message.error('Error al eliminar'); }
  };

  const columns = [
    { title: 'Título', dataIndex: 'title', key: 'title' },
    { title: 'Slug', dataIndex: 'slug', key: 'slug' },
    {
      title: 'Pública', dataIndex: 'public', key: 'public', width: 80,
      render: (pub: boolean) => pub ? '✅' : '❌',
    },
    { title: 'Monitores', key: 'monitors', render: (_: unknown, r: StatusPage) => r.monitors?.length ?? 0 },
    {
      title: 'URL pública', key: 'url', width: 300,
      render: (_: unknown, r: StatusPage) => r.public ? (
        <Text copyable={{ text: `${window.location.origin}/status/${r.slug}` }}>
          <LinkOutlined /> /status/{r.slug}
        </Text>
      ) : '—',
    },
    {
      title: 'Acciones', key: 'actions', width: 160,
      render: (_: unknown, r: StatusPage) => (
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
        <Title level={3} style={{ margin: 0 }}>Status Pages</Title>
        <Space>
          <Button icon={<ReloadOutlined />} onClick={load}>Recargar</Button>
          <Button type="primary" icon={<PlusOutlined />} onClick={handleCreate}>Crear</Button>
        </Space>
      </div>

      <Table dataSource={pages} columns={columns} rowKey="id" loading={loading} pagination={false} />

      <Modal
        title={editingId ? 'Editar status page' : 'Nueva status page'}
        open={modalOpen}
        onOk={handleSave}
        onCancel={() => setModalOpen(false)}
        width={600}
      >
        <Form form={form} layout="vertical">
          <Form.Item name="title" label="Título" rules={[{ required: true }]}>
            <Input />
          </Form.Item>
          <Form.Item name="slug" label="Slug" rules={[{ required: true, pattern: /^[a-z0-9-]+$/ }]}
            extra="solo minúsculas, números y guiones"
          >
            <Input />
          </Form.Item>
          <Form.Item name="description" label="Descripción">
            <Input.TextArea rows={2} />
          </Form.Item>
          <Form.Item name="monitors" label="Monitores">
            <Select mode="multiple" options={monitors.map(m => ({ value: m.id, label: m.name }))} placeholder="Selecciona monitores" />
          </Form.Item>
          <Form.Item name="public" label="Pública" valuePropName="checked">
            <Switch />
          </Form.Item>
        </Form>
      </Modal>
    </div>
  );
}