import React from 'react';
import { Card, Tag, Typography, Space, Button, Dropdown, Modal, Popconfirm, message } from 'antd';
import {
  CheckCircleOutlined, CloseCircleOutlined, WarningOutlined,
  PlayCircleOutlined, MoreOutlined, HeartOutlined, ClockCircleOutlined, CopyOutlined,
} from '@ant-design/icons';
import type { DashboardItem } from '../api/http';
import { runCheck, toggleMonitor } from '../api/http';
import { useNavigate } from 'react-router';
import dayjs from 'dayjs';
import relativeTime from 'dayjs/plugin/relativeTime';
import 'dayjs/locale/es';

dayjs.extend(relativeTime);
dayjs.locale('es');

const STATUS_CONFIG: Record<string, { color: string; icon: React.ReactNode }> = {
  up: { color: '#22c55e', icon: <CheckCircleOutlined style={{ color: '#22c55e', fontSize: 24 }} /> },
  down: { color: '#ef4444', icon: <CloseCircleOutlined style={{ color: '#ef4444', fontSize: 24 }} /> },
  error: { color: '#f59e0b', icon: <WarningOutlined style={{ color: '#f59e0b', fontSize: 24 }} /> },
  ok: { color: '#22c55e', icon: <CheckCircleOutlined style={{ color: '#22c55e', fontSize: 24 }} /> },
  missing: { color: '#ef4444', icon: <CloseCircleOutlined style={{ color: '#ef4444', fontSize: 24 }} /> },
  pending: { color: '#f59e0b', icon: <ClockCircleOutlined style={{ color: '#f59e0b', fontSize: 24 }} /> },
};

interface MonitorCardProps {
  item: DashboardItem;
  onEdit: (item: DashboardItem) => void;
  onDelete: (id: string) => void;
  onRefresh: () => void;
}

export default function MonitorCard({ item, onEdit, onDelete, onRefresh }: MonitorCardProps) {
  if (item.kind === 'heartbeat') {
    return <HeartbeatView item={item} onEdit={onEdit} onDelete={onDelete} onRefresh={onRefresh} />;
  }
  return <MonitorView item={item} onEdit={onEdit} onDelete={onDelete} onRefresh={onRefresh} />;
}

// ── Heartbeat card ──

