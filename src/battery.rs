// ============================================================================
// MODULE DE SURVEILLANCE DE BATTERIE
// ============================================================================
// Ce module surveille l'état de la batterie du Raspberry Pi
// Utilise I2C pour lire les données d'un contrôleur UPS (ex: IP5306, IP5209)

use anyhow::{Context, Result};
use log::{debug, info, warn};
use std::cell::RefCell;

/// Adresse I2C du contrôleur UPS (par défaut 0x2d)
const UPS_I2C_ADDR: u16 = 0x2d;

/// Registres I2C du contrôleur UPS
mod i2c_registers {
    #[allow(dead_code)]
    pub const STATE: u8 = 0x02;           // État de charge (1 byte)
    #[allow(dead_code)]
    pub const VBUS: u8 = 0x10;            // Tension/courant USB (6 bytes)
    #[allow(dead_code)]
    pub const BATTERY: u8 = 0x20;         // Données batterie (12 bytes)
    #[allow(dead_code)]
    pub const CELLS: u8 = 0x30;           // Tensions des 4 cellules (8 bytes)
}

// Importer rust-i2cdev uniquement sur Unix/Linux (RPi)
#[cfg(unix)]
use i2cdev::linux::LinuxI2CDevice; //  le type concret pour créer une instance du device I2C sur Linux
#[cfg(unix)]
use i2cdev::core::I2CDevice;  // Trait nécessaire pour utiliser les méthodes read()/write()
#[cfg(unix)]
use std::os::unix::io::AsRawFd;

/// Informations sur l'état de la batterie
/// 
/// # Ownership en Rust
/// Cette struct "possède" ses données (String, u8, bool, etc.)
/// Quand elle est détruite, ses données le sont aussi
#[derive(Debug, Clone)]
pub struct BatteryInfo {
    /// Niveau de charge en pourcentage (0-100)
    pub level: u8,
    
    /// La batterie est-elle en charge ?
    pub is_charging: bool,
    
    /// État textuel (pour logging)
    pub status: String,

    /// Tension batterie en mV
    pub battery_mv: u16,

    /// Courant batterie en mA (positif = charge, négatif = décharge)
    pub battery_ma: i16,

    /// Capacité restante en mAh
    pub remaining_capacity_mah: u16,

    /// Tension VBUS (USB) en mV
    pub vbus_mv: u16,

    /// Courant VBUS en mA
    pub vbus_ma: u16,
}

/// Moniteur de batterie via I2C
///
/// # Communication I2C
/// Utilise rust-i2cdev (LinuxI2CDevice) pour communiquer avec le contrôleur UPS
/// via le bus I2C (généralement /dev/i2c-1 sur Raspberry Pi)
#[cfg(unix)]
pub struct BatteryMonitor {
    device: Option<RefCell<LinuxI2CDevice>>,
}


/// Helper function pour lire un bloc de bytes via I2C SMBus
/// Utilise RefCell::borrow_mut() pour obtenir une référence mutable
#[cfg(unix)]
fn read_i2c_block_data(device: &RefCell<LinuxI2CDevice>, register: u8, length: usize) -> Result<Vec<u8>> {
    let mut device = device.borrow_mut();
    let mut data = vec![0u8; length];

    // Écrire le registre à lire
    device.write(&[register])
        .context("Erreur lors de l'écriture du registre")?;

    // Lire les données
    device.read(&mut data)
        .context("Erreur lors de la lecture des données I2C")?;

    debug!("Register 0x{:02x}: {} bytes lus = {:?}", register, length, data);

    Ok(data)
}

/// Implémentation Unix/Linux (RPi avec vraie I2C)
#[cfg(unix)]
impl BatteryMonitor {
    /// Crée un nouveau moniteur
    /// Ouvre le device I2C (généralement /dev/i2c-1 sur Raspberry Pi)
    pub fn new() -> Self {
        let device = match LinuxI2CDevice::new("/dev/i2c-1", UPS_I2C_ADDR) {
            Ok(dev) => {
                debug!("Device I2C ouvert: /dev/i2c-1, adresse 0x{:02x}", UPS_I2C_ADDR);
                // Wrapper dans RefCell pour mutabilité intérieure
                Some(RefCell::new(dev))
            },
            Err(e) => {
                warn!("Impossible d'ouvrir le device I2C: {}. Fonctionnalité batterie indisponible.", e);
                None
            }
        };

        Self { device }
    }

