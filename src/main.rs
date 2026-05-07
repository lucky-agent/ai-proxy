mod config;
mod proxy;
pub mod utils;

use rama::error::BoxError;
use tracing::info;

use crate::config::Settings;
use crate::proxy::ProxyServer;

const BANNER: &str = r#"
╔══════════════════════════════════════════════╗
║         AI Proxy Server v0.2.0              ║
║    OpenAI API 反向代理 + 流量日志         ║
╚══════════════════════════════════════════════╝
"#;

#[tokio::main]
async fn main() -> Result<(), BoxError> {
    let settings = Settings::load()?;
    settings.init_logger()?;

    println!("{}", BANNER);
    info!("AI Proxy Server v0.2.0 - 正在启动...");

    let server = ProxyServer::new(settings);
    server.run().await?;

    Ok(())
}