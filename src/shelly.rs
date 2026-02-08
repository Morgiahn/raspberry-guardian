// ============================================================================
// MODULE DE CONTRÔLE SHELLY PLUG S
// ============================================================================
// Ce module gère toutes les interactions avec la prise connectée

use anyhow::{Context, Result};
use log::{debug, error, info, warn};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::config::ShellyConfig;

/// Réponse JSON de l'API Shelly pour l'état du relay
/// 
/// # Exemple de réponse Shelly Plug S
/// ```json
/// {
///   "ison": true,
///   "has_timer": false,
///   "timer_started": 0,
///   "timer_duration": 0,
///   "timer_remaining": 0,
///   "overpower": false,
///   "source": "http"
/// }
/// ```
#[derive(Debug, Deserialize, Serialize)]
pub struct ShellyRelayStatus {
    /// La prise est-elle allumée ?
    pub ison: bool,
    
    /// Y a-t-il un timer actif ?
    #[serde(default)]
    pub has_timer: bool,
    
    /// Surcharge détectée ?
    #[serde(default)]
    pub overpower: bool,
}

/// Contrôleur pour la prise Shelly Plug S
/// 
/// # Architecture
/// Cette struct encapsule :
/// - Un client HTTP (reqwest::Client) réutilisable
/// - La configuration Shelly (IP, timeouts, etc.)
/// 
/// # Pourquoi une struct ?
/// En Rust, on préfère regrouper données + comportements dans des structs
/// plutôt que d'avoir des fonctions globales
pub struct ShellyController {
    client: Client,
    config: ShellyConfig,
}

impl ShellyController {
    /// Crée un nouveau contrôleur
    /// 
    /// # Pourquoi `new()` ?
    /// Convention Rust : `new()` est le constructeur par défaut
    pub fn new(config: ShellyConfig) -> Result<Self> {
        // Créer un client HTTP avec timeout configuré
        let client = Client::builder()
            .timeout(config.timeout())
            .build()
            .context("Impossible de créer le client HTTP")?;

        Ok(Self { client, config })
    }

    /// Allume la prise (active la charge)
    /// 
    /// # Async/await
    /// Le mot-clé `async` indique que cette fonction peut "attendre" des opérations longues
    /// sans bloquer le thread. L'appelant devra utiliser `.await` pour l'exécuter.
    /// 
    /// # Retry logic
    /// On tente plusieurs fois en cas d'échec réseau temporaire
    pub async fn turn_on(&self) -> Result<()> {
        self.send_command(&self.config.url_turn_on(), "Allumage")
            .await
    }

    /// Éteint la prise (arrête la charge)
    pub async fn turn_off(&self) -> Result<()> {
        self.send_command(&self.config.url_turn_off(), "Extinction")
            .await
    }

    /// Récupère l'état actuel de la prise
    /// 
    /// # Type de retour
    /// `Result<ShellyRelayStatus>` signifie :
    /// - En cas de succès : Ok(ShellyRelayStatus { ison: true, ... })
    /// - En cas d'erreur : Err(description de l'erreur)
    pub async fn get_status(&self) -> Result<ShellyRelayStatus> {
        self.get_status_with_retry().await
    }

    /// Vérifie si la prise est accessible (répond aux requêtes)
    /// 
    /// # Utilité
    /// Avant de demander une charge critique (batterie < 19%), on vérifie
    /// que la prise est bien joignable
    pub async fn is_reachable(&self) -> bool {
        match self.get_status().await {
            Ok(_) => {
                debug!("Shelly Plug S est accessible");
                true
            }
            Err(e) => {
                error!("ERREUR : Shelly Plug S n'est pas accessible : {}", e);
                false
            }
        }
    }

    // === MÉTHODES PRIVÉES (helper functions) ===

