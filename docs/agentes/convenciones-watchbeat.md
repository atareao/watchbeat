### Comandos esenciales

```bash
## Pre-commit (siempre en este orden)
just check                    # cargo fmt --check + cargo clippy -- -D warnings

## Backend
cd backend && cargo build
cd backend && cargo test      # Tests unitarios + integración
cd backend && cargo test --test db_integration  # Solo integración
cd backend && cargo clippy --all-targets --all-features
cd backend && cargo fmt

## Frontend
cd frontend && npm run dev    # Dev server en :3050, proxy a backend :3055
cd frontend && npm run build  # tsc -b && vite build

## Docker
just build                    # podman build con tag de versión (vampus)
just push                     # podman push con authfile
docker compose -f compose.yml up -d

## Git Flow (vía just)
just gf-feature <name>        # feature/<name> desde development
just gf-finish <name>         # merge --no-ff a development
just gf-release <version>     # release/<version> desde development
just gf-publish <version>     # merge a main + develop + tag
just gf-hotfix <desc>         # hotfix/<desc> desde main
just gf-hotfix-publish <desc> <version>
just gf-graph                 # git log --oneline --graph --all -30
```

### Convenciones

#### Git Flow
- `main` = producción, `development` = integración
- Prefijos: `feature/`, `release/`, `hotfix/`, `support/`
- Siempre `--no-ff` en merges
- Tags semánticas: `v0.1.0`, `v0.2.0`, etc.

#### Commits (Conventional Commits + gitmoji)
| Tipo | Emoji |
|------|-------|
| feat | ✨ |
| fix | 🐛 |
| fix (seguridad) | 🔒 |
| refactor | ♻️ |
| perf | ⚡ |
| docs | 📝 |
| test | ✅ |
| style | 🎨 |
| chore | 🔧 |
| rename | 🏷️ |

#### Versionado
- `vampus` tool gestiona la versión en `Cargo.toml`
- `just upgrade` incrementa patch, hace `cargo update`, commitea y tagea

