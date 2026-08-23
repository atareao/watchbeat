# GIT_FLOW.md

Este proyecto sigue **Git Flow** con conventional commits (gitmoji).

## Ramas

| Rama | Propósito | Base |
|------|-----------|------|
| `main` | Producción | — |
| `development` | Integración | `main` |
| `feature/*` | Features | `development` |

## Commits

| Tipo | Emoji |
|------|-------|
| feat | ✨ |
| fix | 🐛 |
| docs | 📝 |
| refactor | ♻️ |
| chore | 🔧 |

## Releases

PR `development` → `main`. Al mergear, CI genera CHANGELOG y tag.