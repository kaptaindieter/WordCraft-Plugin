use std::path::{Path, PathBuf};
use std::time::Instant;

/// Drive detection and project manager for ArtCraft apps.
#[derive(Clone, Debug)]
pub struct ArtCraftDrive {
    pub app_name: &'static str,
    pub file_extension: &'static str,
    pub detected_drive_root: PathBuf,
    pub custom_drive_override: Option<PathBuf>,
    pub active_project_name: Option<String>,
    pub auto_save_enabled: bool,
    pub last_save_time: Option<Instant>,
    pub sync_status_message: String,
    pub show_project_picker: bool,
}

#[derive(Clone, Debug)]
pub struct ProjectItem {
    pub name: String,
    pub folder_name: String,
    pub folder_path: PathBuf,
    pub primary_file: Option<PathBuf>,
    pub modified_str: String,
}

impl ArtCraftDrive {
    /// Creates a new ArtCraftDrive manager for a specific application.
    pub fn new(app_name: &'static str, file_extension: &'static str) -> Self {
        let detected = Self::detect_google_drive();
        let s = Self {
            app_name,
            file_extension,
            detected_drive_root: detected,
            custom_drive_override: None,
            active_project_name: None,
            auto_save_enabled: true,
            last_save_time: None,
            sync_status_message: "Bereit".to_string(),
            show_project_picker: false,
        };
        s.ensure_app_directory();
        s
    }

    /// Reads the user-configured Cloud Sync root from `~/.craft-suite/cloud-config.json` if set.
    pub fn user_configured_sync_root() -> Option<PathBuf> {
        let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
        let cfg_path = PathBuf::from(home).join(".craft-suite").join("cloud-config.json");
        let raw = std::fs::read_to_string(cfg_path).ok()?;
        let val: serde_json::Value = serde_json::from_str(&raw).ok()?;
        let sync_path = val.get("sync_path")?.as_str()?.trim();
        if sync_path.is_empty() {
            None
        } else {
            Some(PathBuf::from(sync_path))
        }
    }

    /// Automatically detects a cloud sync folder or falls back to the user's Documents directory.
    pub fn detect_google_drive() -> PathBuf {
        if let Some(custom) = Self::user_configured_sync_root() {
            return custom;
        }

        let candidates = [
            Path::new(r"G:\My Drive"),
            Path::new(r"G:\Meine Ablage"),
            Path::new(r"G:\"),
        ];

        for c in candidates {
            if c.exists() && c.is_dir() {
                return c.to_path_buf();
            }
        }

        if let Some(onedrive) = std::env::var_os("OneDrive") {
            let od = PathBuf::from(onedrive);
            if od.exists() && od.is_dir() {
                return od;
            }
        }

        // Fallback: local user documents
        if let Some(user_profile) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")) {
            let docs = PathBuf::from(user_profile).join("Documents");
            if docs.exists() {
                return docs;
            }
        }

        PathBuf::from("ArtCraft_Sync")
    }

