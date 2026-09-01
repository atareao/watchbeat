import { Layout, Typography, Button, Grid } from 'antd';
import {
  SettingOutlined, LogoutOutlined, UserOutlined,
  SunOutlined, MoonOutlined,
} from '@ant-design/icons';
import { Outlet, useNavigate, useLocation } from 'react-router';
import { useAuth } from '../hooks/useAuth';
import { useTheme } from '../hooks/useTheme';
import { clearToken } from '../store/auth';

const { Header, Content } = Layout;
const { useBreakpoint } = Grid;

export default function AppLayout() {
  const navigate = useNavigate();
  const location = useLocation();
  const { user } = useAuth();
  const { isDark, toggle } = useTheme();
  const screens = useBreakpoint();
  const isMobile = !screens.md;

  const isDashboard = location.pathname === '/dashboard' || location.pathname === '/'

  return (
    <Layout style={{ minHeight: '100vh' }}>
      <Header style={{
        background: isDark ? '#141414' : '#fff',
        padding: '0 24px',
        display: 'flex',
        justifyContent: 'space-between',
        alignItems: 'center',
        borderBottom: isDark ? '1px solid #303030' : '1px solid #f0f0f0',
        height: 56,
      }}>
        {/* Left: brand + navigation */}
        <div style={{ display: 'flex', alignItems: 'center', gap: 16 }}>
          <img
            src="/icon-32.png"
            alt="WatchBeat"
            style={{ height: 28, cursor: 'pointer' }}
            onClick={() => navigate('/dashboard')}
          />
          <Typography.Title level={4} style={{ margin: 0, cursor: 'pointer', display: isMobile ? 'none' : 'block' }} onClick={() => navigate('/dashboard')}>
            WatchBeat
          </Typography.Title>
          {isDashboard && (
            <Button
              type="text"
              icon={<SettingOutlined />}
              onClick={() => navigate('/settings')}
              size="large"
              style={{ fontSize: 16 }}
            >
              Ajustes
            </Button>
          )}
        </div>

        {/* Right: user + theme + logout */}
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          {isMobile ? (
            <Button icon={<UserOutlined />} type="text" />
          ) : (
            <Typography.Text style={{ marginRight: 8, fontSize: 13 }}>{user?.name ?? user?.email ?? ''}</Typography.Text>
          )}
          <Button icon={isDark ? <SunOutlined /> : <MoonOutlined />} onClick={toggle} type="text" />
          <Button icon={<LogoutOutlined />} type="text" onClick={() => { clearToken(); window.location.href = '/auth/logout'; }}>
            {isMobile ? '' : 'Salir'}
          </Button>
        </div>
      </Header>
      <Content style={{ margin: 24 }}>
        <Outlet />
      </Content>
    </Layout>
  );
}