import { useEffect, useState } from 'react';
import { Button, Result, theme, Space, Typography } from 'antd';
import { SunOutlined, MoonOutlined } from '@ant-design/icons';
import { useTheme } from '../hooks/useTheme';

export default function LoginPage() {
  const [error, setError] = useState<string | null>(null);
  const { token } = theme.useToken();
  const { isDark, toggle } = useTheme();

  const handleLogin = () => {
    window.location.href = '/auth/login';
  };

  useEffect(() => {
    const params = new URLSearchParams(window.location.search);
    if (params.get('error')) {
      setError('Error de autenticación. Intenta de nuevo.');
    }
  }, []);

  return (
    <div
      style={{
        display: 'flex',
        justifyContent: 'center',
        alignItems: 'center',
        minHeight: '100vh',
        background: token.colorBgLayout,
        position: 'relative',
      }}
    >
      {/* Theme toggle — top right */}
      <div style={{ position: 'absolute', top: 16, right: 16 }}>
        <Button
          icon={isDark ? <SunOutlined /> : <MoonOutlined />}
          onClick={toggle}
          type="text"
          size="large"
        />
      </div>

      <Result
        icon={
          <img
            src="/icon-192.png"
            alt="WatchBeat"
            style={{ width: 96, height: 96 }}
          />
        }
        title={
          <Typography.Title level={2} style={{ margin: 0, color: token.colorText }}>
            WatchBeat
          </Typography.Title>
        }
        subTitle={
          <Typography.Text style={{ color: token.colorTextSecondary, fontSize: 16 }}>
            Monitor de uptime auto-hosteado
          </Typography.Text>
        }
        extra={
          <Space direction="vertical" size="middle" style={{ width: '100%', alignItems: 'center' }}>
            {error && (
              <Typography.Text type="danger">{error}</Typography.Text>
            )}
            <Button type="primary" size="large" onClick={handleLogin} style={{ minWidth: 200 }}>
              {error ? 'Reintentar' : 'Iniciar sesión con OIDC'}
            </Button>
          </Space>
        }
        style={{ padding: 48 }}
      />
    </div>
  );
}