// ============================================================================
// MODULE DE MACHINE À ÉTATS
// ============================================================================
// Ce module gère la logique de cycle charge/décharge avec une state machine

use log::{debug, info, warn};
use crate::config::BatteryConfig;
use serde::{Serialize, Deserialize};
use anyhow::Result;
use std::path::Path;
use chrono::Local;

/// États possibles du système de gestion de batterie
///
/// # Pattern : State Machine (Machine à états)
/// Une state machine est un pattern où le comportement dépend de l'état actuel
/// et des transitions sont définies entre états
///
/// # Pourquoi enum ?
/// En Rust, `enum` permet de définir un type qui peut être dans un état parmi N
/// C'est plus sûr que des constantes (CHARGING = 1, DISCHARGING = 2, ...)
/// car le compilateur force à gérer tous les cas
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChargeState {
    /// Mode IDLE : batterie chargée ou stable
    /// La prise est éteinte, le Pi fonctionne sur batterie
    Idle,

    /// Mode CHARGE : batterie basse, en cours de charge
    /// La prise est allumée, la batterie se recharge
    Charging,


    /// Mode URGENCE : batterie critique, préparation shutdown
    /// État temporaire avant extinction du système
    Emergency,


}

#[derive(Clone, Debug)]
pub enum ForceMode {
    Charge,
    Discharge,
    Stop,
}


/// Snapshot minimal pour persister l'état
#[derive(Debug, Serialize, Deserialize)]
struct StateSnapshot {
    current_state: ChargeState,
    charge_cycles: u32,
    /// Timestamp ISO8601 de la dernière mise à jour
    last_updated: String,
}

/// Contrôleur de la machine à états
///
/// # Responsabilités
/// - Gérer les transitions entre états
/// - Implémenter l'hystérésis pour éviter les oscillations
/// - Logger les changements d'état
pub struct StateController {
    current_state: ChargeState,
    config: BatteryConfig,
    charge_cycles: u32,
    forced_mode: Option<ForceMode>,
}

impl StateController {
    pub fn set_forced_mode(&mut self, mode: Option<ForceMode>) {
        self.forced_mode = mode;
    }

    pub fn forced_mode(&self) -> Option<ForceMode> {
        self.forced_mode.clone()
    }

    pub fn clear_forced_mode(&mut self) {
        warn!("Retour en mode automatique");
        self.forced_mode = None;
    }
}


impl StateController {
    /// Crée un nouveau contrôleur, initialement en mode Idle
    pub fn new(config: BatteryConfig) -> Self {
        info!("Initialisation du contrôleur d'état en mode Idle");
        Self {
            current_state: ChargeState::Idle,
            config,
            charge_cycles: 0,
            forced_mode: None,
        }
    }

    /// Retourne l'état actuel
    pub fn current_state(&self) -> ChargeState {
        self.current_state
    }

    /// Restaurer l'état à partir d'un fichier JSON (si disponible)
    pub fn restore_state<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        let p = path.as_ref();
        if !p.exists() {
            warn!("Fichier d'état inexistant ({}), démarrage avec état par défaut.", p.display());
            return Ok(());
        }
        let content = std::fs::read_to_string(p)?;

        let snap: StateSnapshot = match serde_json::from_str(&content) {
            Ok(s) => s,
            Err(e) => {
                warn!("Fichier d'état corrompu ({}): {}", p.display(), e);
                return Ok(()); // On ignore et on démarre proprement
            }
        };

        self.current_state = snap.current_state;
        self.charge_cycles = snap.charge_cycles;

