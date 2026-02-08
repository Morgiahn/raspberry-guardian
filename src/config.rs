// ============================================================================
// MODULE DE CONFIGURATION
// ============================================================================
// Ce module gère le chargement et la validation de la configuration depuis config.toml

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Configuration complète de l'application
/// 
/// # Explication du #[derive(...)]
/// - Deserialize : permet de convertir TOML → struct Rust automatiquement
/// - Serialize : permet de convertir struct → TOML (utile pour sauvegarder)
/// - Debug : permet d'afficher la struct avec {:?} (pour le logging)
/// - Clone : permet de faire des copies de la config (utile pour partager entre threads)
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    pub shelly: ShellyConfig,
    pub battery: BatteryConfig,
    pub network: NetworkConfig,
    pub logging: LoggingConfig,
    pub system: SystemConfig,
}

/// Configuration de la prise Shelly Plug S
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ShellyConfig {
    /// Adresse IP de la prise sur le réseau local
    pub ip: String,
    
    /// Timeout pour les requêtes HTTP en secondes
    pub timeout_seconds: u64,
    
    /// Nombre maximum de tentatives
    pub max_retries: u32,
}

impl ShellyConfig {
    /// Convertit le timeout en Duration (type Rust pour durées)
    /// 
    /// # Pourquoi une méthode ?
    /// config.toml stocke des nombres (u64), mais Rust utilise Duration
    /// Cette méthode fait la conversion proprement
    pub fn timeout(&self) -> Duration {
        Duration::from_secs(self.timeout_seconds)
    }
    
    /// Construit l'URL complète pour allumer la prise
    /// 
    /// # API Shelly Plug S
    /// - Allumer : http://IP/relay/0?turn=on
    /// - Éteindre : http://IP/relay/0?turn=off
    /// - Status : http://IP/relay/0
    pub fn url_turn_on(&self) -> String {
        format!("http://{}/relay/0?turn=on", self.ip)
    }
    
    pub fn url_turn_off(&self) -> String {
        format!("http://{}/relay/0?turn=off", self.ip)
    }
    
    pub fn url_status(&self) -> String {
        format!("http://{}/relay/0", self.ip)
    }
}

/// Configuration de la gestion de batterie
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BatteryConfig {
    /// Seuil critique (shutdown immédiat)
    pub critical_threshold: u8,
    
    /// Seuil bas (début de charge)
    pub low_threshold: u8,
    
    /// Seuil haut (fin de charge)
    pub high_threshold: u8,
    
    /// Hystérésis pour éviter l'oscillation
    pub hysteresis: u8,
    
    /// Intervalle de vérification en secondes
    pub check_interval_seconds: u64,
}

impl BatteryConfig {
    pub fn check_interval(&self) -> Duration {
        Duration::from_secs(self.check_interval_seconds)
    }
    
    /// Valide la cohérence des seuils
    /// 
    /// # Pourquoi ?
    /// On veut s'assurer que : critical < low < high
    /// Sinon le système ne fonctionnera pas correctement
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.critical_threshold >= self.low_threshold {
            return Err(anyhow::anyhow!("critical_threshold doit être < low_threshold"));
        }
        if self.low_threshold >= self.high_threshold {
            return Err(anyhow::anyhow!("low_threshold doit être < high_threshold"));
        }
        if self.high_threshold > 100 {
            return Err(anyhow::anyhow!("high_threshold ne peut pas dépasser 100%"));
        }
        Ok(())
    }
}

/// Configuration de la surveillance réseau
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct NetworkConfig {
    /// Nombre d'échecs consécutifs tolérés avant shutdown
    pub max_consecutive_failures: u32,
    
    /// Intervalle de vérification de la connectivité (en secondes)
    pub check_interval_seconds: u64,
}

impl NetworkConfig {
    pub fn check_interval(&self) -> Duration {
        Duration::from_secs(self.check_interval_seconds)
    }
}

/// Configuration du logging
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LoggingConfig {
    /// Niveau de log (trace, debug, info, warn, error)
    pub level: String,
    
    /// Fichier de log optionnel
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
}

