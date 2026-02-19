#!/usr/bin/env bash
set -euo pipefail

mkdir -p apps/api/src/{config,db/migrations,modules,middleware,ws,utils}
mkdir -p apps/web/src/{pages,components,features,api,stores}
mkdir -p packages/shared-types/src
mkdir -p docs/architecture

cat > apps/api/src/index.js <<'JS'
console.log('API bootstrap placeholder');
JS

cat > apps/web/index.html <<'HTML'
<!doctype html><title>Web bootstrap placeholder</title>
HTML

cat > packages/shared-types/src/index.ts <<'TS'
export type UserRole = 'owner' | 'manager' | 'reviewer' | 'viewer';
export type TaskPhase = 'needs' | 'planning' | 'implementation' | 'validation' | 'revision' | 'review' | 'done' | 'blocked';
TS

echo "Bootstrap complete."
