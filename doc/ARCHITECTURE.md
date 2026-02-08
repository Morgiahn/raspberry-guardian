# Architecture et Choix de Design - Battery Guardian

Ce document explique en détail **pourquoi** chaque décision a été prise dans ce projet.

---

## Table des matières

1. [Choix du langage : Pourquoi Rust ?](#1-choix-du-langage--pourquoi-rust-)
2. [Architecture modulaire](#2-architecture-modulaire)
3. [Gestion de la concurrence (async/await)](#3-gestion-de-la-concurrence-asyncawait)
4. [Machine à états](#4-machine-à-états)
5. [Gestion d'erreurs](#5-gestion-derreurs)
6. [Choix des bibliothèques (crates)](#6-choix-des-bibliothèques-crates)
7. [Configuration externe](#7-configuration-externe)
8. [Sécurité et robustesse](#8-sécurité-et-robustesse)

---

## 1. Choix du langage : Pourquoi Rust ?

### Comparaison avec les alternatives

| Critère | Rust | Python | C++ | Go |
|---------|------|--------|-----|-----|
| **Performance** | ⭐⭐⭐⭐⭐ | ⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ |
| **Mémoire (RAM)** | ~5 MB | ~20 MB | ~5 MB | ~10 MB |
| **Sécurité** | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐ | ⭐⭐⭐⭐ |
| **Vitesse de dev** | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐ | ⭐⭐⭐⭐ |
| **Courbe d'apprentissage** | ⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐ | ⭐⭐⭐⭐ |
| **Concurrence** | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ |

### Pourquoi Rust pour ce projet spécifique ?

**1. Fiabilité critique**
- Ce système contrôle une batterie → un bug peut détruire le matériel
- Rust garantit l'absence de data races et de memory leaks à la compilation
- Pas de surprises runtime (contrairement à Python/C++)

**2. Efficacité sur Raspberry Pi**
- RAM limitée (512 MB - 8 GB selon le modèle)
- Rust consomme ~5 MB vs Python ~20 MB
- CPU économisé = batterie économisée

**3. Concurrence native**
- On doit surveiller batterie ET réseau en parallèle
- Tokio (runtime async Rust) est plus efficace que threading Python
- Pas de GIL (Global Interpreter Lock) comme Python

**4. Objectif d'apprentissage**
- Vous voulez apprendre Rust → projet parfait (concret, utile, couvre beaucoup de concepts)
- Alternatives (Python/Go) seraient "trop faciles" et n'enseigneraient pas autant

### Ce que Rust apporte vs les alternatives

**vs Python :**
- 4x moins de RAM, 10x plus rapide
- Erreurs détectées à la compilation, pas en production
- Plus long à écrire

**vs C++ :**
- Pas de segfaults, use-after-free, data races
- Gestion d'erreurs moderne (Result vs errno/exceptions)
- Package manager intégré (Cargo vs CMake hell)
- Compilation plus lente

**vs Go :**
- Pas de garbage collector → latence prédictible
- Plus performant (binaire plus léger)
- Plus complexe à apprendre

---

## 2. Architecture modulaire

### Organisation en modules

```
src/
├── main.rs              # Orchestration (100 lignes)
├── config.rs            # Configuration (150 lignes)
├── battery.rs           # Batterie (120 lignes)
├── network.rs           # Réseau (130 lignes)
├── shelly.rs            # API Shelly (110 lignes)
└── state_machine.rs     # Logique métier (150 lignes)
```

### Principe : Séparation des responsabilités (SoC)

Chaque module a **une seule responsabilité** :

| Module | Responsabilité | Ne fait PAS |
|--------|---------------|-------------|
| `config.rs` | Charger/valider config TOML | Logique métier, I/O système |
| `battery.rs` | Lire état batterie | Décider quoi faire avec |
| `network.rs` | Vérifier connectivité | Contrôler la prise |
| `shelly.rs` | API HTTP Shelly | Savoir quand allumer/éteindre |
| `state_machine.rs` | Décisions (quand charger/décharger) | I/O réseau/système |
| `main.rs` | Orchestration | Logique métier |

### Avantages

**1. Testabilité**
```rust
// On peut tester state_machine.rs SANS Shelly, réseau, ou batterie
#[test]
fn test_transition_to_charging() {
    let mut controller = StateController::new(...);
    controller.update_state(20);
    assert_eq!(controller.current_state(), ChargeState::Charging);
}
```

**2. Maintenabilité**
- Besoin de changer l'API Shelly ? → Modifier seulement `shelly.rs`
- Nouvelle source de batterie ? → Modifier seulement `battery.rs`
- Nouvelle logique de charge ? → Modifier seulement `state_machine.rs`

**3. Réutilisabilité**
- `shelly.rs` pourrait être extrait en crate séparée
- `state_machine.rs` pourrait gérer d'autres systèmes à seuils

### Pattern : Dependency Injection

```rust
// Au lieu de créer les dépendances à l'intérieur :
impl StateController {
    pub fn new() -> Self {
        let shelly = ShellyController::new(...);  // Couplage fort
        Self { shelly }
    }
}

// On les passe en paramètre :
async fn monitor_battery(
    shelly: Arc<ShellyController>,  // Injection
    state: Arc<Mutex<StateController>>,
) {
    // ...
}
```

**Avantage :** On peut injecter un mock pour les tests.

---

## 3. Gestion de la concurrence (async/await)

### Pourquoi async et pas threads ?

**Threads traditionnels (OS threads) :**
```rust
// Chaque thread consomme ~2 MB de stack
std::thread::spawn(|| { /* surveiller batterie */ });
std::thread::spawn(|| { /* surveiller réseau */ });
// Total : ~4 MB juste pour les stacks
```

**Async tasks (green threads) :**
```rust
// Chaque task consomme ~2 KB
tokio::spawn(async { /* surveiller batterie */ });
tokio::spawn(async { /* surveiller réseau */ });
// Total : ~4 KB
```

**➡️ 1000x moins de mémoire !**

### Modèle de concurrence : Tokio

```
┌──────────────────────────────────────────┐
│         TOKIO RUNTIME                    │
│                                          │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐   │
│  │ Thread  │  │ Thread  │  │ Thread  │   │ ← Nombre = CPU cores
│  │ Pool 1  │  │ Pool 2  │  │ Pool 3  │   │
│  └─────────┘  └─────────┘  └─────────┘   │
│       ↑            ↑            ↑        │
│       └────────────┴────────────┘        │
│                                          │
│    Scheduler (répartit les tasks)        │
│         ↓           ↓           ↓        │
│  [Task 1000] [Task 2000] [Task 3000]     │ ← Peuvent être 10 000+ tasks
│                                          │
└──────────────────────────────────────────┘
```

### Pattern : tokio::select!

```rust
tokio::select! {
    _ = monitor_battery(...) => { /* si cette tâche se termine */ },
    _ = monitor_network(...) => { /* ou si celle-ci se termine */ },
}
```

**Comportement :**
- Les 2 tâches s'exécutent en parallèle
- `select!` attend que l'une d'elles se termine (ou les 2)
- Si une tâche panic, l'autre continue (important pour la robustesse)

### Pattern : Arc<Mutex<T>> pour partage d'état

**Problème Rust :**
```rust
let mut state = StateController::new(...);

// ❌ ERREUR : cannot move out of `state` 
tokio::spawn(async move { state.update(...) });
tokio::spawn(async move { state.update(...) });  // `state` déjà déplacé !
```

**Solution :**
```rust
// Arc = Atomic Reference Counted (compteur de références thread-safe)
// Mutex = verrou pour accès mutuellement exclusif
let state = Arc::new(Mutex::new(StateController::new(...)));

// Clone de l'Arc (incrémente juste le compteur, pas de copie des données)
let state_clone = state.clone();

tokio::spawn(async move {
    let mut s = state_clone.lock().await;  // Attendre le verrou
    s.update(...);  // Modifier
});  // Verrou libéré automatiquement (Drop trait)
```

**Analogie :**
- `Arc` = pass de salle de gym (plusieurs personnes peuvent avoir le pass)
- `Mutex` = porte de la salle (une seule personne à la fois peut entrer)
- `.lock()` = attendre devant la porte jusqu'à ce qu'elle soit libre

### Pourquoi `.await` et pas `.lock()` bloquant ?

```rust
// ❌ MAUVAIS : std::sync::Mutex (bloquant)
let state = Arc::new(std::sync::Mutex::new(...));
let mut s = state.lock().unwrap();  // BLOQUE le thread entier
s.update(...);

// ✅ BON : tokio::sync::Mutex (async)
let state = Arc::new(tokio::sync::Mutex::new(...));
let mut s = state.lock().await;  // Libère le thread pendant l'attente
s.update(...);
```

Dans un runtime async, bloquer un thread est catastrophique car il pourrait gérer 1000 tasks.

---

## 4. Machine à états

### Pourquoi une state machine ?

**Alternative 1 : Flags booléens (❌ mauvais)**
```rust
let mut is_charging = false;
let mut is_emergency = false;

// Problème : États invalides possibles
// Exemple : is_charging=true ET is_emergency=true en même temps
```

**Alternative 2 : Enum (✅ bon)**
```rust
enum ChargeState {
    Idle,
    Charging,
    Emergency,
}

// Impossible d'être dans 2 états en même temps
// Le compilateur force à gérer tous les cas
```

### Diagramme d'états

```
        battery ≤ critical_threshold
                    ↓
              ┌──────────┐
              │EMERGENCY │ → SHUTDOWN
              └──────────┘
                    ↑
                    │
    ┌──────────────────────────────────┐
    │                                  │
    v                                  │
┌────────┐  battery ≤ low    ┌─────────────┐
│  IDLE  │───────────────────>│  CHARGING   │
└────────┘                    └─────────────┘
    ↑                                 │
    │      battery ≥ high             │
    │                                 v
    │                              IDLE
    └──────────────────────────────────
```

### Implémentation : Pattern Matching

```rust
match self.current_state {
    ChargeState::Idle => {
        if battery_level <= self.config.low_threshold {
            self.current_state = ChargeState::Charging;
        }
    }
    ChargeState::Charging => {
        if battery_level >= self.config.high_threshold {
            self.current_state = ChargeState::Idle;
        }
    }
    // ...
}
```

**Avantage vs if/else :**
- Le compilateur vérifie qu'on gère tous les états
- Si on ajoute un état, le code ne compile plus tant qu'on ne l'a pas géré partout

### Hystérésis : Éviter les oscillations

**Problème sans hystérésis :**
```
Batterie à 20.1% → Charge ON
Batterie monte à 20.2% → ???
Batterie redescend à 20.1% → ???
➡️ Oscillation infinie autour du seuil
```

**Solution avec hystérésis :**
```
Batterie à 20% → Charge ON
Batterie monte à 21%, 22%, ... → Reste en charge
Batterie atteint 90% → Charge OFF
Batterie descend à 89%, 88% → Reste en décharge
Batterie atteint 88% (90 - 2 hystérésis) → Retour IDLE
Batterie descend à 20% → Charge ON
```

---

## 5. Gestion d'erreurs

### Philosophie Rust : Pas d'exceptions

**Autres langages :**
```python
# Python
def divide(a, b):
    return a / b  # Exception si b == 0

try:
    result = divide(10, 0)
except ZeroDivisionError:
    print("Erreur")
```

**Rust :**
```rust
// Rust
fn divide(a: i32, b: i32) -> Result<i32, String> {
    if b == 0 {
        Err("Division par zéro".to_string())
    } else {
        Ok(a / b)
    }
}

// L'appelant EST FORCÉ de gérer l'erreur
match divide(10, 0) {
    Ok(result) => println!("Résultat : {}", result),
    Err(e) => println!("Erreur : {}", e),
}
```

### Pattern : L'opérateur `?`

```rust
// Sans `?` : verbeux
fn do_something() -> Result<(), MyError> {
    let x = match step1() {
        Ok(val) => val,
        Err(e) => return Err(e),
    };
    
    let y = match step2(x) {
        Ok(val) => val,
        Err(e) => return Err(e),
    };
    
    Ok(())
}

// Avec `?` : concis
fn do_something() -> Result<(), MyError> {
    let x = step1()?;  // Si Err, retourne immédiatement
    let y = step2(x)?;
    Ok(())
}
```

### Dans notre projet

```rust
pub async fn turn_on(&self) -> Result<()> {
    // Si send_command retourne Err, la fonction retourne Err
    self.send_command(&self.config.url_turn_on(), "Allumage").await
}
```

**Avantage :** Impossible d'oublier de gérer une erreur (le compilateur refuse).

### Bibliothèque : anyhow

```rust
use anyhow::{Context, Result};

fn read_file() -> Result<String> {
    std::fs::read_to_string("config.toml")
        .context("Impossible de lire config.toml")?
    // .context() ajoute du contexte à l'erreur
}

// Erreur affichée :
// Error: Impossible de lire config.toml
// Caused by:
//     No such file or directory (os error 2)
```

---

## 6. Choix des bibliothèques (crates)

### Tokio : Runtime async

**Alternatives :**
- `async-std` : Plus simple mais moins mature
- `smol` : Plus léger mais moins de fonctionnalités

**Pourquoi Tokio ?**
- Standard de facto (95% des projets async)
- Très performant (utilisé par Discord, AWS, etc.)
- Documentation excellente
- Écosystème riche (reqwest, etc. sont basés sur tokio)

### Reqwest : Client HTTP

**Alternatives :**
- `ureq` : Bloquant (pas async)
- `surf` : Basé sur async-std

**Pourquoi reqwest ?**
- Async-first (compatible tokio)
- API simple et ergonomique
- Support JSON natif
- Utilisé par 50% des projets Rust

### Serde : Sérialisation

**Alternatives :** Aucune viable

**Pourquoi serde ?**
- Standard universel en Rust
- Support tous les formats (JSON, TOML, YAML, etc.)
- Macros `#[derive(Serialize, Deserialize)]` génèrent le code automatiquement
- Ultra performant (zero-copy deserialization)

### Sysinfo : Infos système

**Alternatives :**
- `battery` crate : Spécialisée batteries mais limitée
- APIs système directes : Plus de contrôle mais non portable

**Pourquoi sysinfo ?**
- Cross-platform (Linux, Windows, macOS)
- API simple
- Bien maintenue

**Limitation connue :** Pas toujours précis sur Raspberry Pi avec UPS HAT.  
**Solution :** Fallback sur lecture `/sys/class/power_supply/` directement.

### Log + env_logger : Logging

**Alternatives :**
- `tracing` : Plus avancé (distributed tracing) mais overkill ici
- `println!` : Pas de niveaux, pas de filtrage

**Pourquoi log + env_logger ?**
- `log` = trait standard (interface)
- `env_logger` = implémentation simple
- Séparation interface/implémentation → on peut changer d'implémentation facilement

---

## 7. Configuration externe

### Pourquoi TOML et pas JSON/YAML ?

```toml
# TOML : Lisible, commentaires natifs
[battery]
low_threshold = 20  # Seuil de charge basse
```

```json
// JSON : Pas de commentaires standard
{
  "battery": {
    "low_threshold": 20
  }
}
```

```yaml
# YAML : Indentation significative (source d'erreurs)
battery:
  low_threshold: 20
```

**Choix TOML :**
- Conçu pour être édité à la main
- Commentaires natifs
- Types clairs (pas d'ambiguïté string/number)
- Standard Rust (utilisé par Cargo.toml)

### Validation à la compilation vs runtime

```rust
#[derive(Deserialize)]
pub struct BatteryConfig {
    pub critical_threshold: u8,  // u8 = 0-255, validé par serde
    pub low_threshold: u8,
    pub high_threshold: u8,
}

impl BatteryConfig {
    pub fn validate(&self) -> Result<(), String> {
        // Validation logique métier
        if self.critical_threshold >= self.low_threshold {
            return Err("critical < low attendu".into());
        }
        Ok(())
    }
}
```

**2 niveaux de validation :**
1. **Types** (u8, String, etc.) : Serde refuse si mauvais type
2. **Logique** (seuils cohérents) : Notre code valide

---

## 8. Sécurité et robustesse

### Protection contre les panics

```rust
// ❌ MAUVAIS : panic si erreur
let level = battery_info.level.unwrap();

// ✅ BON : gérer proprement
let level = match battery_monitor.get_battery_info() {
    Ok(info) => info.level,
    Err(e) => {
        error!("Erreur batterie : {}", e);
        continue;  // Passer à l'itération suivante
    }
};
```

### Retry logic avec backoff

```rust
for attempt in 1..=max_retries {
    match send_request().await {
        Ok(response) => return Ok(response),
        Err(e) => {
            warn!("Tentative {}/{} échouée", attempt, max_retries);
            if attempt < max_retries {
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }
}
```

**Pourquoi ?**
- Réseau peut être temporairement indisponible
- Mieux vaut réessayer que crasher

### Délai de grâce avant shutdown

```rust
warn!("Shutdown dans {} secondes...", grace_period);
tokio::time::sleep(Duration::from_secs(grace_period)).await;
perform_shutdown().await;
```

**Pourquoi ?**
- Permet de logger proprement
- Temps de fermer les fichiers
- Dernière chance de sauvegarder l'état

### Tests unitaires systématiques

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transition_to_emergency() {
        let mut controller = StateController::new(...);
        controller.update_state(18);  // < critical_threshold
        assert!(controller.is_emergency());
    }
}
```

**Avantage :**
- Détecte les régressions
- Documente le comportement attendu
- Donne confiance pour refactorer

---

## Conclusion

Chaque décision dans ce projet a été prise pour :

1. **Fiabilité** : Rust + tests + gestion d'erreurs → système robuste
2. **Efficacité** : Async + optimisations → faible consommation RAM/CPU
3. **Maintenabilité** : Modules + SoC → facile à modifier
4. **Apprentissage** : Code commenté + patterns → comprendre Rust

Le code est volontairement **sur-documenté** pour que vous puissiez :
- Comprendre chaque ligne
- Modifier selon vos besoins
- Apprendre les patterns Rust
- Réutiliser dans d'autres projets

**Prochain niveau :** Lisez le code en parallèle de "The Rust Book" pour approfondir !
