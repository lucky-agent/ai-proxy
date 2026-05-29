# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目概述

ai-proxy 是一个基于 Tauri 2 的桌面应用，后端为 Rust MITM 反向代理，前端为 React + Vite。核心功能是将请求转发到上游 API 并拦截记录 HTTP/HTTPS 流量，支持 SSE 流式响应的逐 chunk 日志输出，通过 GUI 展示流量日志、代理控制与设置。

## 构建与运行

```bash
# 开发（Tauri + Vite 热重载）
bun run dev

# 仅启动前端 Vite 开发服务器
bun run dev:vite

# 发布构建
bun run build

# 仅构建前端
bun run build:vite
```

本项目当前没有测试。Rust 版本要求：1.91.1+，edition 2024。包管理使用 bun。

## 配置

配置文件位于用户数据目录 `~/.ai-proxy/setting.json`（可通过环境变量 `AI_PROXY_HOME` 覆盖根目录）：

- `proxy`：`listen_host`、`listen_port`、`upstream_proxy` 等代理设置
- `ui`：主题（`theme`）、语言（`language`）
- `log`：日志级别、目录、滚动策略等

前端通过 Tauri command `get_settings` / `save_settings` 读写配置，不直接访问文件。

## 目录结构

```
ai-proxy/
├── src/                          # 前端（React + TypeScript）
│   ├── main.tsx                  # 入口，渲染后显示窗口
│   ├── App.tsx                   # 根组件：代理控制、布局编排
│   └── index.css                 # 全局样式（Tailwind 主题变量 + 自定义组件样式）
│   │
│   ├── components/
│   │   └── ui/                   # 通用 UI 原子组件（shadcn）
│   │       ├── button.tsx
│   │       ├── dialog.tsx
│   │       └── dropdown-menu.tsx
│   │
│   ├── features/                 # 按功能域组织的业务组件
│   │   ├── traffic-log/          # 主内容区（三栏布局）
│   │   │   ├── TrafficLog.tsx    # 容器：域名侧栏 + 请求列表 + 详情面板
│   │   │   ├── DomainSidebar.tsx
│   │   │   ├── RequestList.tsx
│   │   │   ├── DetailPanel.tsx
│   │   │   └── index.ts          # 对外导出入口
│   │   ├── title-bar/            # 自定义标题栏
│   │   │   ├── TitleBar.tsx
│   │   │   └── index.ts
│   │   └── settings/             # 设置弹窗
│   │       ├── SettingsDialog.tsx
│   │       └── index.ts
│   │
│   ├── hooks/                    # 跨 feature 复用的 React hooks
│   │   ├── useProxyEvents.ts     # 监听后端流量事件
│   │   ├── useTheme.ts
│   │   └── useLocale.ts
│   ├── i18n/                     # i18next 初始化
│   ├── locales/                  # 翻译文件（en.json、zh.json）
│   ├── lib/                      # 工具函数（format、cn 等）
│   └── types/                    # TypeScript 类型定义
│
├── src-tauri/                    # 后端（Rust + Tauri）
│   ├── src/
│   │   ├── main.rs               # 二进制入口
│   │   ├── lib.rs                # Tauri 应用构建、AppState、事件处理
│   │   ├── commands/             # Tauri invoke 命令
│   │   │   ├── proxy.rs          # start_proxy / stop_proxy / get_status
│   │   │   ├── settings.rs       # get_settings / save_settings
│   │   │   ├── theme.rs          # get_theme / set_theme
│   │   │   └── locale.rs         # get_locale / set_locale
│   │   ├── config/               # 配置加载与存储
│   │   │   ├── settings.rs       # Settings 结构体
│   │   │   └── store.rs          # 数据目录管理（~/.ai-proxy）
│   │   ├── proxy/                # MITM 代理核心
│   │   │   ├── mod.rs            # ProxyServer
│   │   │   ├── client.rs         # 请求转发逻辑
│   │   │   ├── mitm.rs           # TLS 中间人
│   │   │   ├── parser.rs         # 流量解析
│   │   │   ├── state.rs          # 代理状态
│   │   │   └── cert.rs           # 自签名证书
│   │   ├── tray.rs               # 系统托盘
│   │   └── utils/                # 错误处理宏（bail! / anyhow!）
│   ├── tauri.conf.json
│   └── Cargo.toml
│
├── scripts/                      # 开发脚本
├── vite.config.ts
└── package.json
```

