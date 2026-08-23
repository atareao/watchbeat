import React from 'react';
import ReactDOM from 'react-dom/client';
import { BrowserRouter, Routes, Route, Navigate } from 'react-router';
import { ConfigProvider, theme as antTheme } from 'antd';
import AppLayout from './components/AppLayout';
import Dashboard from './pages/Dashboard';
import LoginPage from './pages/LoginPage';
import Monitors from './pages/Monitors';
import MonitorDetail from './pages/MonitorDetail';
import Notifiers from './pages/Notifiers';
import StatusPages from './pages/StatusPages';
import Heartbeats from './pages/Heartbeats';
import Settings from './pages/Settings';
import { useAuth } from './hooks/useAuth';
import { useTheme } from './hooks/useTheme';
import './global.css';

const lightTheme = {
  algorithm: antTheme.defaultAlgorithm,
  token: { colorPrimary: '#1677ff', borderRadius: 6 },
};

const darkTheme = {
  algorithm: antTheme.darkAlgorithm,
  token: { colorPrimary: '#1677ff', borderRadius: 6 },
};

function ThemedApp() {
  const { isDark } = useTheme();

  return (
    <ConfigProvider theme={isDark ? darkTheme : lightTheme}>
      <BrowserRouter>
        <Routes>
          <Route path="/login" element={<LoginPage />} />
          <Route
            path="/"
            element={
              <ProtectedRoute>
                <AppLayout />
              </ProtectedRoute>
            }
          >
            <Route index element={<Root />} />
            <Route path="dashboard" element={<Dashboard />} />
            <Route path="monitors" element={<Monitors />} />
            <Route path="monitors/:id" element={<MonitorDetail />} />
            <Route path="notifiers" element={<Notifiers />} />
            <Route path="status-pages" element={<StatusPages />} />
            <Route path="heartbeats" element={<Heartbeats />} />
            <Route path="settings" element={<Settings />} />
          </Route>
        </Routes>
      </BrowserRouter>
    </ConfigProvider>
  );
}

function ProtectedRoute({ children }: { children: React.ReactNode }) {
  const { user, loading } = useAuth();
  if (loading) return null;
  if (!user) return <Navigate to="/login" replace />;
  return <>{children}</>;
}

function Root() {
  const { user } = useAuth();
  if (!user) return <Navigate to="/login" replace />;
  return <Navigate to="/dashboard" replace />;
}

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <ThemedApp />
  </React.StrictMode>,
);