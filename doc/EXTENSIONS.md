# Extensions et Améliorations

Ce document propose des idées d'améliorations pour étendre le projet et continuer à apprendre Rust.

---

## Niveau 1 : Débutant (1-2 heures)

### 1.1 Ajouter des emojis dans les logs

**Objectif :** Rendre les logs plus visuels  
**Difficulté :** ⭐☆☆☆☆

```rust
// Dans src/main.rs
info!("Batterie : {}%", battery_info.level);
warn!("Attention : batterie basse");
error!("Erreur critique");
```

### 1.2 Configuration des emojis/couleurs

**Objectif :** Rendre les logs configurables  
**Difficulté :** ⭐⭐☆☆☆

```toml
# config.toml
[logging]
use_emojis = true
use_colors = true
```

```rust
// src/config.rs
pub struct LoggingConfig {
    pub level: String,
    pub use_emojis: bool,
    pub use_colors: bool,
}
```

### 1.3 Statistiques simples

**Objectif :** Compter les cycles de charge  
**Difficulté :** ⭐⭐☆☆☆

```rust
// src/state_machine.rs
pub struct StateController {
    current_state: ChargeState,
    config: BatteryConfig,
    charge_cycles: u32,  // Nouveau champ
}

impl StateController {
    pub fn update_state(&mut self, battery_level: u8) -> bool {
        // ...
        if old_state == ChargeState::Charging && new_state == ChargeState::Idle {
            self.charge_cycles += 1;
            info!("Cycle de charge terminé (total : {})", self.charge_cycles);
        }
        // ...
    }
}
```

---

## Niveau 2 : Intermédiaire (3-5 heures)

### 2.1 Persistance de l'état (fichier JSON)

**Objectif :** Sauvegarder l'état entre redémarrages  
**Difficulté :** ⭐⭐⭐☆☆

**Nouvelle crate :**
```toml
# Cargo.toml
[dependencies]
serde_json = "1.0"
```

**Implémentation :**
```rust
// src/state_machine.rs
use std::fs;

impl StateController {
    pub fn save_state(&self) -> Result<()> {
        let state = serde_json::json!({
            "current_state": format!("{:?}", self.current_state),
            "charge_cycles": self.charge_cycles,
        });
        
        fs::write("state.json", state.to_string())?;
        Ok(())
    }
    
    pub fn load_state() -> Result<Self> {
        let content = fs::read_to_string("state.json")?;
        // Parser et reconstruire
        // ...
    }
}
```

### 2.2 Historique des niveaux de batterie

**Objectif :** Logger l'historique dans SQLite  
**Difficulté :** ⭐⭐⭐☆☆

**Nouvelles crates :**
```toml
[dependencies]
rusqlite = "0.31"
chrono = "0.4"
```

**Implémentation :**
```rust
// src/history.rs (nouveau module)
use rusqlite::{Connection, Result};
use chrono::Utc;

pub struct BatteryHistory {
    conn: Connection,
}

impl BatteryHistory {
    pub fn new() -> Result<Self> {
        let conn = Connection::open("battery_history.db")?;
        
        conn.execute(
            "CREATE TABLE IF NOT EXISTS battery_log (
                id INTEGER PRIMARY KEY,
                timestamp TEXT NOT NULL,
                level INTEGER NOT NULL,
                state TEXT NOT NULL
            )",
            [],
        )?;
        
        Ok(Self { conn })
    }
    
    pub fn log_state(&self, level: u8, state: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO battery_log (timestamp, level, state) VALUES (?1, ?2, ?3)",
            &[&Utc::now().to_rfc3339(), &level.to_string(), state],
        )?;
        Ok(())
    }
}
```

### 2.3 API Web pour visualiser l'état

**Objectif :** Interface web simple  
**Difficulté :** ⭐⭐⭐⭐☆

**Nouvelles crates :**
```toml
[dependencies]
axum = "0.7"
tower = "0.4"
```