### 前端组织原则

| 目录 | 放什么 | 判断标准 |
|------|--------|----------|
| `components/ui/` | Button、Dialog 等 | 无业务含义，任何地方可复用 |
| `features/<区域>/` | TrafficLog、SettingsDialog 等 | 属于某块功能/布局 |
| `features/<区域>/components/` | 仅该区域用的子组件 | 不跨 feature 复用时放这里 |
| `hooks/`、`lib/`、`types/` | 逻辑、工具、类型 | 非 UI |

`App.tsx` 通过 `@/features/<区域>` 导入各功能域入口，例如：

```ts
import { TrafficLog } from '@/features/traffic-log'
import { SettingsDialog } from '@/features/settings'
import { TitleBar } from '@/features/title-bar'
```

## 代码架构

**前端入口**：`src/main.tsx` → `App.tsx` → 各 `features/` 组件

**后端入口**：`src-tauri/src/main.rs` → `lib.rs::run()` → 加载配置 → 注册 Tauri commands → 启动应用

**前后端通信**：

- 前端 `invoke('start_proxy')` 等调用 `src-tauri/src/commands/` 中的命令
- 后端通过 Tauri `emit` 向前端推送流量事件（`useProxyEvents` 监听）

**后端模块职责**：

- **config** (`src-tauri/src/config/`)：`Settings` 结构体，从 `~/.ai-proxy/setting.json` 加载配置，管理数据目录
- **proxy** (`src-tauri/src/proxy/`)：`ProxyServer` 和 `State`。核心代理逻辑：监听 TCP → 处理 CONNECT（HTTPS 隧道）→ MITM TLS 解密 → 转发请求到上游
- **client** (`src-tauri/src/proxy/client.rs`)：`http_mitm_proxy` 函数，实际请求转发。SSE 流式透传，非流式响应完整收集后转发
- **commands** (`src-tauri/src/commands/`)：暴露给前端的 Tauri invoke 接口
- **utils** (`src-tauri/src/utils/macros.rs`)：`FormattedError` + `bail!` / `anyhow!` 宏，替代 anyhow crate

**关键框架**：

- 后端：rama（https://github.com/plabayo/rama）—— HTTP 服务端/客户端、TLS、代理
- 前端：React 19 + Vite 8 + Tailwind CSS 4 + shadcn/ui
- 桌面：Tauri 2

## 重要注意事项

- `setting.json` 可能包含敏感配置，位于用户目录，不应提交到仓库
- MITM 代理使用自签名证书，客户端需要信任或忽略证书警告
- SSE 流式响应（`text/event-stream`）采用逐 chunk 透传模式，非流式响应会完整收集后转发
- 错误处理不使用 anyhow，而是自建的 `bail!` / `anyhow!` 宏 + rama 的 `OpaqueError`
- 新增业务组件放 `features/<区域>/`，通用 UI 组件放 `components/ui/`，跨 feature 复用逻辑放 `hooks/` 或 `lib/`

<!-- superpowers-zh:begin (do not edit between these markers) -->
# Superpowers-ZH 中文增强版

本项目已安装 superpowers-zh 技能框架（20 个 skills）。

## 核心规则

1. **收到任务时，先检查是否有匹配的 skill** — 哪怕只有 1% 的可能性也要检查
2. **设计先于编码** — 收到功能需求时，先用 brainstorming skill 做需求分析
3. **测试先于实现** — 写代码前先写测试（TDD）
4. **验证先于完成** — 声称完成前必须运行验证命令

