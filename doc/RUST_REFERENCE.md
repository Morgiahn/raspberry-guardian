# 📚 Référence Rust pour Battery Guardian

Ce document liste tous les concepts Rust utilisés dans le projet avec exemples concrets tirés du code.

---

## Table des matières

1. [Ownership, Borrowing, Lifetime](#1-ownership-borrowing-lifetime)
2. [Types et Enums](#2-types-et-enums)
3. [Result et Option](#3-result-et-option)
4. [Traits](#4-traits)
5. [Async/Await](#5-asyncawait)
6. [Smart Pointers](#6-smart-pointers)
7. [Macros](#7-macros)
8. [Pattern Matching](#8-pattern-matching)

---

## 1. Ownership, Borrowing, Lifetime

### Ownership de base

```rust
// Dans src/battery.rs
pub struct BatteryMonitor {
    system: System,  // BatteryMonitor POSSÈDE system
}

impl BatteryMonitor {
    pub fn new() -> Self {
        let mut system = System::new_all();  // system créé ici
        Self { system }  // Ownership transféré à Self
    }  // system original n'existe plus ici
}
```

**Règle :** Chaque valeur a un propriétaire unique. Quand le propriétaire est détruit, la valeur l'est aussi.

### Borrowing (références)

```rust
// Dans src/shelly.rs
async fn send_command(&self, url: &str, action: &str) -> Result<()> {
    //                         ^^^^        ^^^^
    //                    Emprunte url   Emprunte action
    //                    (ne prend PAS ownership)
}

// Appelant garde ownership
let url = self.config.url_turn_on();
self.send_command(&url, "Allumage").await;
//                ^^^^
//            Emprunte (borrow) temporairement
println!("{}", url);  // url existe toujours ici
```

**Règle :** On peut avoir soit :
- Un emprunt mutable : `&mut T`
- N emprunts immutables : `&T`
Mais pas les deux en même temps.

### Références mutables

```rust
// Dans src/network.rs
pub fn record_failure(&mut self) {
    //                ^^^^
    //           Emprunt mutable de self
    
    self.consecutive_failures += 1;  // Modification autorisée
}

// Appelant :
let mut monitor = NetworkMonitor::new(...);
monitor.record_failure();  // OK
```

### Lifetime (durée de vie)

```rust
// Lifetime implicite (Rust l'infère)
fn get_status(&self) -> Result<ShellyRelayStatus> {
    // self et le Result ont des lifetimes
    // Rust sait que Result ne peut pas vivre plus longtemps que self
}

// Lifetime explicite (rare dans notre projet)
fn longest<'a>(x: &'a str, y: &'a str) -> &'a str {
    if x.len() > y.len() { x } else { y }
}
// 'a signifie : "la référence retournée vit aussi longtemps que x et y"
```

**Dans notre projet :** Rust infère automatiquement les lifetimes, on n'a pas besoin de les écrire explicitement.

---

## 2. Types et Enums

### Structs

```rust
// Dans src/config.rs
#[derive(Debug, Clone)]
pub struct ShellyConfig {
    pub ip: String,          // String = chaîne sur le heap
    pub timeout_seconds: u64, // u64 = entier non signé 64-bit
    pub max_retries: u32,
}
```

**Types numériques :**
- `u8`, `u16`, `u32`, `u64`, `u128` : Entiers non signés (≥ 0)
- `i8`, `i16`, `i32`, `i64`, `i128` : Entiers signés (peuvent être négatifs)
- `f32`, `f64` : Flottants (nombres à virgule)

### Enums simples

```rust
// Dans src/state_machine.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChargeState {
    Idle,
    Charging,
    Emergency,
}

// Utilisation
let state = ChargeState::Idle;
```

### Enums avec données (non utilisé ici mais utile à connaître)

```rust
enum IpAddr {
    V4(u8, u8, u8, u8),      // IPv4 : 4 octets
    V6(String),              // IPv6 : String
}

let home = IpAddr::V4(192, 168, 1, 1);
let loopback = IpAddr::V6(String::from("::1"));
```

### Option<T> (enum spécial)

```rust
// Dans src/config.rs
pub struct LoggingConfig {
    pub level: String,
    pub file: Option<String>,  // Peut être Some(path) ou None
}

// Utilisation
match config.logging.file {
    Some(path) => println!("Log vers {}", path),
    None => println!("Log vers stdout"),
}
```

---

## 3. Result et Option

### Result<T, E>

```rust
// Dans src/config.rs
pub fn from_file(path: &str) -> anyhow::Result<Self> {
    //                          ^^^^^^^^^^^^^^^^^^^
    //                          Result<Self, anyhow::Error>
    
    let content = std::fs::read_to_string(path)?;
    //                                         ^
    //            Si Err, retourne Err immédiatement
    
    let config: Config = toml::from_str(&content)?;
    Ok(config)  // Succès : wrap dans Ok()
}
```

**L'opérateur `?` :**
```rust
// Sans ?
let result = read_file();
let content = match result {
    Ok(c) => c,
    Err(e) => return Err(e),
};

// Avec ?
let content = read_file()?;
```

### Option<T>

```rust
// Exemple non présent dans le code mais fréquent
let numbers = vec![1, 2, 3];
let first = numbers.first();  // Option<&i32>

match first {
    Some(num) => println!("Premier : {}", num),
    None => println!("Liste vide"),
}

// Ou plus court
if let Some(num) = numbers.first() {
    println!("Premier : {}", num);
}
```

---

## 4. Traits

### Traits dérivés (derive)

```rust
// Dans src/state_machine.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
//       ^^^^^  ^^^^^  ^^^^  ^^^^^^^^^  ^^
pub enum ChargeState { /* ... */ }
```

**Signification :**
- `Debug` : Permet `println!("{:?}", state)`
- `Clone` : Permet `let copy = state.clone()`
- `Copy` : Permet copie implicite (types simples seulement)
- `PartialEq` : Permet `state1 == state2`
- `Eq` : Indique que l'égalité est réflexive, symétrique, transitive

### Implémentation de traits

```rust
// Dans src/config.rs
impl ShellyConfig {
    pub fn timeout(&self) -> Duration {
        Duration::from_secs(self.timeout_seconds)
    }
}

// `impl` ajoute des méthodes à une struct
```

### Traits de bibliothèques

```rust
// Dans src/config.rs
#[derive(Deserialize, Serialize)]
//       ^^^^^^^^^^^  ^^^^^^^^^
//       Trait serde pour parser TOML
pub struct Config { /* ... */ }
```

---

## 5. Async/Await

### Fonctions async

```rust
// Dans src/shelly.rs
pub async fn turn_on(&self) -> Result<()> {
//     ^^^^^
//  Mot-clé qui transforme la fonction en Future
    self.send_command(&self.config.url_turn_on(), "Allumage").await
    //                                                         ^^^^^
    //                                            Attend que send_command se termine
}
```

**Fonctions async retournent des Futures :**
```rust
// Définition
async fn fetch() -> String { "data".to_string() }

// Utilisation
let future = fetch();     // Ne s'exécute PAS encore
let result = future.await; // S'exécute maintenant
```

### #[tokio::main]

```rust
// Dans src/main.rs
#[tokio::main]
async fn main() -> Result<()> {
    // Cette macro transforme main() pour utiliser tokio
    run_daemon(config).await?;
    Ok(())
}

// Équivalent à (sans macro) :
fn main() -> Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async {
        run_daemon(config).await?;
        Ok(())
    })
}
```

### tokio::select!

```rust
// Dans src/main.rs
tokio::select! {
    result = monitor_battery(...) => { /* ... */ },
    result = monitor_network(...) => { /* ... */ },
}
// Attend que l'une des 2 branches se termine
```

### tokio::spawn

```rust
// Lancer une tâche en arrière-plan
tokio::spawn(async move {
    loop {
        // Faire quelque chose en boucle
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
});
```

---

## 6. Smart Pointers

### Arc<T> (Atomic Reference Counted)

```rust
// Dans src/main.rs
let shelly = Arc::new(ShellyController::new(...)?);
//           ^^^^^^^^
//       Compteur de références atomique (thread-safe)

let shelly_clone = shelly.clone();
//                        ^^^^^^^
//              Incrémente le compteur (pas de copie des données)

tokio::spawn(async move {
    shelly_clone.turn_on().await;  // Utilise le clone
});
// Quand toutes les références sont drop, la donnée est libérée
```

**Différence avec Rc<T> :**
- `Rc` : Non thread-safe, plus rapide
- `Arc` : Thread-safe (Atomic), utilisable en async

### Mutex<T>

```rust
// Dans src/main.rs
let state = Arc::new(Mutex::new(StateController::new(...)));
//                   ^^^^^^^^^^^^
//               Verrou pour accès mutuellement exclusif

// Utilisation
let mut s = state.lock().await;  // Acquérir le verrou
s.update_state(battery_level);   // Modifier
// Le verrou est libéré automatiquement quand `s` est drop
```

**std::sync::Mutex vs tokio::sync::Mutex :**
```rust
// MAUVAIS : std::sync::Mutex : bloque le thread
let s = state.lock().unwrap();  // Bloque jusqu'à obtenir le verrou

// BON : tokio::sync::Mutex : async
let s = state.lock().await;  // Libère le thread pendant l'attente
```

### Box<T> (non utilisé ici mais important)

```rust
// Allouer sur le heap
let b = Box::new(5);
// Utile pour types récursifs ou très gros
```

---

## 7. Macros

### Macros de logging

```rust
// Dans src/main.rs
info!("Démarrage de Battery Guardian");
warn!("Shutdown dans {} secondes", grace_period);
error!("Prise non accessible");
debug!("Mode urgence maintenu");

// Équivalent à :
log::log!(log::Level::Info, "Démarrage...");
```

### Macros de format

```rust
// format! : créer une String
let msg = format!("Batterie : {}%", level);

// println! : afficher à l'écran
println!("État : {:?}", state);

// Différences :
// {} : Display trait
// {:?} : Debug trait
```

### Macros de tests

```rust
// Dans src/state_machine.rs
#[cfg(test)]
mod tests {
    #[test]
    fn test_transition() {
        assert_eq!(state, ChargeState::Charging);
        //         Vérifie égalité
        
        assert!(state_changed);
        //      Vérifie true
    }
}
```

---

## 8. Pattern Matching

### Match de base

```rust
// Dans src/state_machine.rs
match self.current_state {
    ChargeState::Idle => {
        if battery_level <= self.config.low_threshold {
            self.current_state = ChargeState::Charging;
        }
    }
    ChargeState::Charging => { /* ... */ }
    ChargeState::Emergency => { /* ... */ }
}
```

**Le compilateur force à gérer tous les cas :**
```rust
// ❌ ERREUR : oubli d'un cas
match state {
    ChargeState::Idle => {},
    ChargeState::Charging => {},
    // ERREUR : non-exhaustive patterns
}
```

### Match avec Result

```rust
// Dans src/main.rs
match battery_monitor.get_battery_info() {
    Ok(info) => {
        println!("Batterie : {}%", info.level);
    }
    Err(e) => {
        error!("Erreur : {}", e);
        continue;
    }
}
```

### if let (match simplifié)

```rust
// Au lieu de :
match numbers.first() {
    Some(n) => println!("{}", n),
    None => {},
}

// On peut écrire :
if let Some(n) = numbers.first() {
    println!("{}", n);
}
```

### matches! macro

```rust
// Dans src/state_machine.rs
pub fn should_charge(&self) -> bool {
    matches!(self.current_state, ChargeState::Charging)
    //       Vérifie si current_state est Charging
}

// Équivalent à :
match self.current_state {
    ChargeState::Charging => true,
    _ => false,
}
```

---

## Exercices pratiques

### Exercice 1 : Ownership

**Question :** Ce code compile-t-il ?
```rust
let s1 = String::from("hello");
let s2 = s1;
println!("{}", s1);
```

<details>
<summary>Réponse</summary>

❌ Non. `s1` a été déplacé (moved) vers `s2`. Il faut :
```rust
let s1 = String::from("hello");
let s2 = s1.clone();  // Cloner
println!("{}", s1);
```
</details>

### Exercice 2 : Result

**Question :** Réécrire avec l'opérateur `?` :
```rust
fn do_something() -> Result<i32, String> {
    let x = match step1() {
        Ok(val) => val,
        Err(e) => return Err(e),
    };
    Ok(x * 2)
}
```

<details>
<summary>Réponse</summary>

```rust
fn do_something() -> Result<i32, String> {
    let x = step1()?;
    Ok(x * 2)
}
```
</details>

### Exercice 3 : Async

**Question :** Comment exécuter 3 tâches async en parallèle ?

<details>
<summary>Réponse</summary>

```rust
tokio::join!(
    task1(),
    task2(),
    task3(),
);
// Les 3 s'exécutent en parallèle
```
</details>

---

## Ressources

- **The Rust Book** : https://doc.rust-lang.org/book/
- **Rust by Example** : https://doc.rust-lang.org/rust-by-example/
- **Tokio docs** : https://tokio.rs/
- **Rustlings** (exercices) : https://github.com/rust-lang/rustlings

---

**Prochain niveau :** Lire le code source ligne par ligne et expérimenter !
