import React from 'react';
import { Card, Tag, Typography, Space, Button, Dropdown, Modal, Popconfirm, message } from 'antd';
import {
  CheckCircleOutlined, CloseCircleOutlined, WarningOutlined,
  PlayCircleOutlined, MoreOutlined,
} from '@ant-design/icons';
import type { MonitorSummary } from '../api/http';
import { runCheck, toggleMonitor } from '../api/http';
import dayjs from 'dayjs';
import relativeTime from 'dayjs/plugin/relativeTime';
import 'dayjs/locale/es';

dayjs.extend(relativeTime);
dayjs.locale('es');

const STATUS_CONFIG: Record<string, { color: string; icon: React.ReactNode }> = {
  up: { color: '#22c55e', icon: <CheckCircleOutlined style={{ color: '#22c55e', fontSize: 24 }} /> },
  down: { color: '#ef4444', icon: <CloseCircleOutlined style={{ color: '#ef4444', fontSize: 24 }} /> },
  error: { color: '#f59e0b', icon: <WarningOutlined style={{ color: '#f59e0b', fontSize: 24 }} /> },
};

interface MonitorCardProps {
  monitor: MonitorSummary;
  onToggle?: () => void;
  onEdit?: (monitor: MonitorSummary) => void;
  onDelete?: (id: string) => void;
  onCheck?: () => void;
}

export default function MonitorCard({ monitor, onToggle, onEdit, onDelete, onCheck }: MonitorCardProps) {
  const cfg = monitor.last_status ? STATUS_CONFIG[monitor.last_status] ?? STATUS_CONFIG.error : STATUS_CONFIG.error;

  const handleCheck = async () => {
    try {
      const result = await runCheck(monitor.id);
      message.success(`Check: ${result.status} (${result.response_time_ms}ms)`);
      onCheck?.();
    } catch {
      message.error('Error al ejecutar check');
    }
  };

  const handleToggle = async () => {
    try {
      await toggleMonitor(monitor.id);
      message.success('Estado cambiado');
      onToggle?.();
    } catch {
      message.error('Error al cambiar estado');
    }
  };

  const handleDeleteClick = () => {
    Modal.confirm({
      title: '¿Eliminar monitor?',
      content: `¿Estás seguro de eliminar "${monitor.name}"?`,
      okText: 'Eliminar',
      okType: 'danger',
      cancelText: 'Cancelar',
      onOk: () => onDelete?.(monitor.id),
    });
  };

  const dropdownItems = [
    { key: 'edit', label: 'Editar', onClick: () => onEdit?.(monitor) },
    { key: 'delete', label: 'Eliminar', danger: true, onClick: handleDeleteClick },
  ];

  return (
    <Card
      className="monitor-card"
      style={{ borderLeft: `4px solid ${cfg.color}` }}
    >
      <Space align="start" style={{ justifyContent: 'space-between', width: '100%' }}>
        <div>
          <Typography.Text strong style={{ fontSize: 16 }}>{monitor.name}</Typography.Text>
          <div style={{ marginTop: 4 }}>
            <Tag>{monitor.monitor_type}</Tag>
            <Typography.Text code style={{ fontSize: 12 }}>{monitor.target}</Typography.Text>
          </div>
        </div>
        {cfg.icon}
      </Space>
      <div style={{ marginTop: 12, display: 'flex', justifyContent: 'space-between' }}>
        <div>
          <Typography.Text type="secondary">Latencia: </Typography.Text>
          <Typography.Text>{monitor.last_response_time_ms ? `${monitor.last_response_time_ms}ms` : '—'}</Typography.Text>
        </div>
        <div>
          <Typography.Text type="secondary">Uptime 7d: </Typography.Text>
          <Typography.Text>{monitor.uptime_7d !== null ? `${Math.round(monitor.uptime_7d)}%` : '—'}</Typography.Text>
        </div>
      </div>
      <div style={{ marginTop: 4, marginBottom: 8 }}>
        <Typography.Text type="secondary" style={{ fontSize: 12 }}>
          {monitor.last_checked_at ? dayjs(monitor.last_checked_at).fromNow() : 'Sin datos'}
        </Typography.Text>
      </div>
      <div style={{ borderTop: '1px solid #f0f0f0', paddingTop: 8, display: 'flex', justifyContent: 'space-between' }}>
        <Space>
          <Button size="small" icon={<PlayCircleOutlined />} onClick={handleCheck} />
          <Popconfirm title="¿Cambiar estado?" onConfirm={handleToggle}>
            <Button size="small">{monitor.enabled ? 'Desactivar' : 'Activar'}</Button>
          </Popconfirm>
        </Space>
        <Dropdown menu={{ items: dropdownItems }} trigger={['click']}>
          <Button size="small" icon={<MoreOutlined />} />
        </Dropdown>
      </div>
    </Card>
  );
}