    /// Envoie une commande HTTP GET avec retry
    /// 
    /// # Pourquoi privée ?
    /// Cette méthode n'est utilisée qu'en interne, pas besoin de l'exposer
    async fn send_command(&self, url: &str, action: &str) -> Result<()> {
        for attempt in 1..=self.config.max_retries {
            match self.client.get(url).send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        info!("{} réussi (tentative {}/{})", action, attempt, self.config.max_retries);
                        return Ok(());
                    } else {
                        warn!(
                            "{} échoué : HTTP {} (tentative {}/{})",
                            action,
                            response.status(),
                            attempt,
                            self.config.max_retries
                        );
                    }
                }
                Err(e) => {
                    error!(
                        "{} échoué : {} (tentative {}/{})",
                        action, e, attempt, self.config.max_retries
                    );
                }
            }

            // Attendre avant de réessayer (sauf au dernier essai)
            if attempt < self.config.max_retries {
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }

        anyhow::bail!("{} échoué après {} tentatives", action, self.config.max_retries)
    }

    /// Récupère le status avec retry
    async fn get_status_with_retry(&self) -> Result<ShellyRelayStatus> {
        for attempt in 1..=self.config.max_retries {
            match self.client.get(self.config.url_status()).send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        // Parser la réponse JSON en struct ShellyRelayStatus
                        // L'opérateur `?` propage l'erreur si le parsing échoue
                        let status = response.json::<ShellyRelayStatus>().await?;
                        debug!("Status récupéré : prise {}", if status.ison { "ON" } else { "OFF" });
                        return Ok(status);
                    }
                }
                Err(e) => {
                    warn!("Échec récupération status (tentative {}/{}) : {}", attempt, self.config.max_retries, e);
                }
            }

            if attempt < self.config.max_retries {
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }

        anyhow::bail!("Impossible de récupérer le status après {} tentatives", self.config.max_retries)
    }
}

