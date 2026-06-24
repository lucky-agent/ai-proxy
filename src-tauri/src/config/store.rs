use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use log::LevelFilter;

use tauri_plugin_log::{RotationStrategy, Target, TargetKind};

use crate::config::{LogConfig, db::Db};

/// 应用数据存储路径管理，默认路径在用户目录的 .ai-proxy 下
pub struct Store {
    data_dir: PathBuf,
    db: Arc<Mutex<Db>>,
    scripts_dir: PathBuf,
}

impl Store {
    pub fn new(data_dir: PathBuf) -> Self {
        std::fs::create_dir_all(&data_dir).expect("Failed to create data directory");
        let scripts_dir = data_dir.join("scripts");
        std::fs::create_dir_all(&scripts_dir).ok();
        let db_path = data_dir.join("traffic.db");
        let db = Db::open(&db_path).expect("Failed to initialize database");
        Self {
            data_dir,
            db: Arc::new(Mutex::new(db)),
            scripts_dir,
        }
    }

    pub fn data_dir(&self) -> &PathBuf {
        &self.data_dir
    }

    pub fn scripts_dir(&self) -> &PathBuf {
        &self.scripts_dir
    }

    pub(crate) fn db(&self) -> Arc<Mutex<Db>> {
        self.db.clone()
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
