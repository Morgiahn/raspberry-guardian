# Guide de démarrage rapide

Pour les impatients qui veulent tester rapidement !

## En 5 minutes

### 1. Sur votre PC Windows (WSL)

```bash
# Cloner/télécharger le projet
cd ~/battery-guardian

# Éditer la configuration
nano config.toml
# Modifier l'IP de votre Shelly (ligne 7)
# ip = "192.168.1.XXX"  ← Votre IP

# Tester en local (simulation)
cargo run
```

### 2. Déploiement sur Raspberry Pi

```bash
# Compiler pour Pi (32-bit)
cargo build --release --target armv7-unknown-linux-gnueabihf

# Ou utiliser le script automatique (après avoir édité l'IP du Pi dedans)
chmod +x deploy.sh
./deploy.sh
```

### 3. Sur le Raspberry Pi

```bash
# Se connecter
ssh pi@192.168.1.50

# Tester manuellement
./battery-guardian
# Ctrl+C pour arrêter

# Installer comme service (démarrage auto)
sudo cp battery-guardian.service /etc/systemd/system/
sudo systemctl enable battery-guardian
sudo systemctl start battery-guardian

# Voir les logs
sudo journalctl -u battery-guardian -f
```

---

## Configuration minimale

### config.toml

```toml
[shelly]
ip = "192.168.1.100"        # ← ADAPTER ICI

[battery]
critical_threshold = 19      # Shutdown
low_threshold = 20           # Charge ON
high_threshold = 90          # Charge OFF
check_interval_seconds = 30

[network]
# La connectivité est vérifiée en testant si la prise Shelly répond
# Si la Shelly ne répond pas N fois de suite, on considère le réseau perdu et on shutdown
max_consecutive_failures = 3
check_interval_seconds = 60

[system]
shutdown_command = "sudo shutdown -h now"
```

---

## Checklist avant première utilisation

- [ ] Shelly Plug S configurée sur le WiFi
- [ ] IP de la Shelly trouvée (app Shelly ou scan réseau)
- [ ] Test manuel : `curl http://IP_SHELLY/relay/0?turn=on`
- [ ] `config.toml` édité avec la bonne IP
- [ ] Permissions sudo configurées : `sudo visudo` → `pi ALL=(ALL) NOPASSWD: /sbin/shutdown`
- [ ] Batterie/UPS HAT connecté au Pi

---

## Problèmes courants

### "Impossible de lire la batterie"

```bash
# Vérifier les chemins disponibles
ls -la /sys/class/power_supply/

# Si différent de BAT0, modifier src/battery.rs ligne 71
```

### "Shelly non accessible"

```bash
# Test réseau
ping 192.168.1.100
curl http://192.168.1.100/relay/0

# Vérifier que Pi et Shelly sont sur le même réseau WiFi
```

### "Permission denied" shutdown

```bash
sudo visudo
# Ajouter : pi ALL=(ALL) NOPASSWD: /sbin/shutdown
```

---

## Surveiller en temps réel

```bash
# Logs en direct
sudo journalctl -u battery-guardian -f

# Statut du service
sudo systemctl status battery-guardian

# Logs des dernières 24h
sudo journalctl -u battery-guardian --since "24 hours ago"
```

---

## Pour aller plus loin

- Lire `README.md` (guide complet)
- Lire `ARCHITECTURE.md` (explications des choix)
- Modifier le code et recompiler : `cargo build --release`

---

## Astuce : Mode simulation

Pour tester SANS vraiment éteindre le Pi :

```toml
[system]
shutdown_command = "echo 'SHUTDOWN SIMULÉ'"
```

Puis vérifier les logs :
```bash
sudo journalctl -u battery-guardian | grep "SHUTDOWN"
```
