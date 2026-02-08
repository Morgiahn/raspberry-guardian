# Battery Guardian

**Gestionnaire automatique de batterie pour Raspberry Pi**  
*Écrit en Rust*

---

## Bienvenue !

Ce projet est un **daemon Rust complet** pour gérer automatiquement  
la charge d’un Raspberry Pi avec **une batterie** et **une Shelly Plug S**.

Projet **hyper-documenté** pour apprendre **Rust en pratiquant**.

---

## Par où commencer ?

1. **PROJECT_SUMMARY.md**  
   → Vue d’ensemble (≈ 5 min)

2. **README.md**  
   → Documentation complète

3. **QUICKSTART.md**  
   → Installation & déploiement rapide

4. **LEARNING_PATH.md**  
   → Parcours progressif pour apprendre Rust

---

## Structure du projet

### Documentation
- `README.md` — Guide complet  
- `PROJECT_SUMMARY.md` — Résumé global  
- `QUICKSTART.md` — Démarrage rapide  
- `LEARNING_PATH.md` — Apprentissage Rust  
- `ARCHITECTURE.md` — Architecture technique  

### Code source (`src/`)
- `config.rs` — Configuration (commence ici)  
- `shelly.rs` — Contrôle Shelly Plug S  
- `battery.rs` — Lecture batterie Linux  
- `network.rs` — Surveillance Wi-Fi  
- `state.rs` — Machine à états  
- `main.rs` — Point d’entrée  

### Configuration
- `Cargo.toml` — Dépendances Rust  
- `config.toml` — Configuration runtime  
- `Makefile` — Build / déploiement  
- `battery-guardian.service` — Service systemd  
- `.gitignore` — Fichiers ignorés  

---

## Débutant en Rust ?

Projet **idéal pour apprendre** :

- Code commenté ligne par ligne  
- 73+ concepts Rust expliqués  
- Progression débutant → avancé  
- Tests unitaires pédagogiques  
- Projet réel et utile

---

## Juste l’utiliser ?

1. Ouvre `QUICKSTART.md`  
2. Suis les étapes  
3. Déploie sur le Raspberry Pi  
4. Terminé

---

## Commandes rapides

```bash
# Compiler
make build

# Tester
make test

# Déployer
make deploy PI_HOST=192.168.1.100

# Logs
make logs-follow PI_HOST=192.168.1.100

# Aide
make help
```

## Statistiques

- **Lignes de code** : ~2 685  
- **Lignes de documentation** : ~2 000  
- **Concepts Rust expliqués** : 73+  
- **Tests unitaires** : 15+  
- **Fichiers de documentation** : 5

---

## Ce que tu vas apprendre

### Débutant
- Ownership & Borrowing  
- `Result` et `Option`  
- Structs et Enums  
- Pattern Matching

### Intermédiaire
- Async / Await  
- Itérateurs et Closures  
- Traits et Generics

### Avancé
- `Arc<Mutex<T>>`  
- State Machine Pattern  
- Tokio runtime  
- Cross-compilation

---

## Fonctionnalités

- Surveillance batterie automatique  
- Contrôle prise Shelly Plug S  
- Cycle charge / décharge intelligent  
- Monitoring Wi-Fi avec timeout  
- Shutdown d’urgence si critique  
- Service systemd  
- Logging structuré  
- Configuration externe

---

## Besoin d’aide ?

1. Lire les fichiers `.md`  
2. Examiner les commentaires du code  
3. The Rust Book : <https://doc.rust-lang.org/book/>  
4. r/rust ou Discord Rust

---

## C’est parti !

Commence par **PROJECT_SUMMARY.md**

Puis choisis ton parcours :
- Apprendre Rust → `LEARNING_PATH.md`  
- Déployer rapidement → `QUICKSTART.md`  
- Tout comprendre → `README.md`

---

**Bon apprentissage de Rust**  
**Bonne gestion de ta batterie**
