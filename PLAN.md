# PLAN.md — WatchBeat

## Implementado

- ✅ Scheduler por monitor con timers tokio (MissedTickBehavior::Skip)
- ✅ SSE en tiempo real (EventSource + auto-reconnect)
- ✅ Prometheus eliminado
- ✅ SQLite optimizado (WAL synchronous=NORMAL, índice cubriente)
- ✅ Caches en memoria (checker, was_up, notifier_ids — 0 queries/check)
- ✅ Writes reducidos (solo cambios de estado + muestreo latencia cada 10º check)
- ✅ reqwest::Client global reutilizado
- ✅ Nullable fields corregidos (latency_threshold_ms, notifier_id, templates)
- ✅ Frontend: refresh silencioso sin spinner, latestCheck independiente

## Pendiente

*(nada por ahora)*