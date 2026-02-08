#!/bin/bash
# Script de déploiement automatique vers Raspberry Pi

set -e  # Arrêter en cas d'erreur

# ============================================================================
# CONFIGURATION - À ADAPTER
# ============================================================================

# IP de votre Raspberry Pi
PI_HOST="192.168.1.50"
PI_USER="pi"

# Architecture cible (choisir une des deux lignes)
TARGET="armv7-unknown-linux-gnueabihf"  # Pi 2/3/4 32-bit
# TARGET="aarch64-unknown-linux-gnu"     # Pi 3/4 64-bit

# Chemins
BINARY_NAME="battery-guardian"
REMOTE_DIR="/home/pi"

# ============================================================================
# SCRIPT
# ============================================================================

echo "Compilation pour $TARGET..."
cargo build --release --target $TARGET

echo "Copie du binaire vers $PI_USER@$PI_HOST..."
scp "target/$TARGET/release/$BINARY_NAME" "$PI_USER@$PI_HOST:$REMOTE_DIR/"

echo "Copie de la configuration..."
scp config.toml "$PI_USER@$PI_HOST:$REMOTE_DIR/"

echo "Configuration des permissions..."
ssh "$PI_USER@$PI_HOST" "chmod +x $REMOTE_DIR/$BINARY_NAME"

echo "Déploiement terminé !"
echo ""
echo "Pour installer le service systemd :"
echo "  scp battery-guardian.service $PI_USER@$PI_HOST:/tmp/"
echo "  ssh $PI_USER@$PI_HOST"
echo "  sudo mv /tmp/battery-guardian.service /etc/systemd/system/"
echo "  sudo systemctl daemon-reload"
echo "  sudo systemctl enable battery-guardian"
echo "  sudo systemctl start battery-guardian"
echo ""
echo "Pour tester manuellement :"
echo "  ssh $PI_USER@$PI_HOST"
echo "  cd $REMOTE_DIR && ./$BINARY_NAME"
