## Installation du programme sur Raspberry Pi
```shell
cargo build --release

# Copier le binaire dans /usr/local/bin/ et lui donner les droits d'exécution
sudo cp target/release/battery-guardian /usr/local/bin/
sudo chmod +x /usr/local/bin/battery-guardian
```
Indiquer le chemin du binaire dans le fichier `battery-guardian.service` : 
```ini
ExecStart=/usr/local/bin/battery-guardian
```

## User dédié
Le programme est lancer avec un user dédié : `battery-guardian`  

Créer le user : 
```shell
sudo adduser --system --group battery-guardian
```
Crée un user système  
crée automatiquement un groupe du même nom  
choisit UID/GID proprement  
crée un home dans /home ou /var/lib  
applique les conventions Debian  


Donner les droits sudo pour shutdown:
```shell
sudo visudo

# ajouter à la fin 
battery-guardian ALL=(ALL) NOPASSWD: /usr/sbin/shutdown
```

Donner les droits pour accéder à I2C (stat battery)
```shell
sudo usermod -aG i2c battery-guardian
```

## Créer le dossier du fichier state_file
```shell
sudo mkdir -p /var/lib/battery-guardian
sudo chown battery-guardian:battery-guardian /var/lib/battery-guardian
```

## Service
copîer le fichier `battery-guardian.service` dans `/etc/systemd/system/`  
```shell 
sudo cp battery-guardian.service /etc/systemd/system/
``` 

## Lancer le programme 
```shell
sudo systemctl daemon-reload
sudo systemctl enable battery-guardian
sudo systemctl start battery-guardian
```

## voir le journal

```shell

sudo journalctl -u battery-guardian.service 
sudo journalctl -u battery-guardian.service --since "2026-02-28 21:00:00"

```