        info!(
            "État restauré depuis {} : {:?}, cycles={}",
            p.display(),
            self.current_state,
            self.charge_cycles
        );
        Ok(())
    }

    /// Sauvegarder l'état courant dans un fichier JSON
    pub fn save_state<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let snap = StateSnapshot {
            current_state: self.current_state,
            charge_cycles: self.charge_cycles,
            last_updated: Local::now().to_rfc3339(),
        };
        let s = serde_json::to_string_pretty(&snap)?;
        std::fs::write(path.as_ref(), s)?;
        Ok(())
    }

    /// Évalue et met à jour l'état en fonction du niveau de batterie
    ///
    /// # Logique de transition
    ///
    /// ```text
    /// IDLE ──┐
    ///   ↑    │ battery ≤ low_threshold
    ///   │    ↓
    ///   └─ CHARGING
    ///        │
    ///        │ battery ≥ high_threshold
    ///        ↓
    ///       IDLE
    ///
    /// battery ≤ critical_threshold → EMERGENCY (depuis n'importe quel état)
    /// ```
    ///
    /// # Logique simplifiée
    /// - En IDLE : prise éteinte, on charge si batterie ≤ low_threshold
    /// - En CHARGING : prise allumée, on retourne en IDLE si batterie ≥ high_threshold
    /// - EMERGENCY : mode critique depuis n'importe quel état
    ///
    /// # Retour
    /// true si l'état a changé, false sinon
    pub fn update_state(&mut self, battery_level: u8) -> bool {
        let old_state = self.current_state;

        // FORCE MODE overrides all logic
        if let Some(force) = &self.forced_mode {
            let target_state = match force {
                ForceMode::Charge => ChargeState::Charging,
                ForceMode::Discharge => ChargeState::Idle,
                ForceMode::Stop => ChargeState::Emergency,
            };

            if self.current_state != target_state {
                warn!(
                    "Mode forcé actif: {:?} -> {:?}",
                    self.current_state, target_state
                );
                self.current_state = target_state;
                return true;
            }

            return false;
        }


        // PRIORITÉ 1 : Vérifier le niveau critique
        if battery_level <= self.config.critical_threshold {
            self.current_state = ChargeState::Emergency;
            if old_state != self.current_state {
                warn!(
                    "BATTERIE CRITIQUE ({}%) - PASSAGE EN MODE URGENCE",
                    battery_level
                );
                return true;
            }
            return false;
        }

        // PRIORITÉ 2 : Transitions normales selon l'état actuel
        match self.current_state {
            ChargeState::Idle => {
                // Depuis IDLE, on peut aller vers CHARGING si batterie basse
                if battery_level <= self.config.low_threshold {
                    self.current_state = ChargeState::Charging;
                    info!(
                        "Batterie basse ({}% ≤ {}%) - DÉBUT DE CHARGE",
                        battery_level, self.config.low_threshold
                    );
                }
            }

            ChargeState::Charging => {
                // Depuis CHARGING, on va vers IDLE si batterie haute
                if battery_level >= self.config.high_threshold {
                    self.current_state = ChargeState::Idle;
                    info!(
                        "Batterie chargée ({}% ≥ {}%) - RETOUR EN IDLE",
                        battery_level, self.config.high_threshold
                    );
                    // Incrémenter le compteur de cycles
                    self.charge_cycles = self.charge_cycles.saturating_add(1);
                }
            }


            ChargeState::Emergency => {
                // En mode Emergency, on reste jusqu'au shutdown
                // (ou jusqu'à ce que la batterie remonte, mais c'est peu probable)
                if battery_level > self.config.low_threshold {
                    info!(
                        "Batterie remontée ({}%) - Sortie du mode urgence",
                        battery_level
                    );
                    self.current_state = ChargeState::Idle;  // Retour à Idle si batterie remonte
                } else {
                    debug!("Mode urgence maintenu (batterie à {}%)", battery_level);
                }
            }
        }

        // Retourner true si l'état a changé
        let state_changed = old_state != self.current_state;
        if state_changed {
            debug!("Transition d'état : {:?} -> {:?}", old_state, self.current_state);
        }

        state_changed
    }

    /// Indique si le système doit être en mode charge (prise ON)
    ///
    /// # Logique
    /// La prise doit être allumée UNIQUEMENT en mode CHARGING
    pub fn should_charge(&self) -> bool {
        matches!(self.current_state, ChargeState::Charging)
    }

    /// Indique si le système est en urgence
    pub fn is_emergency(&self) -> bool {
        matches!(self.current_state, ChargeState::Emergency)
    }

    /// Force un état spécifique (pour tests ou récupération)
    #[allow(dead_code)]
    pub fn force_state(&mut self, state: ChargeState) {
        warn!("Etat forcé manuellement : {:?} -> {:?}", self.current_state, state);
        self.current_state = state;
    }

    /// Retourne le nombre de cycles de charge complets enregistrés
    pub fn charge_cycles(&self) -> u32 {
        self.charge_cycles
    }
}

