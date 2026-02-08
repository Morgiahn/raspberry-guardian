#!/usr/bin/env bash

result=${PWD##*/}
if [ "$result" != "raspberry-guardian" ]; then
    echo "Please run this script from the root directory."
    exit 1
fi

echo "Installing raspberry-guardian api command ..."

# Dossier cible
TARGET_DIR="$HOME/bin"

# Créer le dossier s'il n'existe pas
mkdir -p "$TARGET_DIR"

# Copier le binaire
cp install/cmd-api/* "$TARGET_DIR/"

for f in "$PWD"/install/cmd-api/*; do
    # Vérifier que le fichier existe et que son nom commence par "guardian"
    if [ -f "$f" ] && [[ $(basename "$f") == guardian* ]]; then
        # Rendre exécutable
        chmod +x "$f"
        # Créer un lien symbolique dans le dossier cible
        ln -sf "$f" "$TARGET_DIR/"
    fi
done



echo "raspberry-guardian-api commands installed successfully in $TARGET_DIR/"
ls -l "$TARGET_DIR"/guardian-*