**Implémentation :**
```rust
// src/web.rs (nouveau module)
use axum::{routing::get, Router, Json};
use serde::Serialize;

#[derive(Serialize)]
struct StatusResponse {
    battery_level: u8,
    state: String,
    is_charging: bool,
}

async fn get_status() -> Json<StatusResponse> {
    // Lire l'état actuel
    Json(StatusResponse {
        battery_level: 75,
        state: "Idle".to_string(),
        is_charging: false,
    })
}

pub async fn run_web_server() {
    let app = Router::new()
        .route("/api/status", get(get_status));
    
    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}
```

---

## Niveau 3 : Avancé (8-12 heures)

### 3.1 Support multi-prises (TP-Link, Tasmota, etc.)

**Objectif :** Abstraction avec traits  
**Difficulté :** ⭐⭐⭐⭐☆

**Architecture :**
```rust
// src/smart_plug.rs (nouveau module)
use async_trait::async_trait;

#[async_trait]
pub trait SmartPlug: Send + Sync {
    async fn turn_on(&self) -> Result<()>;
    async fn turn_off(&self) -> Result<()>;
    async fn get_status(&self) -> Result<bool>;
    async fn is_reachable(&self) -> bool;
}

// src/plugs/shelly.rs
pub struct ShellyPlug { /* ... */ }

#[async_trait]
impl SmartPlug for ShellyPlug {
    async fn turn_on(&self) -> Result<()> { /* ... */ }
    // ...
}

// src/plugs/tplink.rs
pub struct TpLinkPlug { /* ... */ }

#[async_trait]
impl SmartPlug for TpLinkPlug {
    async fn turn_on(&self) -> Result<()> { /* ... */ }
    // ...
}
```

**Configuration :**
```toml
[plug]
type = "shelly"  # ou "tplink", "tasmota", etc.
ip = "192.168.1.100"
```

### 3.2 Notifications (Telegram, Email)

**Objectif :** Alertes à distance  
**Difficulté :** ⭐⭐⭐⭐☆

**Nouvelles crates :**
```toml
[dependencies]
teloxide = "0.12"  # Pour Telegram
lettre = "0.11"    # Pour email
```

**Implémentation :**
```rust
// src/notifications.rs
use teloxide::prelude::*;

pub struct NotificationService {
    bot: Bot,
    chat_id: i64,
}

impl NotificationService {
    pub async fn send_alert(&self, message: &str) -> Result<()> {
        self.bot.send_message(self.chat_id, message).await?;
        Ok(())
    }
}

// Dans src/main.rs
if is_emergency {
    notifier.send_alert("BATTERIE CRITIQUE !").await?;
}
```

### 3.3 Dashboard web avec graphiques

**Objectif :** Interface graphique complète  
**Difficulté :** ⭐⭐⭐⭐⭐

**Stack technique :**
- Backend : Axum (Rust)
- Frontend : HTML + Chart.js (ou React)
- WebSocket pour temps réel

**Backend :**
```rust
// src/web.rs
use axum::{
    extract::ws::{WebSocket, WebSocketUpgrade},
    response::IntoResponse,
};

async fn ws_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(handle_socket)
}

async fn handle_socket(mut socket: WebSocket) {
    loop {
        let status = get_battery_status();
        socket.send(status.into()).await.ok();
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}
```

**Frontend (static/index.html) :**
```html
<!DOCTYPE html>
<html>
<head>
    <script src="https://cdn.jsdelivr.net/npm/chart.js"></script>
</head>
<body>
    <canvas id="batteryChart"></canvas>
    <script>
        const ws = new WebSocket('ws://localhost:3000/ws');
        const chart = new Chart(ctx, {
            type: 'line',
            data: { labels: [], datasets: [{ data: [] }] }
        });
        
        ws.onmessage = (event) => {
            const data = JSON.parse(event.data);
            chart.data.labels.push(data.timestamp);
            chart.data.datasets[0].data.push(data.battery_level);
            chart.update();
        };
    </script>
</body>
</html>
```

