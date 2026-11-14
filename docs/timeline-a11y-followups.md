# Timeline a11y — follow-ups (contraste AA, roving tabindex, .tsx)

Cierra el issue [#86](https://github.com/atareao/watchbeat/issues/86) (follow-ups de #63).
Change OpenSpec: `openspec/changes/timeline-a11y-followups/` (capabilities `frontend-timeline-a11y`
modificada, `frontend-status-contrast` nueva).

## Qué es

Tres mejoras de accesibilidad/calidad detectadas al auditar #63:

1. **Contraste AA** del texto de los `Tag` de estado de `MonitorCard`.
2. **Roving tabindex** en el timeline: una sola parada de `Tab` por gráfico.
3. **`statusStyles.ts` → `.tsx`** (JSX en vez de `createElement`).

## Ficheros implicados

| Fichero | Cambio |
|---|---|
| `frontend/src/theme/statusStyles.tsx` | renombrado desde `.ts`; iconos en JSX; campos `tagBg`/`tagText` |
| `frontend/src/components/MonitorCard.tsx` | `<Tag variant="solid" color={cfg.tagBg} style={{ color: cfg.tagText }}>` |
| `frontend/src/pages/MonitorDetail.tsx` | hook `useRovingBarFocus` + uso en `HeartbeatChart`/`HealthLatencyChart` |

## Cómo funciona

### Contraste (crítico: `variant="solid"`)

En **antd 6**, un `color` custom **no-preset** cae por defecto en `variant:'filled'`: antd pinta un
**tinte claro** (`hsl.l = 0.95`) de fondo y usa el hex como **color del texto**. Eso rompía el
contraste (p. ej. `no_data` quedaba blanco sobre `#f1f2f3` = 1.12:1). Por eso el `Tag` se renderiza
con **`variant="solid"`**, que hace que el fondo sea el hex del estado y valgan los ratios:

| Estado | `tagBg` (= fondo real) | `tagText` | Ratio | AA |
|---|---|---|---|---|
| up | `#22c55e` | `#111827` | 7.79:1 | ✅ |
| warning | `#f59e0b` | `#111827` | 8.26:1 | ✅ |
| down | `#ef4444` | `#111827` | 4.71:1 | ✅ |
| error | `#f97316` | `#111827` | 6.33:1 | ✅ |
| indeterminate | `#722ed1` | `#ffffff` | 6.94:1 | ✅ |
| no_data | `#6b7280` | `#ffffff` | 4.83:1 | ✅ |

Hex fijo ⇒ idéntico en tema claro y oscuro. Verificado empíricamente por SSR del `Tag` real de antd.

### Roving tabindex

Hook `useRovingBarFocus(barCount, resetKey)` por gráfico: solo la barra activa tiene
`tabIndex={0}` (el resto `-1`); ←/→ y Home/End mueven el foco con `preventDefault`; `resetKey`
(`range.bucketSeconds`) reubica la parada de `Tab` al primer bar cuando cambia el rango/filtro (sin
robar el foco al selector); el índice se recorta si la serie se acorta.

## Verificación

```bash
cd frontend && npx tsc --noEmit && npm run build
```
Contraste: tabla de arriba (calculada sobre el fondo **renderizado**, no el hex asumido).
Teclado: `Tab` entra una vez por gráfico; ←/→/Home/End mueven el foco y muestran el tooltip.

## Decisiones

- **`variant="solid"`** en el `Tag`: mantiene el hex del estado como fondo (identidad cromática) y
  cumple AA; descartado `filled` con tinte (perdía la identidad y rompía el contraste).
- **Roving tabindex** con estado local por gráfico (no `tabIndex` positivo, anti-patrón a11y).
- **`.tsx`**: los imports no llevan extensión → sin cambios en consumidores.

## Follow-ups

- Valorar `role="listbox"`/`aria-activedescendant` si el timeline crece en complejidad.
- `noUnusedLocals: false` en tsconfig oculta imports muertos.