    /// Checks whether a cloud drive or custom sync directory is active.
    pub fn is_drive_detected(&self) -> bool {
        self.custom_drive_override.is_some()
            || Self::user_configured_sync_root().is_some()
            || self.drive_root().starts_with("G:")
            || Path::new(r"G:\").exists()
            || self.drive_root().exists()
    }

    /// Current active project folder name.
    pub fn active_project_folder_name(&self) -> String {
        if let Some(ref name) = self.active_project_name {
            name.clone()
        } else {
            "Standard".to_string()
        }
    }

    /// App suite directory on drive.
    pub fn app_drive_dir(&self) -> PathBuf {
        self.app_suite_dir()
    }

    /// Switches the active project to the given folder name.
    pub fn switch_to_project(&mut self, folder_name: &str) -> PathBuf {
        self.active_project_name = Some(folder_name.to_string());
        self.active_project_file()
    }

    /// Creates a new project folder with timestamp or specified name.
    pub fn create_new_project(&mut self, name: Option<&str>) -> PathBuf {
        self.set_or_init_project(name)
    }

    /// Checks if auto-save should trigger (e.g. 3 seconds elapsed).
    pub fn should_autosave(&self) -> bool {
        if !self.auto_save_enabled {
            return false;
        }
        if let Some(last) = self.last_save_time {
            last.elapsed().as_secs_f64() >= 3.0
        } else {
            true
        }
    }

    /// Marks that a save just succeeded.
    pub fn mark_saved(&mut self) {
        self.last_save_time = Some(Instant::now());
        self.sync_status_message = "Gespeichert".to_string();
    }

    /// Current active Cloud/Sync root directory (prioritizes user configuration).
    pub fn drive_root(&self) -> PathBuf {
        if let Some(ref o) = self.custom_drive_override {
            return o.clone();
        }
        if let Some(user_root) = Self::user_configured_sync_root() {
            return user_root;
        }
        self.detected_drive_root.clone()
    }

    /// App-specific suite root: `{drive_root}\Art Craft\{app_name}`
    pub fn app_suite_dir(&self) -> PathBuf {
        self.drive_root().join("Art Craft").join(self.app_name)
    }

    /// Ensures the `{drive_root}\Art Craft\{app_name}` directory exists.
    pub fn ensure_app_directory(&self) {
        let dir = self.app_suite_dir();
        let _ = std::fs::create_dir_all(&dir);
    }

    /// Returns the active project directory:
    /// `G:\Meine Ablage\Art Craft\<Programmname>\<Projekt- oder Dateiname>\`
    pub fn active_project_dir(&self) -> PathBuf {
        let proj_name = self.active_project_name.as_deref().unwrap_or("Unbenannt");
        self.app_suite_dir().join(proj_name)
    }

    /// Primary document file in the active project directory:
    /// `<project_dir>\<project_name>.<ext>`
    pub fn active_project_file(&self) -> PathBuf {
        let proj_name = self.active_project_name.as_deref().unwrap_or("Unbenannt");
        self.active_project_dir().join(format!("{proj_name}.{}", self.file_extension))
    }

    /// Sets or updates the active project name.
    /// If no name is provided, automatically generates a timestamped name.
    pub fn set_or_init_project(&mut self, name: Option<&str>) -> PathBuf {
        let final_name = match name {
            Some(n) if !n.trim().is_empty() => n.trim().to_string(),
            _ => {
                let now = chrono::Local::now();
                format!("Projekt {}", now.format("%Y-%m-%d %H-%M-%S"))
            }
        };

        self.active_project_name = Some(final_name);
        let dir = self.active_project_dir();
        let _ = std::fs::create_dir_all(&dir);
        self.active_project_file()
    }

    /// Lists all existing project folders under `{drive_root}\Art Craft\{app_name}\`.
    pub fn list_projects(&self) -> Vec<ProjectItem> {
        let suite_dir = self.app_suite_dir();
        let mut items = Vec::new();

        if let Ok(entries) = std::fs::read_dir(&suite_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    let primary_file = path.join(format!("{name}.{}", self.file_extension));
                    let actual_primary = if primary_file.exists() {
                        Some(primary_file)
                    } else {
                        // find first matching extension in project dir
                        std::fs::read_dir(&path).ok().and_then(|mut r| {
                            r.find_map(|e| {
                                e.ok().map(|e| e.path()).filter(|p| {
                                    p.extension().and_then(|x| x.to_str()) == Some(self.file_extension)
                                })
                            })
                        })
                    };

                    let modified_str = entry.metadata().ok().and_then(|m| m.modified().ok()).map_or_else(
                        || "-".to_string(),
                        |t| {
                            let dt: chrono::DateTime<chrono::Local> = t.into();
                            dt.format("%d.%m.%Y %H:%M").to_string()
                        },
                    );

                    items.push(ProjectItem {
                        folder_name: name.clone(),
                        name,
                        folder_path: path,
                        primary_file: actual_primary,
                        modified_str,
                    });
                }
            }
        }

        items.sort_by(|a, b| b.modified_str.cmp(&a.modified_str));
        items
    }
}
