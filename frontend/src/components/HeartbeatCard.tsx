import React from 'react';
import { Card, Tag, Typography, Space, Button, Popconfirm, Dropdown, message, Modal } from 'antd';
import {
  HeartOutlined, CheckCircleOutlined, CloseCircleOutlined,
  ClockCircleOutlined, CopyOutlined, MoreOutlined,
} from '@ant-design/icons';
import type { Heartbeat } from '../api/http';
import dayjs from 'dayjs';
import relativeTime from 'dayjs/plugin/relativeTime';
import 'dayjs/locale/es';

dayjs.extend(relativeTime);
dayjs.locale('es');

const STATUS_CONFIG: Record<string, { color: string; icon: React.ReactNode; label: string }> = {
  ok: { color: 'green', icon: <CheckCircleOutlined style={{ color: '#22c55e', fontSize: 20 }} />, label: 'OK' },
  pending: { color: 'gold', icon: <ClockCircleOutlined style={{ color: '#f59e0b', fontSize: 20 }} />, label: 'Pendiente' },
  missing: { color: 'red', icon: <CloseCircleOutlined style={{ color: '#ef4444', fontSize: 20 }} />, label: 'Perdido' },
};

interface HeartbeatCardProps {
  heartbeat: Heartbeat;
  onEdit: (hb: Heartbeat) => void;
  onDelete: (id: string) => void;
  onRefresh: () => void;
}

export default function HeartbeatCard({ heartbeat, onEdit, onDelete, onRefresh }: HeartbeatCardProps) {
  const cfg = STATUS_CONFIG[heartbeat.status] ?? STATUS_CONFIG.pending;
  const pulseUrl = `${window.location.origin}/api/heartbeat/${heartbeat.token}`;

  const handleCopyToken = () => {
    navigator.clipboard.writeText(heartbeat.token);
    message.success('Token copiado');
  };

  const handleCopyUrl = () => {
    navigator.clipboard.writeText(pulseUrl);
    message.success('URL de pulso copiada');
  };

  const handleDelete = () => {
    Modal.confirm({
      title: '¿Eliminar heartbeat?',
      content: `Se eliminará "${heartbeat.name}" permanentemente.`,
      okText: 'Eliminar',
      okType: 'danger',
      cancelText: 'Cancelar',
      onOk: async () => {
        try {
          const { deleteHeartbeat } = await import('../api/http');
          await deleteHeartbeat(heartbeat.id);
          message.success('Heartbeat eliminado');
          onRefresh();
        } catch {
          message.error('Error al eliminar heartbeat');
        }
      },
    });
  };

  const dropdownItems = [
    {
      key: 'edit',
      label: 'Editar',
      onClick: () => onEdit(heartbeat),
    },
    {
      key: 'copy-token',
      label: 'Copiar token',
      onClick: handleCopyToken,
    },
    {
      key: 'copy-url',
      label: 'Copiar URL de pulso',
      onClick: handleCopyUrl,
    },
    { type: 'divider' as const },
    {
      key: 'delete',
      label: 'Eliminar',
      danger: true,
      onClick: handleDelete,
    },
  ];

  return (
    <Card
      className="monitor-card"
      hoverable
      style={{ borderLeft: `4px solid ${cfg.color}` }}
    >
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start' }}>
        <div style={{ flex: 1, minWidth: 0 }}>
          <Space>
            <HeartOutlined style={{ color: '#ec4899' }} />
            <Typography.Text strong style={{ fontSize: 14 }} ellipsis>
              {heartbeat.name}
            </Typography.Text>
          </Space>
          <div style={{ marginTop: 6 }}>
            <Tag color={cfg.color} style={{ fontSize: 11 }}>{cfg.label}</Tag>
            <Typography.Text code style={{ fontSize: 11 }}>
              Grace: {heartbeat.grace_seconds}s
            </Typography.Text>
          </div>
        </div>
        <Dropdown menu={{ items: dropdownItems }} trigger={['click']}>
          <Button size="small" type="text" icon={<MoreOutlined />} />
        </Dropdown>
      </div>

      <div style={{ marginTop: 10, fontSize: 12, color: '#888' }}>
        <Typography.Text type="secondary" style={{ fontSize: 12 }}>
          {heartbeat.last_seen_at
            ? `Último pulso: ${dayjs(heartbeat.last_seen_at).fromNow()}`
            : 'Sin pulsos recibidos'}
        </Typography.Text>
      </div>
    </Card>
  );
}