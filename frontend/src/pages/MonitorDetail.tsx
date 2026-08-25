import { useEffect, useState } from 'react';
import { useParams } from 'react-router';
import { Card, Typography, Spin, Table, Tag, Button, Descriptions } from 'antd';
import { ReloadOutlined, PlayCircleOutlined } from '@ant-design/icons';
import { fetchMonitors, fetchChecks, fetchTimeline, runCheck, type Monitor, type CheckResult, type TimelinePoint } from '../api/http';
import dayjs from 'dayjs';
import relativeTime from 'dayjs/plugin/relativeTime';
import 'dayjs/locale/es';

dayjs.extend(relativeTime);
dayjs.locale('es');

const { Title } = Typography;

const STATUS_TAG: Record<string, { color: string; text: string }> = {
  up: { color: 'green', text: 'UP' },
  down: { color: 'red', text: 'DOWN' },
  error: { color: 'orange', text: 'ERROR' },
};

export default function MonitorDetail() {
  const { id } = useParams();
  const [monitor, setMonitor] = useState<Monitor | null>(null);
  const [checks, setChecks] = useState<CheckResult[]>([]);
  const [timeline, setTimeline] = useState<TimelinePoint[]>([]);
  const [loading, setLoading] = useState(true);

  const load = async () => {
    if (!id) return;
    setLoading(true);
    try {
      const { monitors } = await fetchMonitors();
      const m = monitors.find(m => m.id === id) ?? null;
      setMonitor(m);

      const { checks: c } = await fetchChecks(id, 100);
      setChecks(c);

      const { timeline: t } = await fetchTimeline(id, 7);
      setTimeline(t);
    } catch { /* ignore */ }
    setLoading(false);
  };

  useEffect(() => { load(); }, [id]);

  const handleCheck = async () => {
    if (!id) return;
    try {
      const result = await runCheck(id);
      load();
    } catch { /* ignore */ }
  };

  if (loading) return <div style={{ textAlign: 'center', padding: 40 }}><Spin size="large" /></div>;
  if (!monitor) return <Typography.Text type="danger">Monitor no encontrado</Typography.Text>;

  const uptime7d = timeline.length > 0
    ? Math.round((timeline.filter(t => t.status === 'up').length / timeline.length) * 100)
    : null;

  const checksColumns = [
    { title: 'Estado', dataIndex: 'status', key: 'status',
      render: (s: string) => <Tag color={STATUS_TAG[s]?.color}>{STATUS_TAG[s]?.text ?? s}</Tag>,
    },
    { title: 'Código', dataIndex: 'status_code', key: 'code', width: 80,
      render: (v: number | null) => v ?? '—',
    },
    { title: 'Latencia', dataIndex: 'response_time_ms', key: 'latency', width: 100,
      render: (v: number) => `${v} ms`,
    },
    { title: 'Error', dataIndex: 'error_message', key: 'error', ellipsis: true,
      render: (v: string | null) => v ?? '—',
    },
    { title: 'Fecha', dataIndex: 'checked_at', key: 'date', width: 160,
      render: (v: string) => dayjs(v).format('DD/MM/YYYY HH:mm:ss'),
    },
  ];

  return (
    <div className="fade-in-up">
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <Title level={3}>{monitor.name}</Title>
        <Button icon={<PlayCircleOutlined />} onClick={handleCheck}>Check ahora</Button>
      </div>

      <Descriptions column={3} style={{ marginTop: 16 }}>
        <Descriptions.Item label="Tipo">{monitor.type}</Descriptions.Item>
        <Descriptions.Item label="Target">{monitor.target}</Descriptions.Item>
        <Descriptions.Item label="Estado">
          <Tag color={checks[0] ? STATUS_TAG[checks[0].status]?.color : 'default'}>
            {checks[0] ? STATUS_TAG[checks[0].status]?.text ?? '—' : 'Sin datos'}
          </Tag>
        </Descriptions.Item>
        <Descriptions.Item label="Intervalo">{monitor.interval_seconds}s</Descriptions.Item>
        <Descriptions.Item label="Timeout">{monitor.timeout_seconds}s</Descriptions.Item>
        <Descriptions.Item label="Uptime 7d">{uptime7d !== null ? `${uptime7d}%` : '—'}</Descriptions.Item>
        {monitor.config_json && Object.keys(monitor.config_json).length > 0 && (
          <Descriptions.Item label="Config">
            <Typography.Text code>{JSON.stringify(monitor.config_json)}</Typography.Text>
          </Descriptions.Item>
        )}
      </Descriptions>

      {timeline.length > 0 && (
        <Card title="Timeline (7 días)" style={{ marginTop: 16 }}>
          <div style={{ display: 'flex', gap: 2, flexWrap: 'wrap' }}>
            {timeline.map((t, i) => (
              <div
                key={i}
                title={`${t.status} · ${t.response_time_ms}ms · ${dayjs(t.checked_at).format('DD/MM HH:mm')}`}
                style={{
                  width: 12,
                  height: 24,
                  background: t.status === 'up' ? '#22c55e' : t.status === 'down' ? '#ef4444' : '#f59e0b',
                  borderRadius: 2,
                }}
              />
            ))}
          </div>
        </Card>
      )}

      {timeline.filter(t => t.response_time_ms && t.response_time_ms > 0).length > 0 && (
        <Card title="Latencia (ms)" style={{ marginTop: 16 }}>
          <div style={{ display: 'flex', gap: 2, flexWrap: 'wrap', alignItems: 'flex-end', height: 80 }}>
            {(() => {
              const withRt = timeline.filter(t => t.response_time_ms && t.response_time_ms > 0);
              const maxRt = Math.max(...withRt.map(t => t.response_time_ms ?? 0), 1);
              const last24h = withRt.slice(-48);
              return last24h.length > 0 ? last24h.map((t, i) => {
                const rt = t.response_time_ms ?? 0;
                const pct = (rt / maxRt) * 100;
                return (
                  <div
                    key={i}
                    title={`${rt}ms · ${dayjs(t.checked_at).format('DD/MM HH:mm')}`}
                    style={{
                      width: '100%', flex: '1 1 auto', minWidth: 4,
                      height: `${Math.max(pct, 5)}%`,
                      background: '#1677ff',
                      borderRadius: '2px 2px 0 0',
                      opacity: t.status === 'up' ? 1 : 0.5,
                    }}
                  />
                );
              }) : null;
            })()}
          </div>
          <Typography.Text type="secondary" style={{ fontSize: 11 }}>
            Últimas 48 muestras · barra más alta = {Math.max(...timeline.filter(t => t.response_time_ms && t.response_time_ms > 0).map(t => t.response_time_ms ?? 0), 1)}ms
          </Typography.Text>
        </Card>
      )}

      <Card title="Histórico" style={{ marginTop: 16 }}>
        <Table dataSource={checks} columns={checksColumns} rowKey="id" pagination={{ pageSize: 20 }} size="small" />
      </Card>
    </div>
  );
}