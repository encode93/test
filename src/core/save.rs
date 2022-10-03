use crate::core::config::DeviceSettings;
use crate::core::sync::{action_handler, Action, CorePackage, Phone, User};
use crate::core::utils::DisplayablePath;
use crate::gui::widgets::package_row::PackageRow;
use crate::CACHE_DIR;
use serde::{Deserialize, Serialize};
use static_init::dynamic;
use std::fs;
use std::path::{Path, PathBuf};

#[dynamic]
pub static BACKUP_DIR: PathBuf = CACHE_DIR.join("backups");

#[derive(Default, Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
struct PhoneBackup {
    device_id: String,
    users: Vec<UserBackup>,
}

#[derive(Default, Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
struct UserBackup {
    id: u16,
    packages: Vec<CorePackage>,
}

// Backup all `Uninstalled` and `Disabled` packages
pub async fn backup_phone(
    users: Vec<User>,
    device_id: String,
    phone_packages: Vec<Vec<PackageRow>>,
) -> Result<(), String> {
    let mut backup = PhoneBackup {
        device_id: device_id.clone(),
        ..PhoneBackup::default()
    };

    for u in users {
        let mut user_backup = UserBackup {
            id: u.id,
            ..UserBackup::default()
        };

        for p in phone_packages[u.index].clone() {
            user_backup.packages.push(CorePackage {
                name: p.name.clone(),
                state: p.state,
            })
        }
        backup.users.push(user_backup);
    }

    match serde_json::to_string_pretty(&backup) {
        Ok(json) => {
            let backup_path = &*BACKUP_DIR.join(device_id);

            if let Err(e) = fs::create_dir_all(backup_path) {
                error!("BACKUP: could not create backup dir: {}", e);
                return Err(e.to_string());
            };

            let backup_filename = format!("{}.json", chrono::Local::now().format("%Y-%m-%d-%H-%M"));

            match fs::write(backup_path.join(backup_filename), json) {
                Ok(_) => Ok(()),
                Err(err) => Err(err.to_string()),
            }
        }
        Err(err) => {
            error!("[BACKUP]: {}", err);
            Err(err.to_string())
        }
    }
}

pub fn list_available_backups(dir: &Path) -> Vec<DisplayablePath> {
    match fs::read_dir(dir) {
        Ok(files) => files
            .filter_map(|e| e.ok())
            .map(|e| DisplayablePath { path: e.path() })
            .collect::<Vec<_>>(),
        Err(_) => vec![],
    }
}

pub fn list_available_backup_user(backup: DisplayablePath) -> Vec<User> {
    match fs::read_to_string(backup.path) {
        Ok(data) => {
            let phone_backup: PhoneBackup =
                serde_json::from_str(&data).expect("Unable to parse backup file");

            let mut users = vec![];
            for u in phone_backup.users {
                users.push(User { id: u.id, index: 0 });
            }
            users
        }
        Err(e) => {
            error!("[BACKUP]: Selected backup file not found: {}", e);
            vec![]
        }
    }
}


// TODO: we need to change the way package state change are handled
// Better to try to match the wanted state instead of applying the "reverse" ADB command
pub fn restore_backup(
    selected_device: &Phone,
    settings: &DeviceSettings,
) -> Result<Vec<String>, String> {
    match fs::read_to_string(settings.backup.selected.as_ref().unwrap().path.clone()) {
        Ok(data) => {
            let phone_backup: PhoneBackup =
                serde_json::from_str(&data).expect("Unable to parse backup file");

            let mut commands = vec![];
            for u in phone_backup.users {
                for packages in u.packages {
                    commands.extend(action_handler(
                        &settings.backup.selected_user.unwrap(),
                        &packages,
                        selected_device,
                        settings,
                        &Action::RestoreDevice,
                    ));
                }
            }
            Ok(commands)
        }
        Err(e) => Err("[BACKUP]: ".to_owned() + &e.to_string()),
    }
}