// ============================================================================
// TESTS UNITAIRES
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> BatteryConfig {
        BatteryConfig {
            critical_threshold: 19,
            low_threshold: 20,
            high_threshold: 90,
            hysteresis: 2,
            check_interval_seconds: 30,
        }
    }

    #[test]
    fn test_initial_state() {
        let config = create_test_config();
        let controller = StateController::new(config);
        assert_eq!(controller.current_state(), ChargeState::Idle);
    }

    #[test]
    fn test_transition_to_charging() {
        let config = create_test_config();
        let mut controller = StateController::new(config);

        // Batterie descend à 20% → devrait passer en Charging
        let changed = controller.update_state(20);
        assert!(changed);
        assert_eq!(controller.current_state(), ChargeState::Charging);
        assert!(controller.should_charge());
    }

    #[test]
    fn test_transition_to_idle_after_charging() {
        let config = create_test_config();
        let mut controller = StateController::new(config);

        // Simuler : Idle → Charging → Idle
        controller.update_state(20);  // Charging
        let changed = controller.update_state(90);  // Idle
        assert!(changed);
        assert_eq!(controller.current_state(), ChargeState::Idle);
        assert!(!controller.should_charge());
    }

    #[test]
    fn test_emergency_transition() {
        let config = create_test_config();
        let mut controller = StateController::new(config);

        // Batterie critique → Emergency
        let changed = controller.update_state(18);
        assert!(changed);
        assert_eq!(controller.current_state(), ChargeState::Emergency);
        assert!(controller.is_emergency());
    }

    #[test]
    fn test_charging_cycle() {
        let config = create_test_config();
        let mut controller = StateController::new(config);

        // Cycle complet : Idle → Charging → Idle
        assert_eq!(controller.current_state(), ChargeState::Idle);

        // Descendre à 20% : passe en Charging
        controller.update_state(20);
        assert_eq!(controller.current_state(), ChargeState::Charging);
        assert!(controller.should_charge());

        // Monter à 90% : revient en Idle
        controller.update_state(90);
        assert_eq!(controller.current_state(), ChargeState::Idle);
        assert!(!controller.should_charge());

        // Descendre à nouveau à 20% : repasse en Charging
        controller.update_state(20);
        assert_eq!(controller.current_state(), ChargeState::Charging);
    }

    #[test]
    fn test_no_oscillation() {
        let config = create_test_config();
        let mut controller = StateController::new(config);

        // Passer en Charging à 20%
        controller.update_state(20);
        assert_eq!(controller.current_state(), ChargeState::Charging);

        // Même si batterie oscille légèrement autour de 20%, on reste en Charging
        controller.update_state(21);
        assert_eq!(controller.current_state(), ChargeState::Charging);

        controller.update_state(20);
        assert_eq!(controller.current_state(), ChargeState::Charging);

        // On ne passe en Idle qu'à 90%
        controller.update_state(89);
        assert_eq!(controller.current_state(), ChargeState::Charging);

        controller.update_state(90);
        assert_eq!(controller.current_state(), ChargeState::Idle);
    }

    #[test]
    fn test_save_and_restore_state() {
        use std::fs;
        use tempfile::NamedTempFile;

        // Créer un fichier temporaire
        let temp_file = NamedTempFile::new().unwrap();
        let temp_path = temp_file.path();

        // Créer un contrôleur et effectuer quelques transitions
        let config = create_test_config();
        let mut controller1 = StateController::new(config.clone());

        // Simuler un cycle de charge complet
        controller1.update_state(20);  // Idle → Charging
        assert_eq!(controller1.current_state(), ChargeState::Charging);
        assert_eq!(controller1.charge_cycles(), 0);

        controller1.update_state(90);  // Charging → Idle
        assert_eq!(controller1.current_state(), ChargeState::Idle);
        assert_eq!(controller1.charge_cycles(), 1);

        // Sauvegarder l'état
        controller1.save_state(temp_path).expect("Failed to save state");

        // Créer un nouveau contrôleur et restaurer l'état
        let mut controller2 = StateController::new(config);
        assert_eq!(controller2.current_state(), ChargeState::Idle);
        assert_eq!(controller2.charge_cycles(), 0);

        // Restaurer depuis le fichier
        controller2.restore_state(temp_path).expect("Failed to restore state");
        assert_eq!(controller2.current_state(), ChargeState::Idle);
        assert_eq!(controller2.charge_cycles(), 1);

        // Nettoyer
        fs::remove_file(temp_path).ok();
    }
}
