import React from "react";
import ReactDOM from "react-dom/client";
import { BrowserRouter } from "react-router";
import { ConfigProvider, theme as antTheme } from "antd";
import App from "./App";
import { useTheme } from "./hooks/useTheme";
import "./global.css";

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
        <App />
      </BrowserRouter>
    </ConfigProvider>
  );
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <ThemedApp />
  </React.StrictMode>
);