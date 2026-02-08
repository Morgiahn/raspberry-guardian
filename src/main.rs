// ============================================================================
// BATTERY GUARDIAN - Gestionnaire de batterie pour Raspberry Pi
// ============================================================================
//! # Architecture du programme
//! 
//! Ce programme surveille la batterie d'un Raspberry Pi et gère automatiquement
//! les cycles de charge/décharge via une prise connectée Shelly Plug S.
//! 
//! ## Modules
//! - `config` : Gestion de la configuration (fichier TOML)
//! - `battery` : Surveillance de l'état de la batterie
//! - `network` : Surveillance de la connectivité WiFi
//! - `shelly` : Contrôle de la prise Shelly Plug S
//! - `state_machine` : Logique de gestion des états (charge/décharge)
//! 
//! ## Flux principal
//! 1. Charger la configuration depuis config.toml
//! 2. Initialiser les moniteurs (batterie, réseau)
//! 3. Lancer les tâches asynchrones en parallèle :
//!    - Surveillance batterie (toutes les N secondes)
//!    - Surveillance réseau (toutes les M secondes)
//! 4. Gérer les transitions d'état et contrôler la prise
//! 5. En cas d'urgence : shutdown propre du système

// Déclaration des modules
// `mod` indique au compilateur d'inclure ces fichiers
mod battery;
mod config;
mod shelly;
mod state_machine;

mod app_state;
use crate::app_state::AppState;

mod api;
mod system;

// indique à Rust que src/api/mod.rs existe
use api::ApiError;


// Imports
use anyhow::{Context, Result};
use log::{error, info, warn};
use std::process::Command;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{interval, sleep, Duration};

use battery::BatteryMonitor;
use config::Config;
use shelly::ShellyController;
use state_machine::StateController;

use crate::system::shutdown_with_grace;


// ============================================================================
// POINT D'ENTRÉE DU PROGRAMME
// ============================================================================

/// Fonction principale
/// 
/// # Attributs importants
/// - `#[tokio::main]` : Macro qui transforme `main` en runtime async
///   Sans cette macro, on ne pourrait pas utiliser `async/await`
/// 
/// # Flux
/// 1. Initialiser le logger
/// 2. Charger la config
/// 3. Lancer le daemon
#[tokio::main]
async fn main() -> Result<()> {
    // Initialiser le système de logging
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    info!("Démarrage de Battery Guardian");

    // Charger la configuration
    let config = Config::from_file("config.toml")
        .context("Impossible de charger la configuration")?;

    info!("Configuration chargée avec succès");
    info!("   - Shelly IP: {}", config.shelly.ip);
    info!("   - Seuil critique: {}%", config.battery.critical_threshold);
    info!("   - Seuil bas: {}%", config.battery.low_threshold);
    info!("   - Seuil haut: {}%", config.battery.high_threshold);

    // Initialiser composants
    let battery_monitor = Arc::new(Mutex::new(BatteryMonitor::new()));
    let shelly_controller = Arc::new(ShellyController::new(config.shelly.clone())?);
    let state_controller = Arc::new(Mutex::new(StateController::new(config.battery.clone())));

    let app_state = AppState {
        state_controller,
        shelly: shelly_controller.clone(),
        config: Arc::new(config.clone()),
        battery_monitor
    };

    // Lancer API en arrière-plan
    tokio::spawn(api::start_api_server(app_state.clone()));


    // Lancer le daemon principal en lui passant AppState
    run_daemon(app_state.clone()).await?;

    Ok(())
}

// ============================================================================
// DAEMON PRINCIPAL
// ============================================================================

/// Daemon principal qui orchestre toutes les tâches
/// 
/// # Architecture avec Arc<Mutex<T>>
/// 
/// ## Problème
/// En Rust, on ne peut pas partager une variable mutable entre plusieurs threads.
/// Le compilateur refuse ce code :
/// ```ignore
/// let mut state = StateController::new(...);
/// tokio::spawn(async { state.update(...) }); // ERREUR
/// tokio::spawn(async { state.update(...) }); // ERREUR
/// ```
/// 
/// ## Solution : Arc<Mutex<T>>
/// - `Arc` (Atomic Reference Counted) : permet de partager ownership entre threads
/// - `Mutex` : assure qu'un seul thread accède à la donnée à la fois
/// 
/// Exemple :
/// ```ignore
/// let state = Arc::new(Mutex::new(StateController::new(...)));
/// let state_clone = state.clone();  // Incrémenter le compteur de références
/// tokio::spawn(async move { 
///     let mut s = state_clone.lock().await;  // Acquérir le lock
///     s.update(...);  // Modifier
/// });  // Le lock est libéré automatiquement ici
/// ```
async fn run_daemon(
    app_state: AppState,
    // battery_monitor: Arc<Mutex<BatteryMonitor>>,
) -> anyhow::Result<()> {


    // Restaurer état si fichier de persistance configuré
    if let Some(ref path) = app_state.config.system.state_file {
        if let Err(e) = app_state.state_controller.lock().await.restore_state(path) {
            warn!("Impossible de restaurer l'état depuis {}: {}", path, e);
        }
    } else {
        warn!("Aucun fichier de persistance configuré (system.state_file). L'état ne sera pas restauré au démarrage.");
    }

    info!("Tous les composants initialisés");

    // Lancer les tâches asynchrones
    tokio::select! {
        result = monitor_battery(
            app_state.battery_monitor.clone(),
            app_state.shelly.clone(),
            app_state.state_controller.clone(),
            (*app_state.config).clone(),
        ) => {
            if let Err(e) = result {
                error!("Erreur dans la surveillance batterie : {}", e);
            }
        }
    }

    Ok(())
}