/// Configuration système
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SystemConfig {
    /// Commande de shutdown
    pub shutdown_command: String,
    
    /// Délai de grâce avant shutdown (secondes)
    pub shutdown_grace_period_seconds: u64,

    pub api_ip: String,                   // ex: "0.0.0.0"
    pub api_port: u16,                    // ex: 7878

    /// Fichier de persistance de l'état (optionnel)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_file: Option<String>,
}

impl SystemConfig {
    pub fn shutdown_grace_period(&self) -> Duration {
        Duration::from_secs(self.shutdown_grace_period_seconds)
    }
}

impl Config {
    /// Charge la configuration depuis un fichier TOML
    /// 
    /// # Gestion d'erreurs en Rust
    /// Le type de retour `Result<Config, anyhow::Error>` signifie :
    /// - En cas de succès : Ok(Config)
    /// - En cas d'erreur : Err(description de l'erreur)
    /// 
    /// L'opérateur `?` propage automatiquement les erreurs
    pub fn from_file(path: &str) -> anyhow::Result<Self> {
        // Lire le contenu du fichier
        let content = std::fs::read_to_string(path)?;
        
        // Parser le TOML en struct Config
        let config: Config = toml::from_str(&content)?;
        
        // Valider la configuration
        config.battery.validate()?;
        
        Ok(config)
    }
}

// ============================================================================
// TESTS UNITAIRES
// ============================================================================
// En Rust, les tests sont dans le même fichier avec #[cfg(test)]

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_battery_validation_ok() {
        let battery = BatteryConfig {
            critical_threshold: 19,
            low_threshold: 20,
            high_threshold: 90,
            hysteresis: 2,
            check_interval_seconds: 30,
        };
        
        assert!(battery.validate().is_ok());
    }

    #[test]
    fn test_battery_validation_fail() {
        let battery = BatteryConfig {
            critical_threshold: 25,  // ERREUR : > low_threshold
            low_threshold: 20,
            high_threshold: 90,
            hysteresis: 2,
            check_interval_seconds: 30,
        };
        
        assert!(battery.validate().is_err());
    }

    #[test]
    fn test_shelly_urls() {
        let shelly = ShellyConfig {
            ip: "192.168.1.100".to_string(),
            timeout_seconds: 5,
            max_retries: 3,
        };
        
        assert_eq!(shelly.url_turn_on(), "http://192.168.1.100/relay/0?turn=on");
        assert_eq!(shelly.url_turn_off(), "http://192.168.1.100/relay/0?turn=off");
    }

    #[test]
    fn test_config_from_file() {
        let toml_content = r#"
        [shelly]
        ip = "192.168.1.100"
        timeout_seconds = 5
        max_retries = 3

        [battery]
        critical_threshold = 19
        low_threshold = 20
        high_threshold = 90
        hysteresis = 2
        check_interval_seconds = 30

        [network]
        max_consecutive_failures = 5
        check_interval_seconds = 60

        [logging]
        level = "info"

        [system]
        shutdown_command = "sudo shutdown -h now"
        shutdown_grace_period_seconds = 10
        "#;

        let temp_file = std::env::temp_dir().join("test_config.toml");
        std::fs::write(&temp_file, toml_content).expect("Failed to write temp config");

        let config = Config::from_file(temp_file.to_str().unwrap()).expect("Failed to load config");

        // Vérifier les valeurs Shelly
        assert_eq!(config.shelly.ip, "192.168.1.100");
        assert_eq!(config.shelly.timeout_seconds, 5);
        assert_eq!(config.shelly.max_retries, 3);

        // Vérifier les valeurs Battery
        assert_eq!(config.battery.critical_threshold, 19);
        assert_eq!(config.battery.low_threshold, 20);
        assert_eq!(config.battery.high_threshold, 90);
        assert_eq!(config.battery.hysteresis, 2);
        assert_eq!(config.battery.check_interval_seconds, 30);

        // Vérifier les valeurs Network
        assert_eq!(config.network.max_consecutive_failures, 5);
        assert_eq!(config.network.check_interval_seconds, 60);

        // Vérifier les valeurs Logging
        assert_eq!(config.logging.level, "info");

        // Vérifier les valeurs System
        assert_eq!(config.system.shutdown_command, "sudo shutdown -h now");
        assert_eq!(config.system.shutdown_grace_period_seconds, 10);

        std::fs::remove_file(&temp_file).expect("Failed to clean up temp file");
    }
}
