use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use dirs::home_dir;
use log::LevelFilter;
use tauri_plugin_log::{RotationStrategy, Target, TargetKind};

use crate::config::LogConfig;

/// 应用数据存储路径管理，默认路径在用户目录的 .ai-proxy 下
pub struct Store {
    data_dir: Arc<Mutex<PathBuf>>,
    scripts_dir: PathBuf,
}

impl Store {
    pub fn new() -> Self {
        let data_dir = std::env::var("AI_PROXY_HOME")
            .map(PathBuf::from)
            .ok()
            .or_else(home_dir)
            .expect("Failed to determine data directory")
            .join(".ai-proxy");
        std::fs::create_dir_all(&data_dir).expect("Failed to create .ai-proxy directory");
        let scripts_dir = data_dir.join("scripts");
        std::fs::create_dir_all(&scripts_dir).ok();
        Self {
            data_dir: Arc::new(Mutex::new(data_dir)),
            scripts_dir,
        }
    }

    pub fn data_dir(&self) -> PathBuf {
        self.data_dir
            .lock()
            .expect("Failed to lock data directory")
            .clone()
    }

    pub fn scripts_dir(&self) -> &PathBuf {
        &self.scripts_dir
    }

    pub fn db_path(&self) -> PathBuf {
        self.data_dir().join("traffic.db")
    }

    pub fn collections_path(&self) -> PathBuf {
        self.data_dir().join("collections.json")
    }

    pub fn build_log_plugin(log: &LogConfig) -> tauri_plugin_log::Builder {
        let log_dir = PathBuf::from(log.dir.clone().unwrap_or_else(|| "logs".to_string()));
        let mut targets = vec![Target::new(TargetKind::Folder {
            path: log_dir,
            file_name: None,
        })];
        if log.console {
            targets.push(Target::new(TargetKind::Stdout));
        }

        let rotation = match log.rotation_strategy.as_str() {
            "KeepOne" => RotationStrategy::KeepOne,
            _ => RotationStrategy::KeepAll,
        };

        let level_filter: LevelFilter = log.level.parse().unwrap_or(LevelFilter::Info);

        tauri_plugin_log::Builder::default()
            .targets(targets)
            .rotation_strategy(rotation)
            .max_file_size(log.max_file_size)
            .level(level_filter)
            .format(|out, message, record| {
                out.finish(format_args!(
                    "[{}][{}] {}",
                    chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
                    record.level(),
                    message
                ))
            })
    }
}
