import React from 'react';
import { Card, Tag, Typography, Space, Button, Dropdown, Modal, Popconfirm, message } from 'antd';
import {
  CheckCircleOutlined, CloseCircleOutlined, WarningOutlined,
  PlayCircleOutlined, MoreOutlined, HeartOutlined, ClockCircleOutlined, CopyOutlined,
  GlobalOutlined, ApiOutlined, WifiOutlined, SafetyOutlined,
} from '@ant-design/icons';
import type { MonitorSummary } from '../api/http';
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

const TYPE_ICONS: Record<string, React.ReactNode> = {
  http: <GlobalOutlined style={{ color: '#1677ff' }} />,
  tcp: <ApiOutlined style={{ color: '#8b5cf6' }} />,
  ping: <WifiOutlined style={{ color: '#06b6d4' }} />,
  tls: <SafetyOutlined style={{ color: '#f59e0b' }} />,
  heartbeat: <HeartOutlined style={{ color: '#ec4899' }} />,
};

interface MonitorCardProps {
  item: MonitorSummary;
  onEdit: (item: MonitorSummary) => void;
  onDelete: (id: string) => void;
  onRefresh: () => void;
}

export default function MonitorCard({ item, onEdit, onDelete, onRefresh }: MonitorCardProps) {
  const navigate = useNavigate();
  const isHeartbeat = item.monitor_type === 'heartbeat';

  // Heartbeat status uses last_seen_at + grace logic
  const hbStatus = isHeartbeat
    ? item.last_seen_at
      ? (() => {
          const elapsed = Date.now() - new Date(item.last_seen_at).getTime();
          return elapsed < (item.grace_seconds ?? 3600) * 1000 ? 'ok' : 'missing';
        })()
      : 'pending'
    : null;

  const cfg = isHeartbeat
    ? STATUS_CONFIG[hbStatus ?? 'pending'] ?? STATUS_CONFIG.pending
    : item.last_status
      ? STATUS_CONFIG[item.last_status] ?? STATUS_CONFIG.error
      : STATUS_CONFIG.error;

  const pulseUrl = isHeartbeat ? `${window.location.origin}/api/heartbeat/${item.token}` : '';
  const typeIcon = TYPE_ICONS[item.monitor_type] ?? null;

  // Card click: navigate to detail (monitors) or open edit modal (heartbeats)
  const handleCardClick = () => {
    if (isHeartbeat) {
      onEdit(item);
    } else {
      navigate(`/monitors/${item.id}`);
    }
  };

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

  const handleCopyToken = () => {
    navigator.clipboard.writeText(item.token ?? '');
    message.success('Token copiado');
  };

  const handleCopyUrl = () => {
    navigator.clipboard.writeText(pulseUrl);
    message.success('URL de pulso copiada');
  };

  const handleDeleteClick = () => {
    Modal.confirm({
      title: isHeartbeat ? '¿Eliminar heartbeat?' : '¿Eliminar monitor?',
      content: `¿Estás seguro de eliminar "${item.name}"?`,
      okText: 'Eliminar',
      okType: 'danger',
      cancelText: 'Cancelar',
      onOk: () => onDelete(item.id),
    });
  };

  // Dropdown menu items — the menu overlay is rendered in a portal (outside Card's DOM),
  // so clicks on menu items don't propagate to the Card. No stopPropagation needed.
  const dropdownItems: any[] = [
    { key: 'edit', label: 'Editar', onClick: () => onEdit(item) },
  ];

  if (isHeartbeat) {
    dropdownItems.push(
      { key: 'copy-token', icon: <CopyOutlined />, label: 'Copiar token', onClick: handleCopyToken },
      { key: 'copy-url', icon: <CopyOutlined />, label: 'Copiar URL de pulso', onClick: handleCopyUrl },
    );
  }

  dropdownItems.push(
    { type: 'divider' },
    { key: 'delete', label: 'Eliminar', danger: true, onClick: handleDeleteClick },
  );

  const statusLabel = isHeartbeat
    ? hbStatus === 'ok' ? 'OK' : hbStatus === 'missing' ? 'Perdido' : 'Pendiente'
    : item.last_status?.toUpperCase() ?? '—';

  return (
    <Card
      className="monitor-card"
      hoverable
      onClick={handleCardClick}
      style={{ borderLeft: `4px solid ${cfg.color}`, height: '100%', cursor: 'pointer' }}
      styles={{ body: { padding: 16, display: 'flex', flexDirection: 'column', height: '100%' } }}
    >
      {/* Header */}
      <Space align="start" style={{ justifyContent: 'space-between', width: '100%' }}>
        <div>
          <Space>
            {typeIcon}
            <Typography.Text strong style={{ fontSize: 16 }}>{item.name}</Typography.Text>
          </Space>
          <div style={{ marginTop: 4 }}>
            <Tag color={cfg.color}>{statusLabel}</Tag>
            {isHeartbeat ? (
              <Typography.Text code style={{ fontSize: 12 }}>Grace: {item.grace_seconds ?? 3600}s</Typography.Text>
            ) : (
              <>
                <Tag>{item.monitor_type}</Tag>
                <Typography.Text code style={{ fontSize: 12 }}>{item.target}</Typography.Text>
              </>
            )}
          </div>
        </div>
        {cfg.icon}
      </Space>

      {/* Body */}
      <div style={{ flex: 1, minHeight: 0 }}>
      {isHeartbeat ? (
        <div style={{ marginTop: 8 }}>
          <Typography.Text type="secondary" style={{ fontSize: 12 }}>
            {item.last_seen_at ? `Último pulso: ${dayjs(item.last_seen_at).fromNow()}` : 'Sin pulsos recibidos'}
          </Typography.Text>
        </div>
      ) : (
        <>
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
        </>
      )}

      </div>

      {/* Actions bar — stopPropagation in buttons prevents Card onClick from firing */}
      <div style={{ borderTop: '1px solid #f0f0f0', paddingTop: 8, display: 'flex', justifyContent: 'space-between' }}>
        <Space>
          {!isHeartbeat && (
            <Button size="small" icon={<PlayCircleOutlined />} onClick={handleCheck} />
          )}
          <Popconfirm
            title={item.enabled ? '¿Desactivar?' : '¿Activar?'}
            onConfirm={handleToggle}
          >
            <Button size="small" onClick={(e: React.MouseEvent) => e.stopPropagation()}>
              {item.enabled ? 'Desactivar' : 'Activar'}
            </Button>
          </Popconfirm>
          {isHeartbeat && (
            <Typography.Text type="secondary" style={{ fontSize: 11, marginLeft: 4 }}>
              {item.token ? `Token: ${item.token.slice(0, 8)}...` : ''}
            </Typography.Text>
          )}
        </Space>
        <Dropdown
          menu={{ items: dropdownItems }}
          trigger={['click']}
        >
          <Button size="small" icon={<MoreOutlined />} onClick={(e: React.MouseEvent) => e.stopPropagation()} />
        </Dropdown>
      </div>
    </Card>
  );
}