# ═══════════════════════════════════════════════════════════════
# Pre-commit checklist
# ═══════════════════════════════════════════════════════════════
# Ejecutar siempre en este orden antes de commitear:
#   just check
# ═══════════════════════════════════════════════════════════════

check:
    cd backend && cargo fmt -- --check && cargo clippy -- -D warnings

user     := "atareao"
name     := "watchbeat"
version  := `vampus show`


list:
    @just --list

lint:
    cd backend && cargo clippy --all-targets --all-features

fmt:
    cd backend && cargo fmt -- --check

fmt-fix:
    cd backend && cargo fmt

build:
    @podman build \
        --tag={{user}}/{{name}}:{{version}} \
        --tag={{user}}/{{name}}:latest .

push:
    @podman image push --authfile ~/.podman-auth.json {{user}}/{{name}}:{{version}}
    @podman image push --authfile ~/.podman-auth.json {{user}}/{{name}}:latest

# ═══════════════════════════════════════════════════════════════
# GitFlow recipes
# ═══════════════════════════════════════════════════════════════

# Iniciar una feature (crea rama desde develop)
gf-feature feature_name:
    git checkout develop
    git pull origin develop
    git checkout -b feature/{{feature_name}}

# Finalizar una feature (merge a develop con --no-ff)
gf-finish feature_name:
    git checkout develop
    git pull origin develop
    git merge --no-ff feature/{{feature_name}} -m "feat: merge feature/{{feature_name}} into develop"
    git push origin develop
    git branch -d feature/{{feature_name}}
    @echo "✅ Feature {{feature_name}} merged into develop and branch deleted"

# Iniciar un release (crea rama desde develop)
gf-release version:
    git checkout develop
    git pull origin develop
    git checkout -b release/{{version}}

# Publicar un release (merge a main + develop + tag)
gf-publish version:
    #!/bin/fish
    # Pre-commit checks
    cd backend && cargo fmt -- --check; and cargo clippy -- -D warnings
    or begin
        echo "❌ Pre-commit checks failed. Aborting."
        exit 1
    end
    # Merge a main
    git checkout main
    git pull origin main
    git merge --no-ff release/{{version}} -m "release: v{{version}}"
    git tag -a "v{{version}}" -m "Version {{version}}"
    # Merge a develop
    git checkout develop
    git pull origin develop
    git merge --no-ff release/{{version}} -m "release: merge v{{version}} into develop"
    # Push both
    git push origin main --tags
    git push origin develop
    git branch -d release/{{version}}
    echo "✅ Release v{{version}} published to main and merged back to develop"

# Iniciar un hotfix (crea rama desde main)
gf-hotfix desc:
    git checkout main
    git pull origin main
    git checkout -b hotfix/{{desc}}

# Publicar un hotfix (merge a main + develop + tag)
gf-hotfix-publish desc version:
    #!/bin/fish
    # Merge a main
    git checkout main
    git pull origin main
    git merge --no-ff hotfix/{{desc}} -m "hotfix: {{desc}}"
    git tag -a "{{version}}" -m "Hotfix {{version}}"
    # Merge a develop
    git checkout develop
    git pull origin develop
    git merge --no-ff hotfix/{{desc}} -m "hotfix: merge {{desc}} into develop"
    # Push both
    git push origin main --tags
    git push origin develop
    git branch -d hotfix/{{desc}}
    echo "✅ Hotfix {{desc}} published"

# Mostar el árbol de ramas
gf-graph:
    git log --oneline --graph --all --decorate=short -30

upgrade:
    #!/bin/fish
    vampus upgrade --patch
    set VERSION $(vampus show)
    cd backend && cargo update
    git commit -am "Upgrade to version $VERSION"
    git tag -a "$VERSION" -m "Version $VERSION"
    # clean old podman images
    podman image list  | grep {{name}} | sort -r | tail -n +5 | awk '{print $3}' | while read id; echo $id; podman rmi $id; end
    just build push