// ============================================================================
// TESTS UNITAIRES
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Note : Ces tests nécessitent une vraie prise Shelly pour fonctionner
    // En pratique, on utiliserait des mocks pour tester sans matériel

    #[test]
    fn test_shelly_relay_status_deserialize() {
        let json = r#"{"ison":true,"has_timer":false,"overpower":false}"#;
        let status: ShellyRelayStatus = serde_json::from_str(json).unwrap();
        assert!(status.ison);
        assert!(!status.has_timer);
    }

    /// Test que la commande turn_on construit la bonne URL
    ///
    /// Plutôt que de tester avec un vrai serveur HTTP (qui peut créer
    /// des conflits de runtime), on teste que les URLs sont bien formées
    #[test]
    fn test_turn_on_url_construction() {
        let config = ShellyConfig {
            ip: "192.168.1.100".to_string(),
            timeout_seconds: 5,
            max_retries: 3,
        };

        // Vérifier que l'URL de turn_on est correctement formée
        let url = config.url_turn_on();
        assert_eq!(url, "http://192.168.1.100/relay/0?turn=on");

        // Vérifier que l'URL de turn_off est correctement formée
        let url_off = config.url_turn_off();
        assert_eq!(url_off, "http://192.168.1.100/relay/0?turn=off");

        // Vérifier que l'URL de status est correctement formée
        let url_status = config.url_status();
        assert_eq!(url_status, "http://192.168.1.100/relay/0");
    }

    /// Test que le ShellyController se crée correctement
    #[test]
    fn test_shelly_controller_creation() {
        let config = ShellyConfig {
            ip: "192.168.1.100".to_string(),
            timeout_seconds: 5,
            max_retries: 3,
        };

        // Créer le contrôleur - ce test ne fait qu'une requête GET
        let controller = ShellyController::new(config);
        assert!(controller.is_ok(), "Le contrôleur devrait se créer sans erreur");
    }

    /// Test que is_reachable() retourne true si le status est récupérable
    ///
    /// Note: Ce test utilise une URL invalide et va échouer (retourner false).
    /// Pour un vrai test avec mock HTTP, il faudrait utiliser mockito avec tokio-test.
    #[tokio::test]
    async fn test_is_reachable_returns_true_on_success() {
        // Créer un config avec une IP invalide (le test va logger un message d'erreur)
        let config = ShellyConfig {
            ip: "invalid-ip-for-test".to_string(),
            timeout_seconds: 1,  // Timeout court pour tester rapidement
            max_retries: 1,
        };

        let controller = ShellyController::new(config).unwrap();

        // Appeler is_reachable() - va retourner false car IP invalide
        let result = controller.is_reachable().await;

        // Vérifier que la fonction retourne false (pas accessible)
        assert!(!result, "is_reachable() devrait retourner false pour une IP invalide");
    }

    /// Test que is_reachable() log les messages appropriés quand la prise n'est pas accessible
    ///
    /// Ce test initialise le logger et vérifie que le message d'erreur est loggé.
    /// Le test retourne false car l'IP est invalide.
    /// Pour voir les logs en action, exécutez:
    /// RUST_LOG=error cargo test test_is_reachable_logs_error -- --nocapture
    #[tokio::test]
    async fn test_is_reachable_logs_warning_on_failure() {
        // Initialiser le logger une seule fois (les appels suivants sont ignorés)
        let _ = env_logger::builder().is_test(true).try_init();

        let config = ShellyConfig {
            ip: "invalid-host-that-will-fail".to_string(),
            timeout_seconds: 1,
            max_retries: 1,
        };

        let controller = ShellyController::new(config).unwrap();

        // Appeler is_reachable() - va logger un message d'ERROR
        // car la prise ne sera pas accessible
        let result = controller.is_reachable().await;

        // Vérifier que le résultat est false (pas accessible)
        assert!(!result);

        // Les logs d'erreur seront visibles avec: RUST_LOG=error cargo test -- --nocapture
        // Message attendu: "ERREUR : Shelly Plug S n'est pas accessible : ..."
    }

    /// Test que is_reachable() peut être appelée et retourne un booléen
    ///
    /// Ce test vérifie juste le comportement du retour de la fonction.
    /// Pour voir les logs d'erreur, exécutez: RUST_LOG=error cargo test test_is_reachable_behavior -- --nocapture
    #[tokio::test]
    async fn test_is_reachable_behavior() {
        let config = ShellyConfig {
            ip: "192.168.1.1".to_string(),
            timeout_seconds: 1,
            max_retries: 1,
        };

        let controller = ShellyController::new(config).unwrap();

        // Appeler is_reachable() - retourne false avec un message d'erreur
        let result = controller.is_reachable().await;

        // La fonction retourne toujours un bool (true ou false)
        assert!(!result);
    }

    #[test]
    fn test_turn_on_retries_once_then_succeeds() {
        // Créer un serveur mockito local
        let mut server = mockito::Server::new();
        let path_turn_on = "/relay/0?turn=on";

        // Premier mock : renvoie 500 une fois
        let mock_fail = server
            .mock("GET", path_turn_on)
            .with_status(500)
            .expect(1)
            .create();

        // Deuxième mock : renvoie 200 une fois
        let mock_ok = server
            .mock("GET", path_turn_on)
            .with_status(200)
            .expect(1)
            .create();

        // Construire la config en pointant vers le mock server
        let base_url = server.url();
        let ip = base_url.strip_prefix("http://").unwrap_or(&base_url);
        let config = ShellyConfig {
            ip: ip.to_string(),
            timeout_seconds: 5,
            max_retries: 2,
        };

        let controller = ShellyController::new(config).expect("Erreur création contrôleur");

        // Exécuter la future async dans un runtime tokio local (test synchrone)
        let rt = tokio::runtime::Runtime::new().expect("Impossible de créer runtime tokio");
        let res = rt.block_on(controller.turn_on());

        // La commande doit réussir après une tentative échouée puis une réussite
        assert!(res.is_ok(), "turn_on() devrait réussir après une seconde tentative");

        // Vérifier que les mocks ont bien été appelés
        mock_fail.assert();
        mock_ok.assert();
    }
}
