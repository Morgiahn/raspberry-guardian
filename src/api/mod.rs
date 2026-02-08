use std::sync::Arc;

mod api_error;
pub use api_error::ApiError;

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response}, 
    routing::{get, post},
    Json,
    Router,
};

use log::{error, info, warn};
use serde::Deserialize;
use tokio::sync::Mutex;
use tokio::time::sleep;
use crate::config::Config;
use crate::perform_shutdown;
use crate::shelly::ShellyController;
use crate::state_machine::{ForceMode, StateController, ChargeState};
use anyhow::Error;

use crate::app_state::AppState;

use crate::system::shutdown_with_grace;


#[derive(Deserialize)]
struct ModeRequest {
    mode: String,
}

#[derive(serde::Serialize)]
struct BatteryStatus {
    level_percent: u8,
    charging: bool,
    mode: Option<String>,
    state: String,
}


pub async fn start_api_server(
    app_state: AppState
) {
    let addr = format!("{}:{}", app_state.config.system.api_ip, app_state.config.system.api_port);

    let app = Router::new()
        .route("/mode", post(set_mode))
        .route("/status", get(get_status))
        .with_state(app_state);


    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|_| panic!("Impossible de binder l'API sur {}", addr));

    info!("API locale démarrée sur {}", addr);

    axum::serve(listener, app)
        .await
        .expect("Erreur serveur API");
}

async fn set_mode(
    State(state): State<AppState>,
    Json(payload): Json<ModeRequest>,
) -> Result<impl IntoResponse, ApiError>  {

    let mut controller = state.state_controller.lock().await;

    match payload.mode.as_str() {
        "charge" => {
            controller.set_forced_mode(Some(ForceMode::Charge));
            drop(controller);

            state
                .shelly
                .turn_on()
                .await
                .map_err(|e| {
                    error!("Erreur Shelly ON: {}", e);
                    anyhow::anyhow!("Erreur allumage prise")
                })?;

            info!(" Mode forcé → CHARGE");
            Ok((StatusCode::OK, "Mode charge activé"))
        }

        "discharge" => {
            controller.set_forced_mode(Some(ForceMode::Discharge));
            drop(controller);

            state.shelly.turn_off()
                .await
                .map_err(|e| {
                    error!("Erreur Shelly OFF: {}", e);
                    anyhow::anyhow!("Erreur extinction prise")
                })?;

            info!("Mode forcé → DÉCHARGE");
            Ok((StatusCode::OK, "Mode discharge activé"))
        }

        "stop" => {
            controller.set_forced_mode(Some(ForceMode::Stop));

            state.shelly.turn_off()
                .await
                .map_err(|e| {
                    error!("Erreur Shelly OFF: {}", e);
                    anyhow::anyhow!("Erreur extinction prise")
                })?;

            // Attendre le délai de grâce puis shutdown
            shutdown_with_grace(&state.config).await?;

            info!("Mode forcé → STOP");
            Ok((StatusCode::OK, "Shutdown activé"))
        }


        "auto" => {
            controller.set_forced_mode(None);
            drop(controller);

            info!("Retour mode automatique");
            Ok((StatusCode::OK, "Mode automatique activé"))
        }

        _ => Err(ApiError::BadRequest(
            "Mode invalide (charge | discharge | stop | auto)".into(),
        )),
    }
}

pub async fn get_status(
    State(state): State<AppState>
) -> Json<BatteryStatus> {
    let controller = state.state_controller.lock().await;
    let battery_monitor = state.battery_monitor.lock().await;
    
    let current_state = controller.current_state();
    let forced_mode = controller.forced_mode();
    let charging = matches!(current_state, ChargeState::Charging);

    let level_percent = match battery_monitor.get_battery_info() {
        Ok(info) => {
            info!("API: get_battery_info() OK, level = {}", info.level);
            info.level
        },
        Err(e) => {
            error!("API: get_battery_info() ERREUR: {}", e);
            0
        }
    };
    info!("level percent: {}", level_percent);

    Json(BatteryStatus {
        level_percent,
        charging,
        mode: forced_mode.map(|m| format!("{:?}", m)),
        state: format!("{:?}", current_state),
    })
}
