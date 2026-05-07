# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目概述

ai-proxy 是一个基于 Rust 的 Anthropic/OpenAI API 反向代理服务器，支持 MITM（中间人）模式拦截和记录 HTTP/HTTPS 流量。核心功能是将请求转发到上游 API 并对请求/响应进行日志记录，支持 SSE 流式响应的逐 chunk 日志输出。

## 构建与运行

```bash
# 构建
cargo build

# 运行（需要 setting.json 或环境变量）
cargo run

# 发布构建
cargo build --release
```

本项目当前没有测试。Rust 版本要求：1.91.1+，edition 2024。

## 配置

通过 `setting.json` 文件或环境变量配置：

- `setting.json`：主配置文件，包含 `anthropic_base_url`、`anthropic_api_key`、`listen_host`、`listen_port` 等
- 环境变量：`ANTHROPIC_BASE_URL` 和 `ANTHROPIC_API_KEY` 作为 fallback
- 日志级别可通过 `RUST_LOG` 环境变量覆盖

## 代码架构

**入口**：`src/main.rs` → 加载配置 → 创建 `ProxyServer` → 启动服务

**模块职责**：

- **config** (`src/config.rs`)：`Settings` 结构体，负责从 `setting.json` 或环境变量加载配置、初始化日志系统。自定义 `bail!` / `anyhow!` 宏用于错误处理（基于 rama 的 `OpaqueError`）
- **proxy** (`src/proxy.rs`)：`ProxyServer` 和 `State`。核心代理逻辑：监听 TCP → 处理 CONNECT 方法（HTTPS 随道）→ MITM TLS 解密 → 转发请求到上游。使用 rama 的 `UpgradeLayer` 处理 HTTP CONNECT 隧道，自签名证书用于 MITM TLS
- **client** (`src/client.rs`)：`http_mitm_proxy` 函数，实际的请求转发逻辑。收集请求 body → 构建上游客户端（含 TLS、解压缩）→ 转发请求 → 根据 content-type 决定是否缓冲响应（SSE 流式透传，非流式 JSON 美化打印）
- **utils** (`src/utils/macros.rs`)：`FormattedError` 类型 + `bail!` / `anyhow!` 宏，替代 anyhow crate

**关键框架**：rama（https://github.com/plabayo/rama）——提供 HTTP 服务端/客户端、TLS、代理、中间件等能力，是整个代理的核心依赖。

## 重要注意事项

- `setting.json` 可能包含 API 密钥，不应提交到仓库（.gitignore 中未显式排除，需手动注意）
- MITM 代理使用自签名证书，客户端需要信任或忽略证书警告
- SSE 流式响应（`text/event-stream`）采用逐 chunk 透传模式，非流式响应会完整收集后转发
- 错误处理不使用 anyhow，而是自建的 `bail!` / `anyhow!` 宏 + rama 的 `OpaqueError`