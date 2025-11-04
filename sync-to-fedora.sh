#!/usr/bin/env bash
# Sync stop project to Fedora for testing

set -euo pipefail

FEDORA_HOST="nick@fedora"
REMOTE_PATH="~/stop"

echo "Syncing stop to Fedora..."
echo ""

# Sync project (exclude build artifacts and git)
rsync -av --delete \
    --exclude 'target/' \
    --exclude '.git/' \
    --exclude '*.swp' \
    --exclude '.DS_Store' \
    ./ ${FEDORA_HOST}:${REMOTE_PATH}/

echo ""
echo "✅ Sync complete!"
echo ""
echo "Next steps:"
echo "  ssh ${FEDORA_HOST}"
echo "  cd ${REMOTE_PATH}"
echo "  ./fedora-test.sh"
