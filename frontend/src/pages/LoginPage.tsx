import { useEffect, useState } from 'react';
import { Button, Result } from 'antd';

export default function LoginPage() {
  const [error, setError] = useState<string | null>(null);

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
    <div style={{
      display: 'flex',
      justifyContent: 'center',
      alignItems: 'center',
      minHeight: '100vh',
      background: '#f0f2f5',
    }}>
      <Result
        icon={<span style={{ fontSize: 48 }}>🕵️</span>}
        title="Vigilatrs"
        subTitle="Monitor de uptime auto-hosteado"
        extra={
          error ? (
            <div>
              <p style={{ color: '#ff4d4f' }}>{error}</p>
              <Button type="primary" size="large" onClick={handleLogin}>
                Reintentar
              </Button>
            </div>
          ) : (
            <Button type="primary" size="large" loading onClick={handleLogin}>
              Iniciar sesión
            </Button>
          )
        }
      />
    </div>
  );
}