## 可用 Skills

Skills 位于 `.claude/skills/` 目录，每个 skill 有独立的 `SKILL.md` 文件。

- **brainstorming**: 在任何创造性工作之前必须使用此技能——创建功能、构建组件、添加功能或修改行为。在实现之前先探索用户意图、需求和设计。
- **chinese-code-review**: 中文 review 沟通参考——话术模板、分级标注（必须修复/建议修改/仅供参考）、国内团队常见反模式应对。仅在用户显式 /chinese-code-review 时调用，不要根据上下文自动触发。
- **chinese-commit-conventions**: 中文 commit 与 changelog 配置参考——Conventional Commits 中文适配、commitlint/husky/commitizen 中文模板、conventional-changelog 中文配置。仅在用户显式 /chinese-commit-conventions 时调用，不要根据上下文自动触发。
- **chinese-documentation**: 中文文档排版参考——中英文空格、全半角标点、术语保留、链接格式、中文文案排版指北约定。仅在用户显式 /chinese-documentation 时调用，不要根据上下文自动触发。
- **chinese-git-workflow**: 国内 Git 平台配置参考——Gitee、Coding.net、极狐 GitLab、CNB 的 SSH/HTTPS/凭据/CI 接入差异与镜像同步配置。仅在用户显式 /chinese-git-workflow 时调用，不要根据上下文自动触发。
- **dispatching-parallel-agents**: 当面对 2 个以上可以独立进行、无共享状态或顺序依赖的任务时使用
- **executing-plans**: 当你有一份书面实现计划需要在单独的会话中执行，并设有审查检查点时使用
- **finishing-a-development-branch**: 当实现完成、所有测试通过、需要决定如何集成工作时使用——通过提供合并、PR 或清理等结构化选项来引导开发工作的收尾
- **mcp-builder**: MCP 服务器构建方法论 — 系统化构建生产级 MCP 工具，让 AI 助手连接外部能力
- **receiving-code-review**: 收到代码审查反馈后、实施建议之前使用，尤其当反馈不明确或技术上有疑问时——需要技术严谨性和验证，而非敷衍附和或盲目执行
- **requesting-code-review**: 完成任务、实现重要功能或合并前使用，用于验证工作成果是否符合要求
- **subagent-driven-development**: 当在当前会话中执行包含独立任务的实现计划时使用
- **systematic-debugging**: 遇到任何 bug、测试失败或异常行为时使用，在提出修复方案之前执行
- **test-driven-development**: 在实现任何功能或修复 bug 时使用，在编写实现代码之前
- **using-git-worktrees**: 当需要开始与当前工作区隔离的功能开发，或在执行实现计划之前使用——通过原生工具或 git worktree 回退机制确保隔离工作区存在
- **using-superpowers**: 在开始任何对话时使用——确立如何查找和使用技能，要求在任何响应（包括澄清性问题）之前调用 Skill 工具
- **verification-before-completion**: 在宣称工作完成、已修复或测试通过之前使用，在提交或创建 PR 之前——必须运行验证命令并确认输出后才能声称成功；始终用证据支撑断言
- **workflow-runner**: 在 Claude Code / OpenClaw / Cursor 中直接运行 agency-orchestrator YAML 工作流——无需 API key，使用当前会话的 LLM 作为执行引擎。当用户提供 .yaml 工作流文件或要求多角色协作完成任务时触发。
- **writing-plans**: 当你有规格说明或需求用于多步骤任务时使用，在动手写代码之前
- **writing-skills**: 当创建新技能、编辑现有技能或在部署前验证技能是否有效时使用

## 如何使用

当任务匹配某个 skill 时，使用 `Skill` 工具加载对应 skill 并严格遵循其流程。绝不要用 Read 工具读取 SKILL.md 文件。

如果你认为哪怕只有 1% 的可能性某个 skill 适用于你正在做的事情，你必须调用该 skill 检查。
<!-- superpowers-zh:end -->