function HeartbeatView({ item, onEdit, onDelete, onRefresh }: MonitorCardProps & { item: DashboardItem & { kind: 'heartbeat' } }) {
  const cfg = STATUS_CONFIG[item.status] ?? STATUS_CONFIG.pending;
  const pulseUrl = `${window.location.origin}/api/heartbeat/${item.token}`;

  const handleCopyToken = () => {
    navigator.clipboard.writeText(item.token);
    message.success('Token copiado');
  };

  const handleCopyUrl = () => {
    navigator.clipboard.writeText(pulseUrl);
    message.success('URL de pulso copiada');
  };

  const handleDelete = () => {
    Modal.confirm({
      title: '¿Eliminar heartbeat?',
      content: `Se eliminará "${item.name}" permanentemente.`,
      okText: 'Eliminar',
      okType: 'danger',
      cancelText: 'Cancelar',
      onOk: async () => {
        try {
          const { deleteHeartbeat } = await import('../api/http');
          await deleteHeartbeat(item.id);
          message.success('Heartbeat eliminado');
          onRefresh();
        } catch {
          message.error('Error al eliminar heartbeat');
        }
      },
    });
  };

  const statusLabel = item.status === 'ok' ? 'OK' : item.status === 'missing' ? 'Perdido' : 'Pendiente';

  const dropdownItems = [
    { key: 'edit', label: 'Editar', onClick: (e: any) => { e?.stopPropagation(); onEdit(item); } },
    { key: 'copy-token', icon: <CopyOutlined />, label: 'Copiar token', onClick: (e: any) => { e?.stopPropagation(); handleCopyToken(); } },
    { key: 'copy-url', icon: <CopyOutlined />, label: 'Copiar URL de pulso', onClick: (e: any) => { e?.stopPropagation(); handleCopyUrl(); } },
    { type: 'divider' as const },
    { key: 'delete', label: 'Eliminar', danger: true, onClick: (e: any) => { e?.stopPropagation(); handleDelete(); } },
  ];

  return (
    <Card className="monitor-card" hoverable onClick={() => onEdit(item)} style={{ borderLeft: `4px solid ${cfg.color}` }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start' }}>
        <div>
          <Space>
            <HeartOutlined style={{ color: '#ec4899' }} />
            <Typography.Text strong style={{ fontSize: 16 }}>{item.name}</Typography.Text>
          </Space>
          <div style={{ marginTop: 4 }}>
            <Tag color={cfg.color}>{statusLabel}</Tag>
            <Typography.Text code style={{ fontSize: 12 }}>Grace: {item.grace_seconds}s</Typography.Text>
          </div>
        </div>
        {cfg.icon}
      </div>
      <div style={{ marginTop: 8 }}>
        <Typography.Text type="secondary" style={{ fontSize: 12 }}>
          {item.last_seen_at ? `Último pulso: ${dayjs(item.last_seen_at).fromNow()}` : 'Sin pulsos recibidos'}
        </Typography.Text>
      </div>
      <div style={{ borderTop: '1px solid #f0f0f0', paddingTop: 8, display: 'flex', justifyContent: 'flex-end' }}>
        <Dropdown menu={{ items: dropdownItems }} trigger={['click']}>
          <Button size="small" icon={<MoreOutlined />} onClick={e => e.stopPropagation()} />
        </Dropdown>
      </div>
    </Card>
  );
}

// ── Monitor card ──

function MonitorView({ item, onEdit, onDelete, onRefresh }: MonitorCardProps & { item: DashboardItem & { kind: 'monitor' } }) {
  const navigate = useNavigate();
  const cfg = item.last_status ? STATUS_CONFIG[item.last_status] ?? STATUS_CONFIG.error : STATUS_CONFIG.error;

  const handleCheck = async (e: React.MouseEvent) => {
    e.stopPropagation();
    try {
      const result = await runCheck(item.id);
      message.success(`Check: ${result.status} (${result.response_time_ms}ms)`);
      onRefresh();
    } catch {
      message.error('Error al ejecutar check');
    }
  };

  const handleToggle = async () => {
    try {
      await toggleMonitor(item.id);
      message.success('Estado cambiado');
      onRefresh();
    } catch {
      message.error('Error al cambiar estado');
    }
  };

  const handleDeleteClick = (e: any) => {
    e?.stopPropagation();
    Modal.confirm({
      title: '¿Eliminar monitor?',
      content: `¿Estás seguro de eliminar "${item.name}"?`,
      okText: 'Eliminar',
      okType: 'danger',
      cancelText: 'Cancelar',
      onOk: () => onDelete(item.id),
    });
  };

  const dropdownItems = [
    { key: 'edit', label: 'Editar', onClick: (e: any) => { e?.stopPropagation(); onEdit(item); } },
    { key: 'delete', label: 'Eliminar', danger: true, onClick: (e: any) => { e?.stopPropagation(); handleDeleteClick(e); } },
  ];

  return (
    <Card className="monitor-card" hoverable onClick={() => navigate(`/monitors/${item.id}`)} style={{ borderLeft: `4px solid ${cfg.color}` }}>
      <Space align="start" style={{ justifyContent: 'space-between', width: '100%' }}>
        <div>
          <Typography.Text strong style={{ fontSize: 16 }}>{item.name}</Typography.Text>
          <div style={{ marginTop: 4 }}>
            <Tag>{item.monitor_type}</Tag>
            <Typography.Text code style={{ fontSize: 12 }}>{item.target}</Typography.Text>
          </div>
        </div>
        {cfg.icon}
      </Space>
      <div style={{ marginTop: 12, display: 'flex', justifyContent: 'space-between' }}>
        <div>
          <Typography.Text type="secondary">Latencia: </Typography.Text>
          <Typography.Text>{item.last_response_time_ms ? `${item.last_response_time_ms}ms` : '—'}</Typography.Text>
        </div>
        <div>
          <Typography.Text type="secondary">Uptime 7d: </Typography.Text>
          <Typography.Text>{item.uptime_7d !== null ? `${Math.round(item.uptime_7d)}%` : '—'}</Typography.Text>
        </div>
      </div>
      <div style={{ marginTop: 4, marginBottom: 8 }}>
        <Typography.Text type="secondary" style={{ fontSize: 12 }}>
          {item.last_checked_at ? dayjs(item.last_checked_at).fromNow() : 'Sin datos'}
        </Typography.Text>
      </div>
      <div style={{ borderTop: '1px solid #f0f0f0', paddingTop: 8, display: 'flex', justifyContent: 'space-between' }}>
        <Space>
          <Button size="small" icon={<PlayCircleOutlined />} onClick={e => handleCheck(e)} />
          <Popconfirm title="¿Cambiar estado?" onConfirm={e => { if (e) (e as any).stopPropagation?.(); handleToggle(); }}>
            <Button size="small" onClick={e => e.stopPropagation()}>{item.enabled ? 'Desactivar' : 'Activar'}</Button>
          </Popconfirm>
        </Space>
        <Dropdown menu={{ items: dropdownItems }} trigger={['click']}>
          <Button size="small" icon={<MoreOutlined />} onClick={e => e.stopPropagation()} />
        </Dropdown>
      </div>
    </Card>
  );
}