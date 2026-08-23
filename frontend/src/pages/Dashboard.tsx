import { useEffect, useState } from 'react';
import { Card, Col, Row, Statistic, Typography, Spin, Tag } from 'antd';
import {
  RocketOutlined, CheckCircleOutlined, CloseCircleOutlined,
  ClockCircleOutlined, FieldTimeOutlined, DashboardOutlined,
} from '@ant-design/icons';
import { fetchStatus, type DashboardStatus, type MonitorSummary } from '../api/http';
import MonitorCard from '../components/MonitorCard';
import dayjs from 'dayjs';
import relativeTime from 'dayjs/plugin/relativeTime';
import 'dayjs/locale/es';

dayjs.extend(relativeTime);
dayjs.locale('es');

const { Title } = Typography;

export default function Dashboard() {
  const [status, setStatus] = useState<DashboardStatus | null>(null);
  const [monitors, setMonitors] = useState<MonitorSummary[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    const load = () =>
      fetchStatus()
        .then(data => {
          setStatus(data.status);
          setMonitors(data.monitors);
        })
        .catch(console.error)
        .finally(() => setLoading(false));
    load();
    const interval = setInterval(load, 30_000);
    return () => clearInterval(interval);
  }, []);

  if (loading) return <div style={{ textAlign: 'center', padding: 40 }}><Spin size="large" /></div>;
  if (!status) return <div style={{ textAlign: 'center', padding: 40 }}><Typography.Text type="danger">Error al cargar</Typography.Text></div>;

  return (
    <div className="fade-in-up">
      <Title level={3}><DashboardOutlined /> Dashboard</Title>

      <Row gutter={[16, 16]}>
        <Col xs={24} sm={12} lg={6}>
          <Card><Statistic title="Monitores" value={status.total_monitors} prefix={<RocketOutlined style={{ color: '#1677ff' }} />} /></Card>
        </Col>
        <Col xs={24} sm={12} lg={6}>
          <Card><Statistic title="UP" value={status.up_monitors} prefix={<CheckCircleOutlined style={{ color: '#22c55e' }} />} suffix={`/ ${status.enabled_monitors}`} /></Card>
        </Col>
        <Col xs={24} sm={12} lg={6}>
          <Card><Statistic title="DOWN" value={status.down_monitors} prefix={<CloseCircleOutlined style={{ color: '#ef4444' }} />} /></Card>
        </Col>
        <Col xs={24} sm={12} lg={6}>
          <Card><Statistic title="Latencia media" value={status.avg_response_time_24h ?? 0} suffix="ms" prefix={<FieldTimeOutlined style={{ color: '#a78bfa' }} />} /></Card>
        </Col>
      </Row>

      <Row gutter={[16, 16]} style={{ marginTop: 16 }}>
        {monitors.map(m => (
          <Col xs={24} sm={12} lg={8} key={m.id}>
            <MonitorCard monitor={m} />
          </Col>
        ))}
      </Row>

      {monitors.length === 0 && (
        <Card style={{ marginTop: 16 }}>
          <Typography.Text type="secondary">
            No hay monitores configurados. Crea tu primer monitor en la sección Monitores.
          </Typography.Text>
        </Card>
      )}
    </div>
  );
}