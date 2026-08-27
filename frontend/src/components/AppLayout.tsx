import { useState } from 'react';
import { Layout, Menu, Typography, Button } from 'antd';
import {
  DashboardOutlined, BellOutlined, SettingOutlined, LogoutOutlined,
  ControlOutlined, SunOutlined, MoonOutlined,
} from '@ant-design/icons';
import { Outlet, useNavigate, useLocation } from 'react-router';
import { useAuth } from '../hooks/useAuth';
import { useTheme } from '../hooks/useTheme';
import { clearToken } from '../store/auth';

const { Header, Sider, Content } = Layout;

const menuItems = [
  { key: '/dashboard', icon: <DashboardOutlined />, label: 'Dashboard' },
  { key: '/notifiers', icon: <BellOutlined />, label: 'Notificadores' },
  { key: '/status-pages', icon: <ControlOutlined />, label: 'Status Pages' },
  { key: '/settings', icon: <SettingOutlined />, label: 'Ajustes' },
];

export default function AppLayout() {
  const navigate = useNavigate();
  const location = useLocation();
  const { user } = useAuth();
  const { isDark, toggle } = useTheme();
  const [collapsed, setCollapsed] = useState(false);

  const handleMenuClick = ({ key }: { key: string }) => {
    navigate(key);
  };

  return (
    <Layout style={{ minHeight: '100vh' }}>
      <Sider collapsible collapsed={collapsed} onCollapse={setCollapsed} theme={isDark ? 'dark' : 'light'} style={{ borderRight: isDark ? '1px solid #303030' : '1px solid #f0f0f0' }}>
        <div style={{ padding: 16, textAlign: 'center', fontWeight: 'bold', fontSize: collapsed ? 14 : 18 }}>
          {collapsed ? '💓' : '💓 WatchBeat'}
        </div>
        <Menu
          mode="inline"
          selectedKeys={[location.pathname]}
          items={menuItems}
          onClick={handleMenuClick}
        />
      </Sider>
      <Layout>
        <Header style={{ background: isDark ? '#141414' : '#fff', padding: '0 24px', display: 'flex', justifyContent: 'flex-end', alignItems: 'center', borderBottom: isDark ? '1px solid #303030' : '1px solid #f0f0f0' }}>
          <Typography.Text style={{ marginRight: 16 }}>{user?.name ?? user?.email ?? ''}</Typography.Text>
          <Button icon={isDark ? <SunOutlined /> : <MoonOutlined />} onClick={toggle} type="text" style={{ marginRight: 8 }} />
          <Button icon={<LogoutOutlined />} type="text" onClick={() => { clearToken(); window.location.href = '/auth/logout'; }}>Salir</Button>
        </Header>
        <Content style={{ margin: 24 }}>
          <Outlet />
        </Content>
      </Layout>
    </Layout>
  );
}