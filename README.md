# Battery Guardian

Gestionnaire intelligent de batterie pour Raspberry Pi avec contrôle automatique de charge via Shelly Plug S.

## Table des matières

- [Vue d'ensemble](#vue-densemble)
- [Architecture du projet](#architecture-du-projet)
- [Concepts Rust expliqués](#concepts-rust-expliqués)
- [Installation et compilation](#installation-et-compilation)
- [Configuration](#configuration)
- [Déploiement sur Raspberry Pi](#déploiement-sur-raspberry-pi)
- [Tests](#tests)
- [Dépannage](#dépannage)

---

## Vue d'ensemble

### Fonctionnalités

- **Surveillance batterie en continu** - Lecture du niveau de charge toutes les 30s  
- **Gestion automatique des cycles** - Charge à 20%, décharge à 90%  
- **Protection critique** - Shutdown automatique si batterie < 19%  
- **Surveillance WiFi** - Détection de perte de connexion  
- **Contrôle Shelly Plug S** - Allumage/extinction via HTTP  
- **Logs détaillés** - Traçabilité complète des événements  
- **Hystérésis** - Évite les oscillations autour des seuils  

---

## Architecture du projet

```
battery-guardian/
├── Cargo.toml              # Dépendances et configuration du projet
├── config.toml             # Configuration utilisateur (IP Shelly, seuils, etc.)
├── README.md               # Ce fichier
├── docs/                   # Documentation détaillée (à créer)
└── src/
    ├── main.rs             # Point d'entrée + orchestration
    ├── config.rs           # Gestion configuration TOML
    ├── battery.rs          # Surveillance batterie
    ├── network.rs          # Surveillance WiFi
    ├── shelly.rs           # API Shelly Plug S
    └── state_machine.rs    # Machine à états (logique métier)
```

### Modules

| Module | Responsabilité | Concepts clés |
|--------|---------------|---------------|
| `main.rs` | Orchestration, boucle principale async | `tokio::main`, `Arc<Mutex<T>>`, `tokio::select!` |
| `config.rs` | Chargement TOML → Structs Rust | `serde::Deserialize`, validation |
| `battery.rs` | Lecture `/sys/class/power_supply/` | Accès fichiers système Linux |
| `network.rs` | Surveillance WiFi via Shelly | Compteur d'échecs, gestion perte réseau |
| `shelly.rs` | Requêtes HTTP vers Shelly API | `reqwest`, async/await, retry logic |
| `state_machine.rs` | États (Idle/Charging/Emergency) | `enum`, pattern matching |

---

## Concepts Rust expliqués

### 1. Ownership et Borrowing

**Le problème que Rust résout :**
En C/C++, gérer manuellement la mémoire cause des bugs (use-after-free, double-free).  
En Java/Python, un garbage collector ralentit le programme.

**La solution Rust : Ownership**
Chaque valeur a un propriétaire unique. Quand le propriétaire est détruit, la valeur l'est aussi.

```rust
// OK : s1 possède la String
let s1 = String::from("hello");

// ERREUR : s1 a été "déplacée" vers s2, s1 n'existe plus
let s2 = s1;
println!("{}", s1);  // ERREUR DE COMPILATION

// OK : emprunter (borrow) sans transférer ownership
let s1 = String::from("hello");
let len = calculate_length(&s1);  // &s1 = référence (borrow)
println!("{} a {} caractères", s1, len);  // s1 existe toujours

fn calculate_length(s: &String) -> usize {
    s.len()
}
```

**Dans notre projet :**
```rust
// battery_monitor "possède" les données de la batterie
let battery_monitor = BatteryMonitor::new();

// On "emprunte" pour lire sans prendre ownership
let info = battery_monitor.get_battery_info();
```

### 2. Result<T, E> et gestion d'erreurs

**Pas d'exceptions en Rust !**  
À la place : `Result<T, E>` qui force à gérer les erreurs.

```rust
// Type qui peut réussir (Ok) ou échouer (Err)
fn divide(a: i32, b: i32) -> Result<i32, String> {
    if b == 0 {
        Err("Division par zéro".to_string())
    } else {
        Ok(a / b)
    }
}

// Utilisation avec pattern matching
match divide(10, 2) {
    Ok(result) => println!("Résultat : {}", result),
    Err(e) => println!("Erreur : {}", e),
}

// Ou propagation avec l'opérateur ?
fn do_math() -> Result<i32, String> {
    let x = divide(10, 2)?;  // Si Err, retourne immédiatement
    let y = divide(x, 5)?;
    Ok(y)
}
```

**Dans notre projet :**
```rust
pub async fn turn_on(&self) -> Result<()> {
    self.send_command(&self.config.url_turn_on(), "Allumage").await
    // Si erreur HTTP, Result<()> devient Err(...) automatiquement
}
```

### 3. Async/Await

**Pourquoi async ?**  
On veut faire plusieurs choses en parallèle sans bloquer :
- Surveiller batterie toutes les 30s
- Surveiller réseau toutes les 60s
- Répondre instantanément aux urgences

**Comment ça marche :**
```rust
// Fonction normale (bloquante)
fn fetch_data() -> String {
    std::thread::sleep(Duration::from_secs(5));  // BLOQUE le thread
    "data".to_string()
}

// Fonction async (non-bloquante)
async fn fetch_data_async() -> String {
    tokio::time::sleep(Duration::from_secs(5)).await;  // NE BLOQUE PAS
    "data".to_string()
}

// Exécuter plusieurs tâches en parallèle
#[tokio::main]
async fn main() {
    tokio::join!(
        fetch_data_async(),
        fetch_data_async(),
        fetch_data_async(),
    );
    // Les 3 s'exécutent en parallèle, durée totale = 5s (pas 15s)
}
```

**Dans notre projet :**
```rust
tokio::select! {
    _ = monitor_battery(...) => {},  // Tâche 1 en parallèle
    _ = monitor_network(...) => {},  // Tâche 2 en parallèle
}
```

### 4. Arc<Mutex<T>> - Partage de données entre tâches

**Le problème :**
```rust
let mut state = StateController::new(...);

// ERREUR : on ne peut pas partager `state` mutable entre 2 tâches
tokio::spawn(async { state.update(...) });
tokio::spawn(async { state.update(...) });
```

**La solution : Arc<Mutex<T>>**

- **`Arc`** (Atomic Reference Counted) = pointeur partagé thread-safe
- **`Mutex`** = verrou pour accès exclusif

```rust
// Envelopper dans Arc<Mutex<>>
let state = Arc::new(Mutex::new(StateController::new(...)));

// Cloner Arc (incrémente juste un compteur, pas de copie des données)
let state_clone = state.clone();

tokio::spawn(async move {
    let mut s = state_clone.lock().await;  // Acquérir le verrou
    s.update(...);  // Modifier
    // Le verrou est libéré automatiquement ici
});
```

**Analogie :**  
Arc = pass de salle de gym (plusieurs personnes ont le pass)  
Mutex = porte de la salle (une seule personne à la fois peut entrer)

### 5. Enums et Pattern Matching

```rust
// Enum = type qui peut être dans un état parmi N
enum ChargeState {
    Idle,
    Charging,
    Emergency,
}

// Pattern matching = switch case sur stéroïdes
fn handle_state(state: ChargeState) {
    match state {
        ChargeState::Idle => println!("En attente"),
        ChargeState::Charging => println!("Charge en cours"),
        ChargeState::Emergency => println!("URGENCE !"),
    }
}
```

**Avantage vs constantes :**
```rust
// MAUVAIS : En C, facile d'oublier un cas
#define IDLE 0
#define CHARGING 1
int state = IDLE;
switch(state) {
    case IDLE: break;
    // Oubli de CHARGING = bug silencieux
}

// BON : En Rust, le compilateur force à gérer tous les cas
match state {
    ChargeState::Idle => {},
    // ERREUR si on oublie Charging/Emergency
}
```

### 6. Traits (= Interfaces)

```rust
// Trait = ensemble de méthodes qu'un type doit implémenter
trait Switchable {
    fn turn_on(&self) -> Result<()>;
    fn turn_off(&self) -> Result<()>;
}

// Implémenter le trait pour ShellyController
impl Switchable for ShellyController {
    fn turn_on(&self) -> Result<()> {
        // Code spécifique Shelly
    }
    
    fn turn_off(&self) -> Result<()> {
        // Code spécifique Shelly
    }
}

// Maintenant on peut écrire du code générique
fn control_device<T: Switchable>(device: &T) {
    device.turn_on();
}
```

**Traits dérivés automatiquement :**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Config {
    // ...
}

// On peut maintenant :
println!("{:?}", config);  // Debug
let copy = config.clone();  // Clone
let json = serde_json::to_string(&config);  // Serialize
```

---

## Installation et compilation

### Prérequis

**Sur votre PC Windows (WSL) :**
```bash
# 1. Installer Rust (si pas déjà fait)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# 2. Installer le target pour Raspberry Pi
rustup target add armv7-unknown-linux-gnueabihf  # Pi 2/3/4 (32-bit)
# ou
rustup target add aarch64-unknown-linux-gnu      # Pi 3/4 (64-bit)

# 3. Installer le cross-compiler
sudo apt update
sudo apt install gcc-arm-linux-gnueabihf  # Pour 32-bit
# ou
sudo apt install gcc-aarch64-linux-gnu    # Pour 64-bit
```

### Compilation locale (pour tester sur PC)

```bash
cd battery-guardian

# Compiler en mode debug (rapide, binaire non optimisé)
cargo build

# Compiler en mode release (lent, binaire optimisé)
cargo build --release

# Exécuter directement
cargo run
```

### Cross-compilation pour Raspberry Pi

**1. Configurer Cargo pour cross-compilation**

Créer `.cargo/config.toml` :
```toml
[target.armv7-unknown-linux-gnueabihf]
linker = "arm-linux-gnueabihf-gcc"

[target.aarch64-unknown-linux-gnu]
linker = "aarch64-linux-gnu-gcc"
```

**2. Compiler**
```bash
# Pour Pi 2/3/4 32-bit
cargo build --release --target armv7-unknown-linux-gnueabihf

# Pour Pi 3/4 64-bit
cargo build --release --target aarch64-unknown-linux-gnu
```

**3. Le binaire se trouve dans :**
```
target/armv7-unknown-linux-gnueabihf/release/battery-guardian
# ou
target/aarch64-unknown-linux-gnu/release/battery-guardian
```

---

## Configuration

### Fichier `config.toml`

```toml
[shelly]
ip = "192.168.1.100"      # À ADAPTER selon votre réseau
timeout_seconds = 5
max_retries = 3

[battery]
critical_threshold = 19   # Shutdown immédiat
low_threshold = 20        # Début de charge
high_threshold = 90       # Fin de charge
hysteresis = 2            # Marge anti-oscillation
check_interval_seconds = 30

[network]
# La connectivité réseau est vérifiée en testant si la prise Shelly répond
# (ping_host et ping_timeout_seconds ne sont plus utilisés)
max_consecutive_failures = 3  # Nombre d'échecs avant shutdown
check_interval_seconds = 60   # Vérifier la Shelly toutes les 60s

[logging]
level = "info"  # trace | debug | info | warn | error

[system]
shutdown_command = "sudo shutdown -h now"
shutdown_grace_period_seconds = 10
```

### Trouver l'IP de votre Shelly

**Méthode 1 : App Shelly**
1. Ouvrir l'app Shelly sur smartphone
2. Sélectionner votre Shelly Plug S
3. Paramètres → Informations → Adresse IP

**Méthode 2 : Scanner le réseau**
```bash
# Installer nmap
sudo apt install nmap

# Scanner votre réseau local (adapter 192.168.1.0)
nmap -sn 192.168.1.0/24

# Ou utiliser l'app Fing sur smartphone
```

**Méthode 3 : Routeur**
Accéder à l'interface de votre box et consulter la liste des appareils connectés.

### Tester manuellement la Shelly

```bash
# Allumer
curl http://192.168.1.100/relay/0?turn=on

# Éteindre
curl http://192.168.1.100/relay/0?turn=off

# Status
curl http://192.168.1.100/relay/0

# Réponse attendue :
# {"ison":true,"has_timer":false,...}
```

**Api**
```shell
# force mode auto
curl -X POST http://localhost:7878/mode      -H "Content-Type: application/json"      -d '{"mode":"auto"}' -v

```


---

## Déploiement sur Raspberry Pi

### 1. Transférer le binaire

```bash
# Depuis WSL, copier vers le Pi
scp target/armv7-unknown-linux-gnueabihf/release/battery-guardian pi@192.168.1.50:/home/pi/

# Copier aussi le fichier de config
scp config.toml pi@192.168.1.50:/home/pi/
```

### 2. Configuration des permissions sudo

Sur le Raspberry Pi :
```bash
# Éditer sudoers (ATTENTION : utiliser visudo, pas nano directement)
sudo visudo

# Ajouter cette ligne (remplacer 'pi' par votre nom d'utilisateur)
pi ALL=(ALL) NOPASSWD: /sbin/shutdown
```

### 3. Test manuel

```bash
ssh pi@192.168.1.50

cd /home/pi
chmod +x battery-guardian

# Test (Ctrl+C pour arrêter)
./battery-guardian
```

### 4. Créer un service systemd (démarrage automatique)

Créer `/etc/systemd/system/battery-guardian.service` :

```ini
[Unit]
Description=Battery Guardian - Gestionnaire de batterie intelligent
After=network.target

[Service]
Type=simple
User=pi
WorkingDirectory=/home/pi
ExecStart=/home/pi/battery-guardian
Restart=always
RestartSec=10
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
```

Activer le service :
```bash
sudo systemctl daemon-reload
sudo systemctl enable battery-guardian
sudo systemctl start battery-guardian

# Vérifier le statut
sudo systemctl status battery-guardian

# Voir les logs en temps réel
sudo journalctl -u battery-guardian -f
```

---

## Tests

### Tests unitaires

```bash
# Lancer tous les tests
cargo test

# Tests avec logs
cargo test -- --nocapture

# Test d'un module spécifique
cargo test battery::tests
```

### Tests d'intégration

```bash
# Créer un fichier de test : tests/integration_test.rs
# (À implémenter selon vos besoins)
```

### Tests manuels

1. **Simuler batterie basse**
   - Modifier temporairement `config.toml` : `low_threshold = 80`
   - Vérifier que la prise s'allume

2. **Simuler perte WiFi**
   - Arrêter la Shelly ou modifier son IP dans `config.toml`
   - Vérifier le shutdown après 3 échecs consécutifs (3 × 60s = 3 minutes)

3. **Tester shutdown (ATTENTION)**
   ```toml
   shutdown_command = "echo 'SHUTDOWN WOULD HAPPEN'"
   ```

---

## Dépannage

### Problème : "Impossible de lire la batterie"

**Cause :** Chemins `/sys/class/power_supply/` non standard sur votre UPS HAT.

**Solution :**
```bash
# Lister les chemins disponibles
ls -la /sys/class/power_supply/

# Si votre batterie est ailleurs, modifier battery.rs ligne 71
```

### Problème : "Shelly non accessible"

**Vérifications :**
1. IP correcte dans `config.toml`
2. Shelly et Pi sur le même réseau WiFi
3. Firewall Shelly désactivé (dans l'app Shelly)

**Test :**
```bash
ping 192.168.1.100
curl http://192.168.1.100/relay/0
```

### Problème : "Permission denied" pour shutdown

**Cause :** Pas de droits sudo sans mot de passe.

**Solution :**
```bash
sudo visudo
# Ajouter : pi ALL=(ALL) NOPASSWD: /sbin/shutdown
```

### Problème : Erreurs de compilation

**Erreur fréquente :** "cannot find -lgcc_s"

**Solution :**
```bash
# Sur WSL
sudo apt install gcc-multilib
```

### Logs et debug

```bash
# Niveau de log dans config.toml
level = "debug"  # Plus verbeux

# Logs systemd
sudo journalctl -u battery-guardian --since "1 hour ago"

# Logs en temps réel
sudo journalctl -u battery-guardian -f
```

---

## Ressources pour apprendre Rust

- **The Rust Book** : https://doc.rust-lang.org/book/ (LA référence)
- **Rust by Example** : https://doc.rust-lang.org/rust-by-example/
- **Tokio Tutorial** : https://tokio.rs/tokio/tutorial (pour l'async)
- **Rustlings** : https://github.com/rust-lang/rustlings (exercices interactifs)

---

## Prochaines améliorations possibles

- [ ] Interface web pour visualiser l'état (avec warp/axum)
- [ ] Historique des cycles de charge (SQLite)
- [ ] Notifications (email, Telegram)
- [ ] Support autres prises (TP-Link Kasa, Tasmota)
- [ ] Calibration automatique des seuils
- [ ] Mode debug avec simulation de batterie

---

**Besoin d'aide ?** Ouvrez une issue sur GitHub ou contactez-moi !
