import React from 'react';
import { Card, Tag, Typography, Space } from 'antd';
import { useNavigate } from 'react-router';
import { CheckCircleOutlined, CloseCircleOutlined, WarningOutlined } from '@ant-design/icons';
import type { MonitorSummary } from '../api/http';
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

export default function MonitorCard({ monitor }: { monitor: MonitorSummary }) {
  const navigate = useNavigate();
  const cfg = monitor.last_status ? STATUS_CONFIG[monitor.last_status] ?? STATUS_CONFIG.error : STATUS_CONFIG.error;

  return (
    <Card
      className="monitor-card"
      hoverable
      onClick={() => navigate(`/monitors/${monitor.id}`)}
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
      <div style={{ marginTop: 4 }}>
        <Typography.Text type="secondary" style={{ fontSize: 12 }}>
          {monitor.last_checked_at ? dayjs(monitor.last_checked_at).fromNow() : 'Sin datos'}
        </Typography.Text>
      </div>
    </Card>
  );
}