// ============================================================================
// TÂCHE : SURVEILLANCE BATTERIE
// ============================================================================

/// Tâche asynchrone qui surveille la batterie en continu
/// 
/// # Boucle infinie
/// Cette fonction tourne en boucle infinie (`loop`) et ne se termine jamais
/// (sauf en cas d'erreur ou shutdown)
async fn monitor_battery(
    battery_monitor: Arc<Mutex<BatteryMonitor>>,
    shelly: Arc<ShellyController>,
    state_controller: Arc<Mutex<StateController>>,
    config: Config,
) -> Result<()> {
    // Créer un timer qui tick toutes les N secondes
    let mut ticker = interval(config.battery.check_interval());

    let mut battery_error_count :u8 = 0;

    loop {
        // Attendre le prochain tick
        ticker.tick().await;

        // Lire le niveau de batterie
        let battery_info = {
            let monitor = battery_monitor.lock().await;
            match monitor.get_battery_info() {
                Ok(info) => info,
                Err(e) => {
                    battery_error_count += 1;
                    error!("Impossible de lire la batterie ({}) : {}", battery_error_count, e);
                    // if battery_error_count >= 3 {
                    //     error!("Impossible de lire la batterie après plusieurs tentatives, arrêt du programme");
                    //     return Err(anyhow::anyhow!("Impossible de lire la batterie après plusieurs tentatives"));
                    // }
                    continue;  // Passer à l'itération suivante
                }
            }
        };

        info!(
            "Batterie : {}% ({})",
            battery_info.level, battery_info.status
        );

        // Mettre à jour la machine à états
        let (state_changed, current_state, should_charge, is_emergency) = {
            let mut controller = state_controller.lock().await;
            let changed = controller.update_state(battery_info.level);
            (
                changed,
                controller.current_state(),
                controller.should_charge(),
                controller.is_emergency(),
            )
        };

        // URGENCE : Batterie critique
        if is_emergency {
            error!("BATTERIE CRITIQUE ({}%) - ARRÊT D'URGENCE", battery_info.level);

            // Vérifier si la prise est accessible
            if shelly.is_reachable().await {
                warn!("Tentative d'allumage de la prise en urgence...");
                if let Err(e) = shelly.turn_on().await {
                    error!("Impossible d'allumer la prise : {}", e);
                }
            } else {
                error!("Prise non accessible - shutdown imminent");
            }

            // Attendre le délai de grâce puis shutdown
            shutdown_with_grace(&config).await?;
            break;  // Sortir de la boucle (même si shutdown devrait tout arrêter)
        }

        // Contrôler la prise selon l'état
        if state_changed {
            if should_charge {
                info!("Activation de la charge");
                if let Err(e) = shelly.turn_on().await {
                    error!("Erreur lors de l'allumage de la prise : {}", e);
                }
            } else {
                info!("Désactivation de la charge");
                if let Err(e) = shelly.turn_off().await {
                    error!("Erreur lors de l'extinction de la prise : {}", e);
                }
            }

            // Sauvegarder l'état si demandé
            if let Some(ref path) = config.system.state_file {
                if let Err(e) = state_controller.lock().await.save_state(path) {
                    error!("Erreur lors de la sauvegarde de l'état: {}", e);
                }
            }
        }

        // Afficher l'état actuel
        info!("État actuel : {:?}", current_state);
    }

    Ok(())
}


// ============================================================================
// FONCTION : SHUTDOWN DU SYSTÈME
// ============================================================================

/// Exécute le shutdown du système
/// 
/// # Sécurité
/// Cette fonction utilise `sudo shutdown -h now` par défaut
/// Sur un système de production, s'assurer que l'utilisateur a les droits sudo
/// 
/// # Configuration pour sudo sans mot de passe
/// Ajouter dans `/etc/sudoers` (via `sudo visudo`) :
/// ```
/// pi ALL=(ALL) NOPASSWD: /sbin/shutdown
/// ```
async fn perform_shutdown(config: &Config) -> Result<()> {
    warn!("EXÉCUTION DU SHUTDOWN : {}", config.system.shutdown_command);

    // Diviser la commande en programme + arguments
    let parts: Vec<&str> = config.system.shutdown_command.split_whitespace().collect();
    
    if parts.is_empty() {
        error!("Commande de shutdown vide");
        return Ok(());
    }

    let program = parts[0];
    let args = &parts[1..];

    // Exécuter la commande
    match Command::new(program).args(args).spawn() {
        Ok(_) => {
            info!("Commande de shutdown lancée");
        }
        Err(e) => {
            error!("Erreur lors du shutdown : {}", e);
            error!("   Vérifiez les permissions sudo");
        }
    }

    Ok(())
}

// ============================================================================
// TESTS D'INTÉGRATION
// ============================================================================

#[cfg(test)]
mod tests {
    #[test]
    fn test_config_loading() {
        // Ce test vérifie que le fichier config.toml est valide
        // En vrai, on utiliserait un fichier de config de test
        
        // Pour l'instant, juste vérifier que le module compile
        assert!(true);
    }
}