---

## Niveau 4 : Expert (15+ heures)

### 4.1 Machine Learning : Prédiction de durée de vie batterie

**Objectif :** Prédire combien de temps reste avant shutdown  
**Difficulté :** ⭐⭐⭐⭐⭐

**Crate :**
```toml
[dependencies]
linfa = "0.7"  # ML en Rust
ndarray = "0.15"
```

**Approche :**
1. Collecter historique (niveau batterie vs temps)
2. Entraîner modèle de régression linéaire
3. Prédire temps restant

### 4.2 Auto-calibration des seuils

**Objectif :** Ajuster automatiquement les seuils selon l'usage  
**Difficulté :** ⭐⭐⭐⭐⭐

**Algorithme :**
```rust
// Analyser les cycles passés
// Si cycles trop courts (< 2h) → augmenter hystérésis
// Si cycles trop longs (> 12h) → réduire hystérésis
```

### 4.3 Clustering de Raspberry Pi (mode haute disponibilité)

**Objectif :** Plusieurs Pi surveillent la même batterie  
**Difficulté :** ⭐⭐⭐⭐⭐

**Technologies :**
- Raft consensus (crate `raft`)
- Communication inter-Pi (gRPC)

---

## Exercices progressifs

### Exercice 1 : Ajouter un mode debug

**Objectif :** Mode simulation sans vraie batterie

**Étapes :**
1. Ajouter `debug_mode` dans config.toml
2. Créer `MockBatteryMonitor` qui simule des niveaux
3. Utiliser trait pour abstraire BatteryMonitor

### Exercice 2 : Créer des tests d'intégration

**Objectif :** Tester le système complet

**Fichier : tests/integration_test.rs**
```rust
#[tokio::test]
async fn test_charge_cycle() {
    // Simuler batterie à 20%
    // Vérifier que prise s'allume
    // Simuler montée à 90%
    // Vérifier que prise s'éteint
}
```

### Exercice 3 : CLI pour contrôler manuellement

**Objectif :** Interface ligne de commande

**Crate :**
```toml
[dependencies]
clap = { version = "4.0", features = ["derive"] }
```

**Usage :**
```bash
battery-guardian status
battery-guardian force-charge on
battery-guardian history --last 24h
```

---

## Bonnes pratiques à adopter

### 1. Tests automatiques

```bash
# Lancer tests avant chaque commit
cargo test

# Vérifier le formatage
cargo fmt --check

# Linter
cargo clippy
```

### 2. Documentation

```rust
/// Allume la prise connectée.
/// 
/// # Erreurs
/// 
/// Retourne une erreur si :
/// - La prise n'est pas accessible
/// - Timeout de la requête HTTP
/// 
/// # Exemple
/// 
/// ```
/// let shelly = ShellyController::new(config)?;
/// shelly.turn_on().await?;
/// ```
pub async fn turn_on(&self) -> Result<()> {
    // ...
}
```

### 3. CI/CD avec GitHub Actions

**.github/workflows/ci.yml :**
```yaml
name: CI

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - run: cargo test
      - run: cargo clippy
```

---

## Ressources pour aller plus loin

### Livres
- **"Programming Rust"** (O'Reilly) - Approfondir Rust
- **"Zero To Production In Rust"** - Web services en Rust

### Projets similaires à étudier
- **systemd** (Rust rewrite) - Gestion de services
- **ripgrep** - Outil de recherche ultra-rapide
- **tokio-console** - Debugger pour async

### Communautés
- **Discord Rust** : https://discord.gg/rust-lang
- **Reddit /r/rust** : https://reddit.com/r/rust
- **Forum Rust Users** : https://users.rust-lang.org/

---

**Prochain défi :** Choisir une amélioration de Niveau 2 et l'implémenter !