    /// Récupère les informations actuelles de la batterie
    /// 
    /// # Communication I2C via rust-i2cdev
    /// Utilise smbus_read_i2c_block_da qui est compatible avec smbus2 Python
    /// - État de charge (registre 0x02)
    /// - Tension/courant batterie (registre 0x20)
    /// - Tension/courant VBUS (registre 0x10)
    pub fn get_battery_info(&self) -> Result<BatteryInfo> {
        let device = self.device.as_ref()
            .context("Device I2C non disponible")?;

        // Lire l'état de charge (1 byte)
        let state_data = read_i2c_block_data(device, i2c_registers::STATE, 1)
            .context("Erreur lors de la lecture de l'état de charge")?;
        let state_byte = state_data.get(0).copied().unwrap_or(0);

        // Décoder l'état
        let (is_charging, status) = match state_byte {
            s if (s & 0x40) != 0 => (true, "Fast Charging".to_string()),
            s if (s & 0x80) != 0 => (true, "Charging".to_string()),
            s if (s & 0x20) != 0 => (false, "Discharging".to_string()),
            _ => (false, "Idle".to_string()),
        };

        // Lire les données batterie (12 bytes)
        let battery_data = read_i2c_block_data(device, i2c_registers::BATTERY, 12)
            .context("Erreur lors de la lecture des données batterie")?;

        debug!("battery_data complète: {:?}", battery_data);

        // Décoder les champs batterie (mapping identique au code Python)
        let battery_mv = if battery_data.len() > 1 {
            battery_data[0] as u16 | ((battery_data[1] as u16) << 8)
        } else {
            0
        };

        let battery_ma_raw = if battery_data.len() > 3 {
            battery_data[2] as u16 | ((battery_data[3] as u16) << 8)
        } else {
            0
        };

        // Interpréter comme nombre signé sur 16 bits
        let battery_ma = battery_ma_raw as i16;

        // Lire le pourcentage sur 16 bits (comme en Python: b[4] | (b[5] << 8))
        let level_raw = if battery_data.len() > 5 {
            let b4 = battery_data[4] as u16;
            let b5 = battery_data[5] as u16;
            debug!("Pourcentage: byte[4]={} (0x{:02x}), byte[5]={} (0x{:02x})", b4, b4, b5, b5);
            b4 | (b5 << 8)
        } else {
            0
        };
        // Convertir en pourcentage (0-100)
        let level = (level_raw % 100) as u8;
        info!("level_raw: {} (0x{:04x}), level_percent: {}", level_raw, level_raw, level);

        let remaining_capacity_mah = if battery_data.len() > 7 {
            battery_data[6] as u16 | ((battery_data[7] as u16) << 8)
        } else {
            0
        };

        // Lire les données VBUS (6 bytes)
        let vbus_data = read_i2c_block_data(device, i2c_registers::VBUS, 6)
            .context("Erreur lors de la lecture des données VBUS")?;

        let vbus_mv = if vbus_data.len() > 1 {
            vbus_data[0] as u16 | ((vbus_data[1] as u16) << 8)
        } else {
            0
        };

        let vbus_ma = if vbus_data.len() > 3 {
            vbus_data[2] as u16 | ((vbus_data[3] as u16) << 8)
        } else {
            0
        };

        debug!(
            "Batterie lue via I2C (rust-i2cdev): {}% ({}mV, {}mA) - {}",
            level, battery_mv, battery_ma, status
        );

        Ok(BatteryInfo {
            level,
            is_charging,
            status,
            battery_mv,
            battery_ma,
            remaining_capacity_mah,
            vbus_mv,
            vbus_ma,
        })
    }

    /// Vérifie si le niveau de batterie est critique
    pub fn is_critical(&self, threshold: u8) -> Result<bool> {
        let info = self.get_battery_info()?;
        Ok(info.level <= threshold)
    }

    /// Vérifie si le niveau de batterie est bas
    pub fn is_low(&self, threshold: u8) -> Result<bool> {
        let info = self.get_battery_info()?;
        Ok(info.level <= threshold)
    }

    /// Vérifie si le niveau de batterie est haut
    pub fn is_high(&self, threshold: u8) -> Result<bool> {
        let info = self.get_battery_info()?;
        Ok(info.level >= threshold)
    }
}

/// Stub pour les plateformes non-Unix (compilation uniquement)
#[cfg(not(unix))]
pub struct BatteryMonitor;

