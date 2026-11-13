# Retención: opciones 7 y 14 días

## Why

La retención por defecto de 30 días acumula 300k checks en 134 MB, causando queries lentas y errores falsos. Se redujo a 14 días en producción. Las opciones del Select deben reflejar los valores útiles: 7 y 14 días (no 15 ni 30).

## What Changes

- `frontend/src/pages/Settings.tsx`: opción `15 días` → `14 días`
- Sin cambios en backend (el valor se guarda igual como entero)
