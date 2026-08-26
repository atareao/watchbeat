import { useEffect, useState } from 'react';
import {
  Card, Typography, InputNumber, Input, Button, message, Space, Divider, Alert,
} from 'antd';
import { SettingOutlined, DownloadOutlined, FileTextOutlined, SaveOutlined, WarningOutlined } from '@ant-design/icons';
import { fetchStatus } from '../api/http';

const { Title, Text, Paragraph } = Typography;

export default function Settings() {
  const [retentionDays, setRetentionDays] = useState(30);
  const [saving, setSaving] = useState(false);
  const [backingUp, setBackingUp] = useState(false);
  const [status, setStatus] = useState<{ total_monitors: number } | null>(null);
  const [defaultTemplates, setDefaultTemplates] = useState({
    down: '',
    latency: '',
    up: '',
    expiry: '',
  });
  const [savingTemplates, setSavingTemplates] = useState(false);

  useEffect(() => {
    fetchStatus().then(s => setStatus({ total_monitors: s.status.total_monitors })).catch(() => {});
    // Load current retention
    fetch('/api/settings?key=retention_days')
      .then(r => r.json().catch(() => ({})))
      .then(d => {
        if (d.value) setRetentionDays(parseInt(d.value, 10));
      })
      .catch(() => {});
    // Load default templates
    Promise.all([
      fetch('/api/settings?key=default_template_down').then(r => r.json().catch(() => ({}))),
      fetch('/api/settings?key=default_template_latency').then(r => r.json().catch(() => ({}))),
      fetch('/api/settings?key=default_template_up').then(r => r.json().catch(() => ({}))),
      fetch('/api/settings?key=default_template_expiry').then(r => r.json().catch(() => ({}))),
    ]).then(([down, latency, up, expiry]) => {
      setDefaultTemplates({
        down: down.value || '',
        latency: latency.value || '',
        up: up.value || '',
        expiry: expiry.value || '',
      });
    }).catch(() => {});
  }, []);

  const saveRetention = async () => {
    setSaving(true);
    try {
      const res = await fetch('/api/settings', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ key: 'retention_days', value: String(retentionDays) }),
      });
      if (!res.ok) throw new Error('Error al guardar');
      message.success(`Retención guardada: ${retentionDays} días`);
    } catch (err: unknown) {
      message.error(err instanceof Error ? err.message : 'Error al guardar');
    } finally {
      setSaving(false);
    }
  };

  const doBackup = async () => {
    setBackingUp(true);
    try {
      const res = await fetch('/api/backup', { method: 'POST' });
      if (!res.ok) throw new Error('Error al hacer backup');
      const data = await res.json();
      message.success(`Backup creado: ${data.path}`);
    } catch (err: unknown) {
      message.error(err instanceof Error ? err.message : 'Error al hacer backup');
    } finally {
      setBackingUp(false);
    }
  };

  const saveDefaultTemplates = async () => {
    setSavingTemplates(true);
    try {
      const entries = [
        { key: 'default_template_down', value: defaultTemplates.down },
        { key: 'default_template_latency', value: defaultTemplates.latency },
        { key: 'default_template_up', value: defaultTemplates.up },
        { key: 'default_template_expiry', value: defaultTemplates.expiry },
      ];
      for (const entry of entries) {
        await fetch('/api/settings', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(entry),
        });
      }
      message.success('Plantillas por defecto guardadas');
    } catch (err: unknown) {
      message.error(err instanceof Error ? err.message : 'Error al guardar');
    } finally {
      setSavingTemplates(false);
    }
  };

  const getMonitorSlug = () => {
    const path = window.location.hash;
    const match = path.match(/\/monitors\/([^/]+)/);
    return match ? match[1] : null;
  };

  return (
    <div className="fade-in-up">
      <Title level={3}><SettingOutlined /> Ajustes</Title>

      <Card title="Retención de datos" style={{ marginBottom: 16 }}>
        <Paragraph>
          Los checks antiguos se eliminan automáticamente según la retención configurada.
          El cambio se aplica en el siguiente ciclo del scheduler (~15s).
        </Paragraph>
        <Space>
          <InputNumber
            min={1}
            max={365}
            value={retentionDays}
            onChange={(v) => setRetentionDays(v ?? 30)}
            addonAfter="días"
            style={{ width: 200 }}
          />
          <Button type="primary" icon={<SaveOutlined />} onClick={saveRetention} loading={saving}>
            Guardar
          </Button>
        </Space>
      </Card>

      <Card title="Plantillas por defecto" style={{ marginBottom: 16 }} id="templates">
        <Paragraph>
          Estas plantillas se usan cuando un monitor no tiene una plantilla personalizada.
          Déjalas en blanco para usar las plantillas internas por defecto.
        </Paragraph>
        <Space direction="vertical" style={{ width: '100%' }} size="middle">
          <div>
            <Text strong>DOWN</Text>
            <Input.TextArea
              rows={3}
              value={defaultTemplates.down}
              onChange={(e) => setDefaultTemplates(prev => ({ ...prev, down: e.target.value }))}
              placeholder={'🔴 {{ monitor_name }} — {{ target }}\nStatus: {{ status }}\nResponse: {{ response_time_ms }}ms\n{{ error_message }}'}
            />
          </div>
          <div>
            <Text strong>LATENCIA</Text>
            <Input.TextArea
              rows={3}
              value={defaultTemplates.latency}
              onChange={(e) => setDefaultTemplates(prev => ({ ...prev, latency: e.target.value }))}
              placeholder={'🟡 {{ monitor_name }} — {{ target }}\nHigh latency: {{ response_time_ms }}ms (threshold: {{ latency_threshold_ms }}ms)'}
            />
          </div>
          <div>
            <Text strong>UP (recuperación)</Text>
            <Input.TextArea
              rows={3}
              value={defaultTemplates.up}
              onChange={(e) => setDefaultTemplates(prev => ({ ...prev, up: e.target.value }))}
              placeholder={'🟢 {{ monitor_name }} — {{ target }}\nRecovered — Response: {{ response_time_ms }}ms'}
            />
          </div>
          <div>
            <Text strong>EXPIRACIÓN TLS</Text>
            <Input.TextArea
              rows={3}
              value={defaultTemplates.expiry}
              onChange={(e) => setDefaultTemplates(prev => ({ ...prev, expiry: e.target.value }))}
              placeholder={'🟡 {{ monitor_name }} — {{ target }}\nCertificate expires in {{ days_left }} days'}
            />
          </div>
          <Button type="primary" icon={<SaveOutlined />} onClick={saveDefaultTemplates} loading={savingTemplates}>
            Guardar plantillas por defecto
          </Button>
        </Space>
      </Card>

      <Card title="Backup manual" style={{ marginBottom: 16 }}>
        <Paragraph>
          Crea una copia del archivo SQLite en el directorio de datos.
          El backup incluye un checkpoint WAL para consistencia.
        </Paragraph>
        <Button icon={<WarningOutlined />} onClick={doBackup} loading={backingUp}>
          Crear backup ahora
        </Button>
      </Card>

      <Card title="Exportar histórico" style={{ marginBottom: 16 }}>
        <Paragraph>
          Descarga el histórico de checks en formato CSV o JSON para un monitor específico.
          Selecciona el monitor desde la página de monitores y usa los enlaces de exportación.
        </Paragraph>
        {status ? (
          <Space direction="vertical" style={{ width: '100%' }}>
            <Alert
              type="info"
              message="Exportar desde un monitor"
              description="Ve a Monitores, abre el detalle de un monitor, y en la URL añade /export/csv o /export/json al final."
              showIcon
            />
            <Space>
              <Button icon={<DownloadOutlined />} onClick={() => {
                const mid = getMonitorSlug();
                if (mid) {
                  window.open(`/api/monitors/${mid}/export/csv`, '_blank');
                } else {
                  message.warning('Navega a un monitor primero');
                }
              }}>
                Exportar CSV (monitor actual)
              </Button>
              <Button icon={<FileTextOutlined />} onClick={() => {
                const mid = getMonitorSlug();
                if (mid) {
                  window.open(`/api/monitors/${mid}/export/json`, '_blank');
                } else {
                  message.warning('Navega a un monitor primero');
                }
              }}>
                Exportar JSON (monitor actual)
              </Button>
            </Space>
          </Space>
        ) : (
          <Text type="secondary">Cargando...</Text>
        )}
      </Card>

      <Card title="Variables de entorno">
        <Paragraph>
          La configuración principal (OIDC, puerto, etc.) se define mediante
          variables de entorno. Consulta <code>watchbeat.env.example</code> para la lista completa.
        </Paragraph>
        <table style={{ width: '100%', borderCollapse: 'collapse' }}>
          <thead>
            <tr style={{ borderBottom: '1px solid #eee' }}>
              <th style={{ padding: 8, textAlign: 'left' }}>Variable</th>
              <th style={{ padding: 8, textAlign: 'left' }}>Descripción</th>
            </tr>
          </thead>
          <tbody>
            <tr><td style={{ padding: 8 }}>HOST</td><td style={{ padding: 8 }}>Host de escucha (0.0.0.0)</td></tr>
            <tr><td style={{ padding: 8 }}>PORT</td><td style={{ padding: 8 }}>Puerto (3055)</td></tr>
            <tr><td style={{ padding: 8 }}>DATA_DIR</td><td style={{ padding: 8 }}>Directorio de datos</td></tr>
            <tr><td style={{ padding: 8 }}>DATABASE_URL</td><td style={{ padding: 8 }}>Ruta a SQLite</td></tr>
            <tr><td style={{ padding: 8 }}>OIDC_ISSUER_URL</td><td style={{ padding: 8 }}>URL del issuer OIDC</td></tr>
            <tr><td style={{ padding: 8 }}>OIDC_CLIENT_ID</td><td style={{ padding: 8 }}>Client ID OIDC</td></tr>
            <tr><td style={{ padding: 8 }}>OIDC_CLIENT_SECRET</td><td style={{ padding: 8 }}>Client Secret OIDC</td></tr>
            <tr><td style={{ padding: 8 }}>TIMEZONE</td><td style={{ padding: 8 }}>Zona horaria (Europe/Madrid)</td></tr>
            <tr><td style={{ padding: 8 }}>RUST_LOG</td><td style={{ padding: 8 }}>Nivel de log (info)</td></tr>
          </tbody>
        </table>
      </Card>
    </div>
  );
}