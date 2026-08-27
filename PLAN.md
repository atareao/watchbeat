# PLAN — Fusión Dashboard + Monitores (v0.8.0)

## Objetivo

Unificar las vistas Dashboard (`/dashboard`) y Monitores (`/monitors`) en una sola página principal que combine:
- Métricas globales (tarjetas de stats)
- Lista paginada de monitores en formato cards con acciones inline
- Búsqueda y filtros
- Auto-refresh cada 30s

La ruta `/monitors` desaparece. La página `MonitorDetail` (`/monitors/:id`) se mantiene intacta.

## Cambios en backend

### 1. Endpoint `GET /api/monitors` — ampliar con search + filters + summary data

**Nuevos query params:**
- `q` (opcional, string): búsqueda por nombre o target
- `type` (opcional, string): filtrar por tipo de monitor (http, tcp, ping, tls)
- `status` (opcional, string): filtrar por último estado (up, down, error)

**Nuevos campos en respuesta (por monitor):**
- `last_status: string | null`
- `last_response_time_ms: number | null`
- `last_checked_at: string | null`
- `uptime_7d: number | null`
- `uptime_30d: number | null`

**SQL:** LEFT JOIN con checks para obtener el último check de cada monitor. Calcular uptime en Rust post-query (solo para los monitores de la página actual, max 100).

### 2. Endpoint `GET /api/status` — simplificar

Mantener solo las métricas globales (total_monitors, up_monitors, down_monitors, avg_response_time_24h). Quitar la lista de `monitors` y `scheduler` del response (ya no se necesita, los monitores vienen del endpoint paginado).

## Cambios en frontend

### 1. `Dashboard.tsx` — reescritura completa

Página unificada con:

```
┌─── 🖥️ Dashboard ─────────────────────────── [🔄] [➕ Añadir] ─┐
│ 📊 18 monitores │ ✅ 12 UP │ ❌ 3 DOWN │ ⏱️ 234ms media       │
├─────────────────────────────────────────────────────────────────┤
│ 🔍 Buscar...    [Tipo: ▾]     [Estado: ▾]                      │
├─────────────────────────────────────────────────────────────────┤
│ ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│ │ atareao.es   │  │ blog         │  │ api          │          │
│ │ ✅ UP 45ms   │  │ ❌ DOWN err  │  │ ✅ UP 120ms  │          │
│ │ Uptime 99.8% │  │ Uptime 76.3% │  │ Uptime 98.2% │          │
│ │ Últ: 10:32   │  │ Últ: 10:27   │  │ Últ: 10:31   │          │
│ │ [▶] [🔘] [⋯] │  │ [▶] [🔘] [⋯] │  │ [▶] [🔘] [⋯] │          │
│ └──────────────┘  └──────────────┘  └──────────────┘          │
│                                                                  │
│ « 1-6 de 18  ▸ ❮ 1 2 3 ❯ »                                   │
└──────────────────────────────────────────────────────────────────┘
```

**Componentes:**
- **StatsRow**: 4 tarjetas de métricas globales (total, UP, DOWN, latencia media)
- **SearchBar**: input de búsqueda + selects de tipo y estado + botón crear + botón recargar
- **CardGrid**: grid de MonitorCard con acciones inline
- **Pagination**: paginación servidor-side

**MonitorCard** se modifica para incluir acciones:
- `▶` Run check (siempre visible)
- `🔘` Toggle enable/disable (siempre visible)
- `⋯` Menú con "Editar" y "Eliminar" (con Popconfirm para eliminar)

**Auto-refresh:** cada 30s, pero se pausa si hay un modal abierto o si el usuario está editando.

**Modal de creación/edición:** se reutiliza el mismo modal que estaba en Monitors.tsx con los 3 tabs (General, Específico, Plantillas).

### 2. `App.tsx` — eliminar ruta `/monitors`

- Ruta `/monitors` → eliminada
- Ruta index (`/`) → Dashboard
- Ruta `/dashboard` → Dashboard (mantener por compatibilidad)

### 3. `AppLayout.tsx` — eliminar "Monitores" del sidebar

- Quitar `{ key: '/monitors', icon: <MonitorOutlined />, label: 'Monitores' }`
- Renombrar "Dashboard" a algo más descriptivo o mantenerlo

### 4. `http.ts` — actualizar tipos

- Añadir `search`, `typeFilter`, `statusFilter` a `fetchMonitors()`
- Crear tipo `MonitorCardData` que extiende `Monitor` con los campos de summary

## Flujo de datos

1. Dashboard carga → `GET /api/monitors?page=1&per_page=20` con filtros → recibe monitores + stats globales
2. Usuario busca/filtra → mismo endpoint con query params → recarga la grid
3. Usuario hace clic en card → navega a `/monitors/:id`
4. Usuario hace check/toggle → llama al endpoint correspondiente → refresca la página actual
5. Auto-refresh cada 30s → repite la misma request con los mismos filtros/página

## Archivos a modificar

### Backend
- `backend/src/db.rs` — modificar `list_monitors_paginated` para search/filter + summary data
- `backend/src/routes/monitors.rs` — ampliar query params en `list` handler
- `backend/src/routes/status.rs` — simplificar response

### Frontend
- `frontend/src/pages/Dashboard.tsx` — reescritura completa
- `frontend/src/components/MonitorCard.tsx` — añadir acciones inline
- `frontend/src/App.tsx` — eliminar ruta `/monitors`
- `frontend/src/components/AppLayout.tsx` — actualizar sidebar
- `frontend/src/api/http.ts` — actualizar tipos y fetchMonitors

## Archivos a eliminar
- `frontend/src/pages/Monitors.tsx` — toda su funcionalidad se migra a Dashboard.tsx

## No tocar
- `frontend/src/pages/MonitorDetail.tsx` — se queda igual
- `backend/src/routes/checks.rs` — timeline y checks list se quedan igual
- Estructura de base de datos — no hay migraciones