import { useState } from 'react';
import { Layout, Menu, Typography, Button } from 'antd';
import {
  DashboardOutlined, MonitorOutlined, BellOutlined, SettingOutlined, LogoutOutlined,
  ControlOutlined, HeartOutlined, SunOutlined, MoonOutlined,
} from '@ant-design/icons';
import { Outlet, useNavigate, useLocation } from 'react-router';
import { useAuth } from '../hooks/useAuth';
import { useTheme } from '../hooks/useTheme';

const { Header, Sider, Content } = Layout;

const menuItems = [
  { key: '/dashboard', icon: <DashboardOutlined />, label: 'Dashboard' },
  { key: '/monitors', icon: <MonitorOutlined />, label: 'Monitores' },
  { key: '/notifiers', icon: <BellOutlined />, label: 'Notificadores' },
  { key: '/status-pages', icon: <ControlOutlined />, label: 'Status Pages' },
  { key: '/heartbeats', icon: <HeartOutlined />, label: 'Heartbeats' },
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
      <Sider collapsible collapsed={collapsed} onCollapse={setCollapsed} theme="light" style={{ borderRight: '1px solid #f0f0f0' }}>
        <div style={{ padding: 16, textAlign: 'center', fontWeight: 'bold', fontSize: collapsed ? 14 : 18 }}>
          {collapsed ? '🕵️' : '🕵️ Vigilatrs'}
        </div>
        <Menu
          mode="inline"
          selectedKeys={[location.pathname]}
          items={menuItems}
          onClick={handleMenuClick}
        />
      </Sider>
      <Layout>
        <Header style={{ background: '#fff', padding: '0 24px', display: 'flex', justifyContent: 'flex-end', alignItems: 'center', borderBottom: '1px solid #f0f0f0' }}>
          <Typography.Text style={{ marginRight: 16 }}>{user?.name ?? user?.email ?? ''}</Typography.Text>
          <Button icon={isDark ? <SunOutlined /> : <MoonOutlined />} onClick={toggle} type="text" style={{ marginRight: 8 }} />
          <Button icon={<LogoutOutlined />} type="text">Salir</Button>
        </Header>
        <Content style={{ margin: 24 }}>
          <Outlet />
        </Content>
      </Layout>
    </Layout>
  );
}