#[cfg(not(unix))]
impl BatteryMonitor {
    pub fn new() -> Self {
        BatteryMonitor
    }

    pub fn get_battery_info(&self) -> Result<BatteryInfo> {
        Err(anyhow::anyhow!("I2C non disponible sur cette plateforme"))
    }

    pub fn is_critical(&self, _threshold: u8) -> Result<bool> {
        Err(anyhow::anyhow!("I2C non disponible sur cette plateforme"))
    }

    pub fn is_low(&self, _threshold: u8) -> Result<bool> {
        Err(anyhow::anyhow!("I2C non disponible sur cette plateforme"))
    }

    pub fn is_high(&self, _threshold: u8) -> Result<bool> {
        Err(anyhow::anyhow!("I2C non disponible sur cette plateforme"))
    }
}

// ============================================================================
// ALTERNATIVE : Moniteur spécifique pour UPS HAT populaires
// ============================================================================
// Si vous utilisez un UPS HAT spécifique (ex: PiJuice, UPS-Lite),
// décommentez et adaptez cette section

/*
use std::process::Command;

pub struct UpsHatMonitor;

impl UpsHatMonitor {
    /// Exemple pour UPS-Lite (via I2C)
    /// Nécessite le package `i2ctools` installé
    pub fn read_battery_level() -> Result<u8> {
        let output = Command::new("i2cget")
            .args(&["-y", "1", "0x36", "0x04"])
            .output()
            .context("Impossible d'exécuter i2cget")?;

        let hex_value = String::from_utf8(output.stdout)?;
        let level = u8::from_str_radix(hex_value.trim().trim_start_matches("0x"), 16)?;
        
        Ok(level)
    }
}
*/

// ============================================================================
// TESTS UNITAIRES
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_battery_info_creation() {
        let info = BatteryInfo {
            level: 75,
            is_charging: true,
            status: "Charging".to_string(),
            battery_mv: 4200,
            battery_ma: 500,
            remaining_capacity_mah: 2500,
            vbus_mv: 5000,
            vbus_ma: 1000,
        };

        assert_eq!(info.level, 75);
        assert!(info.is_charging);
        assert_eq!(info.battery_mv, 4200);
    }

    #[test]
    fn test_battery_monitor_creation() {
        let _monitor = BatteryMonitor::new();
        // Le moniteur se crée sans panique, même si le bus I2C n'est pas disponible
        // (mode dégradé avec bus = None)
    }

    #[test]
    fn test_decode_state_byte() {
        // Test du décodage des états
        // 0x40 = Fast Charging
        // 0x80 = Charging
        // 0x20 = Discharging
        // Sinon = Idle

        let (is_charging, status) = match 0x40u8 {
            s if (s & 0x40) != 0 => (true, "Fast Charging".to_string()),
            s if (s & 0x80) != 0 => (true, "Charging".to_string()),
            s if (s & 0x20) != 0 => (false, "Discharging".to_string()),
            _ => (false, "Idle".to_string()),
        };
        assert!(is_charging);
        assert_eq!(status, "Fast Charging");

        let (is_charging, status) = match 0x80u8 {
            s if (s & 0x40) != 0 => (true, "Fast Charging".to_string()),
            s if (s & 0x80) != 0 => (true, "Charging".to_string()),
            s if (s & 0x20) != 0 => (false, "Discharging".to_string()),
            _ => (false, "Idle".to_string()),
        };
        assert!(is_charging);
        assert_eq!(status, "Charging");

        let (is_charging, status) = match 0x20u8 {
            s if (s & 0x40) != 0 => (true, "Fast Charging".to_string()),
            s if (s & 0x80) != 0 => (true, "Charging".to_string()),
            s if (s & 0x20) != 0 => (false, "Discharging".to_string()),
            _ => (false, "Idle".to_string()),
        };
        assert!(!is_charging);
        assert_eq!(status, "Discharging");

        let (is_charging, status) = match 0x00u8 {
            s if (s & 0x40) != 0 => (true, "Fast Charging".to_string()),
            s if (s & 0x80) != 0 => (true, "Charging".to_string()),
            s if (s & 0x20) != 0 => (false, "Discharging".to_string()),
            _ => (false, "Idle".to_string()),
        };
        assert!(!is_charging);
        assert_eq!(status, "Idle");
    }
}











