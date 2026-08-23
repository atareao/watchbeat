import { useEffect, useState } from 'react';
import {
  Card, Typography, InputNumber, Button, message, Space, Divider, Alert,
} from 'antd';
import { SettingOutlined, DownloadOutlined, FileTextOutlined, SaveOutlined, WarningOutlined } from '@ant-design/icons';
import { fetchStatus } from '../api/http';

const { Title, Text, Paragraph } = Typography;

export default function Settings() {
  const [retentionDays, setRetentionDays] = useState(30);
  const [saving, setSaving] = useState(false);
  const [backingUp, setBackingUp] = useState(false);
  const [status, setStatus] = useState<{ total_monitors: number } | null>(null);

  useEffect(() => {
    fetchStatus().then(s => setStatus({ total_monitors: s.status.total_monitors })).catch(() => {});
    // Load current retention
    fetch('/api/settings?key=retention_days')
      .then(r => r.json().catch(() => ({})))
      .then(d => {
        if (d.value) setRetentionDays(parseInt(d.value, 10));
      })
      .catch(() => {});
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
          variables de entorno. Consulta <code>vigilatrs.env.example</code> para la lista completa.
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