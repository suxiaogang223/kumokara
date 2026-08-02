# Kumokara（雲殻）— Product Design Document

> Version: 0.4.0 | Status: Draft | License: MIT  
> *Agents never sleep in Kumokara（雲殻）.*

---

## 1. Brand

| 要素 | 内容 |
|------|------|
| **名称** | Kumokara |
| **汉字** | 雲殻 |
| **读音** | くもから / koo-mo-kah-rah |
| **语义** | 雲（cloud）+ 殻（shell）— 云中的壳，AI Agent 在云端永恒不灭的容器 |
| **Tagline** | Agents never sleep in Kumokara（雲殻）. |
| **定位** | Self-hosted Agent Development Environment（自部署 Agent 开发环境） |

### 品牌故事

传统终端是人的工具，人打字，Shell 执行。Kumokara 反转这个关系——Shell 是 Agent 的躯壳，Agent 是住在壳里的灵魂（Ghost in the Shell）。你把 Agent 部署到 Kumokara，它 24 小时在线，你通过任何设备的浏览器来视察进度、下达新任务、审批关键决策。

### 视觉方向

Logo：云的轮廓内嵌一枚开口的贝壳，壳中透出青色微光——Agent 栖息在云壳之中。  
色彩：鼠色（#949495）为基底 + 霓虹青（#00E5FF）为强调。

---

## 2. Product Overview

Kumokara 是一个 **自部署的 Agent 开发环境**。核心能力：

1. **Workspace** — 项目级工作空间：独立的文件系统、环境变量、Agent 配置、事件历史
2. **Session** — Workspace 下 0..N 个终端会话（shell / agent），关掉浏览器不中断
3. **Agent Integration** — Otty 式集成：向 Claude Code / Codex / OpenCode 自身配置安装 hooks，感知状态、离线通知、resume / fork
4. **Observability** — 注入 shell integration（OSC 133/7），把终端流解析为结构化事件流
5. **Reliability** — tmux 包裹 + agent resume + 进程白名单，Server 重启不丢现场
6. **SSH Target** — Agent 可以操作远程 SSH 机器

### 竞品生态位

```
                    本地/桌面                  Web/云端
           ┌────────────────────┬─────────────────────────┐
 传统终端   │ iTerm, Kitty,      │ ttyd, wetty             │
           │ Alacritty, Warp    │ (终端转发)               │
           ├────────────────────┼─────────────────────────┤
 Agent友好 │ Otty               │ ★ Kumokara              │
 终端      │ (桌面Agent友好终端)  │ (ADE — Agent永久驻地)    │
           ├────────────────────┼─────────────────────────┤
 云端IDE   │ Cursor, Windsurf   │ Replit, Codespaces      │
           │ (本地AI IDE)       │ (全托管平台)              │
           └────────────────────┴─────────────────────────┘
```

Kumokara 的独特组合：**自托管 + Agent 永久在线 + 全设备 Web 访问**。相邻形态有
Vibe Kanban、Conductor、Omnara 等"多 Agent 管理"工具，但均非自托管的完整 ADE。

### 部署形态：Local 与 Remote

同一套代码、同一份数据格式、同一条协议，两种部署形态：

| | **Local 模式** | **Remote 模式** |
|---|---|---|
| Server | 本机（`kumokara` 单命令启动） | VPS / 家庭服务器（`kumokara server` 守护进程） |
| 客户端 | 本机浏览器 / Tauri App（内嵌 server sidecar，见 8.6） | 任意设备浏览器 / App |
| 认证 | 绑定 127.0.0.1 + 启动器自动注入一次性 token | token / OAuth + TLS（见第 9 章） |
| 形态等价物 | Otty（本地 Agent 终端） | 24h Agent 永久驻地 |

要点：

- **Local 模式不是单独产品**：只是 server 与 client 同机，架构零分叉；
- **关键差异化**：Local 模式下 server 注册为用户级守护进程（launchd /
  systemd --user），**退出 App 不杀 Agent**——Otty 关窗即终止，Kumokara 关窗
  Agent 继续跑，"Agents never sleep in Kumokara（雲殻）" 在本地同样成立；
- tmux + 恢复模型（第 7 章）兜底：即使 server 进程被杀，下次启动现场可恢复；
- **战略价值**：Local 模式是零配置入口（install → `kumokara` → 自动打开
  浏览器），是用户试用与口碑扩散的路径；用爽之后一键部署到服务器，
  数据格式相同，workspace 可迁移。
- **Token 生命周期**：Local 模式的"一次性 token"指单次 server 进程生命周期内有效——
  启动器通过 `--token` 传给 server，存于内存，server 关闭即失效。浏览器刷新页面时
  启动器自动重新注入当前 token，用户无感知。

**首次启动体验**（`kumokara` 命令）：

```
$ kumokara
Kumokara（雲殻） v0.1.0 — Agents never sleep in Kumokara.
✓ tmux 3.5 detected (session recovery: enabled)
✓ Workspace directory: ~/.kumokara/workspaces/
→ Server listening on http://127.0.0.1:9876
→ Opening browser...
```

启动流程：
1. 检测 tmux 版本 → 显示恢复能力状态（有/无）
2. 创建 `~/.kumokara/` 目录结构（若不存在）
3. 生成初始 token → 打印到 stdout（首次）+ 写入 config.yaml
4. 启动 axum server（绑定 127.0.0.1）
5. 自动打开浏览器 → 首页为 workspace 列表（首次为空 → 显示"创建第一个 Workspace"引导）

若 tmux 未安装，提示：
```
⚠ tmux not found — session recovery disabled. Install tmux for 24h agent persistence.
```

---

## 3. Core Concepts

### 3.1 概念关系

```
Workspace（项目目录，持久，第一公民）
├── Session 0..N（PTY 终端会话，易逝但可恢复）
│    ├── shell session     普通终端
│    └── agent session     运行 claude / codex / opencode 的终端
│                          （记录 cli_session_id，支持 resume / fork）
├── event_log（结构化事件流，SQLite 持久化）
└── output buffer（原始终端输出，环形缓冲，易逝）
```

设计原则：**持久化边界 = 产品价值**。PTY 随 Server 重启消亡，能 24h 存活的是
文件、env、Agent 配置、任务队列、事件历史——这些构成 Workspace。Session 是
易逝的执行体，Workspace 是长寿的身份。

### 3.2 Workspace

```
Workspace
├── id              UUID
├── name            "my-saas"
├── status          Ready / AgentRunning / AgentWaiting / Error（聚合自各 session；workspace 表冗余 cached_status 字段，事件驱动增量更新，避免每次列表查询遍历全部 session）
├── work_dir        ~/.kumokara/workspaces/{id}/files
├── env             { OPENAI_API_KEY, DATABASE_URL, ... }  # 文件权限 0600
├── agent_config
│   ├── provider    claude_code | codex | opencode
│   ├── model       claude-sonnet-4-20250514
│   ├── system_prompt  (optional)
│   └── permissions { allow_shell: true, ... }  # 仅用于生成 CLI 原生权限配置（见 6.5）
├── sessions        0..N 个 Session（见 3.3）
├── event_log       结构化事件流（见 3.5）
└── clients         当前连接的 WebSocket clients（可能为 0）
```

**Workspace 生命周期**：

```
            create
               │
               ▼
            Ready ───────────────┐
               │                 │
          agent/spawn            │ pause（手动 / 磁盘满 / 长时间空闲）
               │                 │
               ▼                 ▼
         AgentRunning         Paused ──→ resume ──→ Ready
               │
        task complete / idle
               │
               ▼
            Ready
               │
          destroy
               ▼
            (deleted)
```

- **Paused**：暂停所有 session 的 PTY 输出缓冲（tmux detach），释放活跃连接资源。
  触发条件：用户手动暂停、磁盘空间低于阈值、空闲超过可配时长（默认 24h）
- **Resume**：重连 tmux session、回放增量事件、恢复 Agent 上下文（如果 agent 仍在运行）
- **Server 重启后**：所有非 Paused 的 Workspace 自动恢复（tmux 重连 + agent `--resume`）；
  Paused 的 Workspace 需手动恢复
- **销毁**：先关 PTY 进程 → 清理 tmux session → 删除 work_dir → 删除 DB 记录，不可逆

**资源限制**（per workspace，Phase 3 引入）：

| 约束项 | 默认值 | 可配 | 超限行为 |
|--------|--------|------|---------|
| 最大 Workspace 数 | 10 | 是 | 拒绝创建，`WORKSPACE_QUOTA_EXCEEDED` |
| 每个 Workspace 最大 Session 数 | 8 | 是 | 拒绝创建，`SESSION_LIMIT_EXCEEDED` |
| 每个 Workspace 磁盘配额 | 10 GB | 是 | 事件写入降级为 warn，通知用户清理 |
| 全局磁盘配额 | 总磁盘的 80% | 是 | 自动 pause 最久未活跃的 workspace |
| CPU / 内存 | 不做硬限制 | — | Phase 4 可选 cgroup v2 沙箱 |

### 3.3 Session

Session 是 Workspace 内的一个终端会话（PTY 封装），是用户和 Agent 实际操作的对象。
**Agent 会话以项目目录为单位关联**——Session 永远属于某个 Workspace，不存在游离的
Session。Otty 的 tab 可以"按 git 项目分组"，Kumokara 把这个分组直接提升为数据模型本身。

```
Session
├── id            UUID
├── workspace_id  所属 Workspace（项目目录）
├── type          shell | agent
├── agent         (type=agent 时) { provider, cli_session_id, model }
├── title         跟随前台程序（OSC 0/1/2），可 rename / prefix 覆盖
├── state         active / background / exited
├── created_at    创建时间（UTC）
├── last_active_at 最后活跃时间（UTC，驱动侧栏 frecency 排序）
├── pty           PTY 进程句柄（由 tmux 包裹，见第 7 章）
└── output_seq    原始输出序号（用于增量同步）
```

- 每个 Workspace 可以有 0..N 个 Session；无 client 在线时 agent session 继续运行
- agent session 记录 CLI 原生 session id（`cli_session_id`），支持 resume / fork（见 6.3）
- 每个 session 注入环境变量 `$KUMOKARA_SESSION_ID`，供脚本与 Agent 跨会话协作（见 7.5）

### 3.4 Agent

Agent 是运行在 agent session 中的 AI 编码助手进程。与 Otty 的区别：

| | Otty | Kumokara |
|---|---|---|
| Agent 生命周期 | 绑定桌面应用进程 | 绑定 Server 进程，独立于客户端 |
| 关闭客户端 | Agent 终止 | Agent 继续运行 |
| 离线通知 | 无（应用已关闭） | Web push / webhook |
| 多设备 | ❌ 仅桌面 | ✅ 任意浏览器 |

**Agent 状态机**（Kumokara **观察到**的状态，而非受控状态机——状态迁移由
Agent CLI 自身决定，见 6.5）：

```
  Idle ──→ Running ──→ WaitingApproval ──→ Running
    ↑         │              │                │
    │         └──→ Error ────┘                │
    │         │              │                │
    └─────────┴──────────────┴──→ Completed ──┘
```

状态枚举说明：

| 状态 | 含义 | 触发 |
|------|------|------|
| `Idle` | Agent 未运行，等待任务或手动启动 | 初始状态 / 任务队列空 |
| `Running` | Agent 正在执行任务 | 收到 prompt / `agent_start` |
| `WaitingApproval` | Agent 在执行过程中等待用户确认 | CLI hook 上报 `awaiting_input` |
| `Error` | Agent 进程异常退出或启动失败 | 进程 crash / `AgentError` 事件 |
| `Completed` | 当前任务正常完结，回到 Idle | hook 上报 `completed` / 队列空 |

> 完整生命周期流程（从 Server boot → spawn → task dispatch → 完成/重启）见 §6.4。

### 3.5 Event Log — 两层模型

终端数据分两层，**分开存储、不同寿命**——这是徽标、大纲、通知、审计的统一数据源：

**第一层：结构化事件**（per workspace，SQLite，长期保留，可配 retention）

```
event_log
├── SessionCreated    { session_id, type, title }
├── SessionDestroyed  { session_id }
├── CommandStarted    { session_id, cmd, cwd }              # 来自 OSC 133;C
├── CommandFinished   { session_id, exit_code, duration_ms } # 来自 OSC 133;D
├── CwdChanged        { session_id, path }                  # 来自 OSC 7
├── AgentStarted      { session_id, provider, model, cli_session_id }
├── AgentStateChanged { session_id, state }    # processing / idle / awaiting_input（CLI hooks）
├── AgentTask         { session_id, task_id, prompt }
├── AgentApproval     { session_id, action_id, description, decision }
├── AgentCompleted    { session_id, task_id, summary }
├── AgentError        { session_id, task_id?, error }
└── WorkspaceEvent    { ... }
```

**第二层：原始终端输出**（per session，内存 + 磁盘环形缓冲，有容量上限）

```
output_buffer
└── OutputChunk { session_id, seq, data }  → 用于 attach 回放、screen dump、delta sync
```

分层理由：原始输出量大且易逝（一次编译刷屏几万行），只用于传输与回放；
结构化事件量小且永久，支撑查询、通知与审计。两者通过 `seq` 关联时间序。

所有事件经 Workspace 内事件总线广播，支持：
- 客户端重连后按 seq 回放增量
- 审计与回溯
- 驱动 UI 徽标、大纲、通知

**事件来源标识**：每个事件统一携带 `source` 字段，标明信号来源：
- `shellint` — 来自 OSC 序列解析（命令边界、退出码、cwd）
- `agent_hook` — 来自 Agent CLI hook 回调（状态变更、任务事件）
- `server` — 来自 Kumokara 自身的状态推导（session 创建/销毁等）

当徽标状态与实际不符时，`source` 是排查"信号由谁发出"的第一手线索。

### 3.6 终端可观测性（Shell Integration）

Otty 的实践：一切"智能"特性（大纲、退出码徽标、命令导航、cwd 跟踪）的数据底座
是注入 shell 的 **OSC 133（FTCS）与 OSC 7** 标记。Kumokara 在服务端做同样的事，
且更彻底——Server 直接控制 PTY 的启动环境，注入比桌面端更简单：

| 序列 | 时机 | 生成的事件 |
|------|------|-----------|
| `OSC 133 ; A` | 绘制 prompt 前 | （prompt 边界，内部使用） |
| `OSC 133 ; C` | 命令开始执行 | `CommandStarted` |
| `OSC 133 ; D ; <exit>` | 命令结束 | `CommandFinished{exit_code}` |
| `OSC 7 ; file://<host><cwd>` | 每个 prompt | `CwdChanged` |

- 注入脚本内置于 `kumokara-shellint` crate（zsh / bash / fish），随 session 启动装载
- `KUMOKARA_DISABLE_INTEGRATION=1` 可按 session 关闭；shell 不支持时优雅降级——
  终端功能完整，只是没有结构化事件
- 与 Agent hooks（6.3）互补：OSC 感知**命令层**，hooks 感知 **Agent 层**

---

## 4. Architecture

### 4.1 Tech Stack

从头选型（旧 kara 代码全部丢弃，不继承任何实现）：

| 层 | 选择 | 理由 |
|----|------|------|
| 服务端语言 | **Rust** | 长驻 daemon 的稳定性与类型安全；单二进制分发，自部署友好 |
| 异步运行时 | **tokio** | Rust 异步事实标准，后续所有库的生态前提 |
| HTTP / WebSocket | **axum** | tokio 原生、类型化 extractor、WS 支持成熟 |
| PTY | **portable-pty** | wezterm 出品，跨平台，久经考验 |
| 会话持久层 | **tmux**（control mode `-C`） | 最成熟且无处不在；Server 重启后重连即恢复；用户可手动 `tmux attach` 自救。未安装时降级为纯 portable-pty（无崩溃恢复），启动时检测并友好提示 |
| 数据库 | **SQLite**（via **sqlx**，WAL 模式） | 单机部署零依赖；sqlx 编译期检查 SQL；WAL 支持并发读 + 单写者 |
| SSH | **russh** | 纯 Rust 异步 SSH，无 C 依赖 |
| 序列化 / 配置 | serde + serde_json / serde_yaml | 标准选择 |
| CLI | **clap** | 标准选择 |
| Web 通知 | web-push crate（VAPID） | Phase 2 |
| 前端框架 | **React + Vite** | 生态最大，xterm.js 封装与招聘/协作最容易 |
| 终端组件 | **xterm.js**（@xterm/xterm + fit/search/webgl addons） | Web 终端事实标准，无替代品 |
| 前端状态 | zustand（轻量） | 单 store 即可覆盖 workspace/session 状态树 |
| 跨平台 App（长期） | **Tauri 2.x** | 与服务端同为 Rust 生态；桌面可内嵌 server sidecar；同一代码库覆盖桌面+iOS/Android（见 8.6） |

> 各项选型的详细权衡与备选方案见 §11 Design Decisions Log。

### 4.2 System Architecture

```
┌──────────────────────────────────────────────────────┐
│                    CLIENTS                            │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐           │
│  │ Desktop  │  │  Mobile  │  │  Tablet  │           │
│  │ Browser  │  │ Browser  │  │ Browser  │           │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘           │
│       └──────────────┼──────────────┘                │
│                      │ WebSocket + REST               │
└──────────────────────┼──────────────────────────────┘
                       │
┌──────────────────────┼──────────────────────────────┐
│              Kumokara Server (axum)                   │
│  ┌───────────────────┴───────────────────┐           │
│  │         Auth / API Gateway             │           │
│  └───────────────────┬───────────────────┘           │
│  ┌───────────────────┴───────────────────┐           │
│  │         Workspace Manager              │           │
│  │  ┌──────────────────────────────────┐ │           │
│  │  │  Workspace A                     │ │           │
│  │  │  ├── Session 1 (agent, tmux PTY) │ │           │
│  │  │  ├── Session 2 (shell, tmux PTY) │ │           │
│  │  │  ├── Event Bus                   │ │           │
│  │  │  ├── Event Log (SQLite)          │ │           │
│  │  │  └── Output Ring Buffer          │ │           │
│  │  └──────────────────────────────────┘ │           │
│  │  ┌──────────────────────────────────┐ │           │
│  │  │  Workspace B  ...                │ │           │
│  │  └──────────────────────────────────┘ │           │
│  └───────────────────────────────────────┘           │
│                                                       │
│  ┌──────────────────────────────────────────────────┐│
│  │ PTY 输出管道（按序处理）                             ││
│  │ tmux control %output 解包 → OSC 133/7 解析          ││
│  │ → 结构化事件 + 原始输出环形缓冲                        ││
│  └──────────────────────────────────────────────────┘│
│  ┌───────────────┐  ┌───────────────┐                │
│  │ OSC Parser    │  │ Agent Hooks   │                │
│  │ (shellint)    │  │ Receiver      │                │
│  └───────────────┘  └───────────────┘                │
│  ┌───────────────────────────────────────┐           │
│  │  SSH Connector → 远程服务器            │           │
│  └───────────────────────────────────────┘           │
└──────────────────────────────────────────────────────┘
```

### 4.3 Crate Structure

```
crates/
├── kumokara-protocol/    # 协议定义
│   ├── messages.rs       #   所有 WebSocket 消息类型（含 request_id 约定）
│   ├── workspace.rs      #   WorkspaceInfo, SessionInfo, AgentConfig
│   └── event.rs          #   EventLog 事件类型
│
├── kumokara-engine/      # PTY 引擎
│   ├── session.rs        #   portable-pty 封装 + tmux control mode 管理
│   └── lib.rs
│
├── kumokara-shellint/    # Shell integration（OSC 133/7）
│   ├── scripts/          #   zsh / bash / fish 注入脚本
│   ├── inject.rs         #   PTY 启动环境注入
│   └── parse.rs          #   OSC 序列 → 结构化事件
│
├── kumokara-workspace/   # Workspace 生命周期管理
│   ├── workspace.rs      #   Workspace 创建/销毁/暂停/恢复
│   ├── session.rs        #   Session 生命周期（attach/detach/恢复）
│   ├── filesystem.rs     #   文件系统隔离（per-workspace 目录）
│   ├── env.rs            #   环境变量管理
│   └── config.rs         #   Workspace 配置持久化
│
├── kumokara-agent/       # Agent 编排层（核心）
│   ├── runtime.rs        #   Agent 进程生命周期管理
│   ├── detector.rs       #   Agent 状态感知（前台进程检测 + CLI hooks + 输出解析）
│   ├── queue.rs          #   任务队列（prompt queue）
│   ├── approval.rs       #   审批提示检测，将用户选择转发为终端按键
│   ├── context.rs        #   Agent 上下文构建（从 event_log 构建）
│   └── providers/        #   LLM Provider 适配
│       ├── mod.rs        #   Provider trait 定义
│       ├── claude.rs     #   Claude Code CLI 封装
│       ├── codex.rs      #   OpenAI Codex CLI 封装
│       └── opencode.rs   #   OpenCode 集成
│
├── kumokara-event/       # 事件总线与持久化
│   ├── bus.rs            #   Workspace 内事件广播
│   ├── log.rs            #   SQLite 事件日志 CRUD + retention（sqlx）
│   └── buffer.rs         #   原始输出环形缓冲
│
├── kumokara-ssh/         # SSH 远程连接
│   ├── connector.rs      #   SSH 连接管理（基于 russh）
│   └── target.rs         #   SSH 目标配置
│
├── kumokara-auth/        # 认证授权
│   ├── middleware.rs     #   Token / API Key 中间件
│   └── users.rs          #   用户管理
│
├── kumokara-server/      # 编排层（axum）
│   ├── ws_handler.rs     #   WebSocket 连接处理
│   ├── api/              #   REST API 路由
│   │   ├── workspace.rs  #   Workspace CRUD
│   │   ├── agent.rs      #   Agent 控制 API + hooks 回调接收
│   │   └── events.rs     #   Event 查询 API
│   └── lib.rs
│
└── kumokara-cli/         # CLI（clap）
    ├── main.rs
    └── commands/
        ├── local.rs      #   kumokara（默认）= Local 模式启动器：拉起 server + 打开浏览器
        ├── server.rs     #   kumokara server 子命令（守护进程）
        ├── workspace.rs  #   kumokara workspace 子命令
        └── agent.rs      #   kumokara agent / agent-event 子命令（hooks 回调通道）

web/                       # 前端 SPA（React + Vite + xterm.js）
├── index.html
├── src/
│   ├── App.tsx
│   ├── components/
│   │   ├── Terminal.tsx          # xterm.js 终端组件
│   │   ├── WorkspacePanel.tsx    # Workspace + Session 两级侧栏（含 badge）
│   │   ├── SessionHeader.tsx     # Session 标题栏（OSC 跟随 / rename / prefix）
│   │   ├── DetailsPanel.tsx      # 右侧面板：大纲 / 事件时间线 / 任务队列
│   │   ├── PromptComposer.tsx    # Prompt 输入与附件（仅 agent session 显示）
│   │   └── ApprovalInline.tsx    # 终端流内联审批按钮
│   ├── hooks/
│   │   ├── useWebSocket.ts
│   │   └── useWorkspace.ts
│   └── store/
│       └── workspaceStore.ts
└── package.json
```

### 4.4 Data Directory

```
~/.kumokara/
├── config.yaml                  # 全局配置（含访问 token）
├── kumokara.db                  # 全局数据库（用户、workspace 索引）
├── shellint/                    # shell integration 注入脚本
├── workspaces/
│   ├── {workspace_id}/
│   │   ├── workspace.yaml       # workspace 配置（env 文件权限 0600）
│   │   ├── files/               # 工作目录
│   │   ├── events.db            # 结构化事件日志（SQLite）
│   │   └── sessions/
│   │       └── {session_id}.log # 原始输出环形缓冲（有上限）
│   └── ...
└── auth/
    ├── api_keys.db              # API Key 管理
    └── sessions.db              # 用户登录 session
```

备份 = 备份 `~/.kumokara/` 整个目录。注意：直接 `cp` 正在写入的 SQLite 可能拿到
损坏的备份，建议通过 `sqlite3 .backup` 或 `VACUUM INTO` 导出后再打包。

**版本升级与 Migration**：数据格式随版本迭代变化，启动时检测并自动迁移：

- SQLite 数据库使用 `PRAGMA user_version` 存储 schema version
- 启动时读取 `user_version`，按序执行未应用的 migration（Rust 内嵌 SQL 文件，类似 Rails migration）
- YAML 配置文件在 `workspace.yaml` 顶层新增 `schema_version` 字段，启动时做兼容性检查
- 迁移前自动备份（copy `.db` → `.db.bak.{version}`），迁移失败可回滚
- 破环性升级（主版本号变更）要求用户手动确认

---

## 5. Protocol

客户端 ↔ 服务端通信通过**单一 WebSocket 连接**：`wss://host/api/ws`。

### 5.1 通用约定

- **认证**：连接后首条消息必须为 `auth { token }`（不放 URL query，避免泄露进
  代理日志与浏览器历史）；认证失败即关闭连接
- **请求关联**：所有 Client→Server 请求带 `request_id`，响应与 `error` 回显，
  支持并发请求与幂等重试
- **双通道编码**：控制消息走 text frame（JSON tagged enum）；终端 IO 走
  **binary frame**（24 字节固定头部：16 bytes session_id UUID + 8 bytes seq u64 big-endian；
  剩余为终端原始输出），避免 JSON 编码高频大块数据
- **错误**：`error { request_id?, code, message }`，`code` 为稳定枚举，前端重连、幂等重试
  均依赖稳定错误码：
  
  | 错误码 | 含义 | 重试策略 |
  |--------|------|---------|
  | `AUTH_INVALID` | Token 无效或过期 | 不重试，提示重新认证 |
  | `WORKSPACE_NOT_FOUND` | Workspace 不存在 | 不重试 |
  | `WORKSPACE_QUOTA_EXCEEDED` | Workspace 数量/资源超限 | 不重试，提示升级/清理 |
  | `SESSION_NOT_FOUND` | Session 不存在 | 不重试 |
  | `SESSION_LIMIT_EXCEEDED` | Workspace 下 Session 数达上限 | 不重试 |
  | `AGENT_NOT_AVAILABLE` | Agent CLI 未安装或不可用 | 不重试，提示安装 |
  | `AGENT_ALREADY_RUNNING` | 同一 workspace 已有 agent 运行 | 不重试 |
  | `INTERNAL_ERROR` | 服务端内部错误 | 可重试（exponential backoff） |
  | `RATE_LIMITED` | 请求频率超限 | 重试（after Retry-After 秒） |

### 5.2 Workspace Management

```
Client → Server:
  create_workspace    { request_id, name, env?, agent_config? }
  list_workspaces     { request_id }
  get_workspace       { request_id, workspace_id }
  destroy_workspace   { request_id, workspace_id }
  update_workspace    { request_id, workspace_id, name?, env?, agent_config? }

Server → Client:
  workspace_created   { request_id, workspace: WorkspaceInfo }
  workspace_list      { request_id, workspaces: [WorkspaceInfo] }
  workspace_updated   { workspace_id, workspace: WorkspaceInfo }
  workspace_destroyed { workspace_id }
```

### 5.3 Session & Terminal IO

```
Client → Server:
  session_create  { request_id, workspace_id, type, cols, rows }  # type: shell | agent
  session_list    { request_id, workspace_id }
  session_attach  { request_id, session_id, last_seq? }
  session_detach  { session_id }
  session_destroy { request_id, session_id }
  terminal_input  { session_id, data }        # binary frame
  terminal_resize { session_id, cols, rows }

Server → Client:
  session_created   { request_id, workspace_id, session: SessionInfo }
  session_list      { request_id, sessions: [SessionInfo] }
  terminal_output   { session_id, seq, data } # binary frame
  screen_dump       { session_id, cols, rows, cursor, content, seq }
  session_destroyed { session_id }
```

`session_attach` 语义：服务端先回 `screen_dump`（当前屏幕快照），再从
`last_seq` 增量回放——重连成本与历史长度无关。若 `last_seq` 已被环形缓冲覆盖
（客户端离线过久），则丢弃增量，仅返回 `screen_dump` + 当前最小可用 seq 作为
新起点（附 `gap_detected: true` 标记，UI 可在状态栏短暂提示"输出历史不连续"）。

### 5.4 Agent Control

```
Client → Server:
  agent_start         { request_id, workspace_id, provider? }   # 新建 agent session
  agent_stop          { request_id, session_id }
  agent_send_prompt   { request_id, session_id, prompt, attachments?: [Attachment] }
  agent_approve       { request_id, session_id, action_id }
  agent_reject        { request_id, session_id, action_id, reason? }
  agent_cancel_task   { request_id, session_id, task_id }
  agent_pause_queue   { request_id, session_id }
  agent_resume_queue  { request_id, session_id }

Server → Client:
  agent_status          { session_id, status, current_task?, queue_length }
  agent_progress        { session_id, task_id, step, output }
  agent_approval_needed { session_id, action_id, description, risk_level, context }
  agent_task_completed  { session_id, task_id, summary }
  agent_task_failed     { session_id, task_id, error }
  agent_message         { session_id, role, content }
```

> `agent_approve` / `agent_reject` 的语义是**按键转发**：服务端把用户选择翻译为
> 对应的终端输入注入 Agent 所在的 PTY，等价于用户亲手在终端里回答 Agent CLI
> 的原生确认提示。`agent_approval_needed` 是"检测到等待审批"的通知，而非拦截事件。

### 5.5 Event Stream

```
Client → Server:
  event_subscribe   { workspace_id }
  event_unsubscribe { workspace_id }
  event_query       { request_id, workspace_id, after_seq?, limit?, types? }

Server → Client:
  event_batch       { request_id, workspace_id, events: [EventEntry] }
  event_live        { workspace_id, event: EventEntry }
```

---

## 6. Agent Integration

### 6.1 集成哲学

参考 Otty：**Kumokara 不重新实现 Agent，也不解析其内部行为**——用户照旧使用
`claude` / `codex` / `opencode`，Kumokara 向每个 Agent 自身的配置里安装一个
hook / plugin，让 Agent 主动把每个 turn 的状态（processing / idle /
awaiting_input）汇报回来。状态信号驱动徽标、通知、history 与 resume。

### 6.2 Provider Trait

```rust
#[async_trait]
pub trait AgentProvider: Send + Sync {
    /// Provider identifier ("claude_code" | "codex" | "opencode").
    fn name(&self) -> &str;

    /// Check if this agent CLI is installed on the system.
    async fn check_available(&self) -> Result<bool>;

    /// Install Kumokara's state-reporting integration into the agent's own
    /// config (hooks / plugin). Only touches Kumokara-owned entries.
    async fn install_integration(&self, workspace: &Workspace) -> Result<()>;

    /// Remove Kumokara's entries, leaving the rest of the config untouched.
    async fn uninstall_integration(&self, workspace: &Workspace) -> Result<()>;

    /// Build the launch command for start / resume / fork.
    fn launch_command(&self, mode: LaunchMode, session_id: Option<&str>) -> Vec<String>;

    /// Locate the agent's transcript/session files (for History & audit).
    fn transcript_paths(&self, workspace: &Workspace) -> Result<Vec<PathBuf>>;
}
```

### 6.3 Integration Mechanism（参考 Otty）

每个 Provider 的集成方式、resume / fork 命令：

| Agent | 启动命令 | 集成方式 | Resume | Fork |
|-------|---------|---------|--------|------|
| Claude Code | `claude` | hooks → `~/.claude/settings.json` | `claude --resume <id>` | `claude --resume <id> --fork-session` |
| Codex | `codex` | hooks → `~/.codex/hooks.json`（需 `config.toml` 开启 `hooks = true`） | `codex resume <id>` | `codex fork <id>` |
| OpenCode | `opencode` | plugin → `~/.config/opencode/plugins/` | `opencode --session <id>` | `opencode --fork --session <id>` |

**状态回报通道**：Otty 桌面版的 hook 直接通知本地 App；Kumokara 的 hook 需要回连 Server：

- 首选：hook 执行 `kumokara agent-event --workspace <id> --session <id> --state <state>`
  （CLI 子命令，内部 POST 到本机 Server）
- 备选：hook 直接 `POST /api/workspaces/{id}/agent-events`（携带 per-workspace token）
- SSH target 场景：hook 在远端机器执行，需经 SSH reverse tunnel 或 HTTPS endpoint 回连

**状态信号 → 行为映射**（per-workspace 可配，类比 Otty 的 Agent Behavior 设置）：

| 信号 | UI | 离线通知 |
|------|----|---------|
| `processing` | Workspace 列表 badge → Running | — |
| `idle` / `completed` | badge → Idle | web push（可开关） |
| `awaiting_input` | badge → Waiting + 终端内联审批 | web push（可开关） |

**其他借鉴点**：

- **自定义 launch command**：允许覆盖启动命令以支持包装器、绝对路径、全局 flag
  （如 `claude --dangerously-skip-permissions`），session 参数由 Kumokara 追加；
- **History**：Agent 的 transcript（如 Claude Code 的 `~/.claude/projects/**/*.jsonl`）
  可检索、可 resume，会话上下文不随终端关闭而丢失；
- **一键卸载**：uninstall 只移除 Kumokara 写入的条目，不动用户其余配置；
- **已知限制**：OpenCode 通过 `/sessions` 切换会话时不广播 session id，
  需发送一条消息后集成才能关联（Otty 同样有此限制）。

### 6.4 Agent Process Lifecycle

```
                         Server boot
                             │
                   workspace.auto_start?
                        yes  │
                   ┌─────────┴─────────┐
                   │  AgentRuntime::    │
                   │  spawn_agent()     │
                   └─────────┬─────────┘
                             │
                    ┌────────┴────────┐
                    │  Agent Process  │
                    │  (tmux PTY)     │
                    └────────┬────────┘
                             │
              ┌──────────────┼──────────────┐
              │              │              │
              ▼              ▼              ▼
           Running      WaitingApproval  Error/Crash
              │              │              │
              │    ┌────────┴────────┐      │
              │    │ User approves?  │      │
              │    │ yes → resume    │      │
              │    │ no  → reject    │      │
              │    └─────────────────┘      │
              │              │              │
              └──────────────┴──────┬───────┘
                                    │
                              Task Complete
                                    │
                            Queue empty?
                            yes → idle
                            no  → next task
```

> 图中的 "User approves" 发生在 Agent CLI 自己的终端界面内。Kumokara 不拦截，
> 只负责检测"等待审批"状态并通知到各端（见 6.5）。

### 6.5 Safety Model — 不干预，只适配

**Kumokara 不做安全围栏**（详见 §11 Design Decision Log），只做感知与适配：

| Kumokara 做什么 | Kumokara 不做什么 |
|---|---|
| 检测前台进程树，识别 Agent CLI 启动，更新 Workspace 状态 | 拦截或审计 Agent CLI 的工具调用 |
| 通过 CLI hooks 感知"等待审批/完成/出错"，驱动 UI 与通知 | 实现自己的安全策略（黑名单、权限沙箱等） |
| 提供 UI 按钮——等价于代为按键，注入 PTY | 替代 Agent CLI 自身的审批流程 |
| 记录结构化事件与终端输出，供事后审计（§3.5） | — |
| 在用户同意下生成 Agent CLI 原生权限配置（如 `.claude/settings.json`） | 强制执行权限配置 |

安全边界完全由 Agent CLI 自身的权限系统负责（Claude Code 的 permission rules /
hooks、Codex 的 sandbox / approval policy）。

---

## 7. Reliability — Server 重启恢复模型

"Agent 24h 在线"的技术兑现，参考 Otty 的 Session Recovery 四层模型：

1. **进程层**：session 的 PTY 由 tmux（或等价 multiplexer）包裹——Server 重启后
   重连 tmux，终端与其中进程不丢；
2. **Agent 层**：agent session 凭 `cli_session_id` 走 `--resume` 恢复上下文（见 6.3）；
3. **服务层**：长驻进程（dev server 等）按**前缀白名单**决定是否在恢复时重跑，
   默认 None；
4. **熔断**：同一 session 连续恢复失败 N 次，退化为全新启动并通知用户，
   打破崩溃循环。

快照策略（借自 Otty）：布局/状态快照在**变更时增量写入**（session 创建、销毁、
标题变化），而非周期性全量写——崩溃损失以秒计。

---

## 8. Web UI Design

设计参照 Otty：优雅、简洁、信息密度克制。概念映射：

| Otty（桌面） | Kumokara（Web） |
|---|---|
| window | 浏览器页面 |
| 垂直 tab 侧栏（可按项目分组） | Workspace 侧栏（项目 = 第一公民，分组即数据模型） |
| tab → pane | Workspace → Session（终端 / agent 会话） |
| tab badge | Workspace / Session 状态徽标 |
| details panel | 右侧详情面板（大纲 / 事件时间线 / 任务队列） |

### 8.1 Layout

```
┌──────────────────────────────────────────────────────────────┐
│  Kumokara ☁                                  ● Connected     │ ← Top Bar（仅品牌 + 连接态）
├──────────────────┬───────────────────────────────────────────┤
│  WORKSPACES      │  ⠿ claude — my-saas                   ⌄  │ ← Session 标题（跟随 OSC，可改名）
│                  ├───────────────────────────────────────────┤
│ ▾ ● my-saas      │                                           │
│    ⠿ claude   ✋  │   $ claude                                │
│    ⠿ codex    ⠿  │   > Let's build the auth module           │
│    $  shell       │   > Created src/auth/mod.rs               │
│ ▸ ○ api-srv      │   > Running cargo test...                 │
│ ▸ △ frontend     │                                           │
│                  │   ── ✋ Waiting for input ───────────────   │
│ [+ New]          │   git push origin main?                   │
│                  │   [Approve]  [Reject]                     │
│                  ├───────────────────────────────────────────┤
│                  │  Composer › Write tests for auth…     [⏎] │ ← 仅 agent session 显示
├──────────────────┴───────────────────────────────────────────┤
│  seq:142 · 12:34 UTC                             [☰ Details] │ ← Status Bar
└──────────────────────────────────────────────────────────────┘
```

**侧栏规则**：

- Workspace 行：名称 + 聚合徽标（优先级：✋ 等待 > △ 出错 > ⠿ 忙碌 > ○ 空闲），点击展开/折叠其 Session 列表
- Session 行：图标区分类型——`$` = shell session（纯终端）、`⠿` = agent session（AI 编码助手运行中）；标题跟随前台程序，行尾徽标显示自身状态
- 排序：默认最近活跃优先（frecency），可拖拽手动排序；折叠状态与排序跨重连保留

**主视图规则**：

- 一个 Session 一个终端视图，v1 不做分屏（保持 Otty 式克制，分屏列入 Phase 4）
- Composer 仅在聚焦 agent session 时显示；shell session 就是纯终端，无任何附加 UI
- 审批提示内联在终端流中呈现（如上图），不打断操作路径
- 任务队列、大纲、事件时间线收进右侧 Details 面板（`☰` 开合），不占主视图

### 8.2 Badge 语义（借自 Otty）

| 徽标 | 含义 |
|------|------|
| ⠿ spinner | 会话忙碌中（agent 工作 / 长命令运行） |
| ✓ checkmark | 任务刚完成（短暂显示后消失） |
| ✋ hand | Agent 等待你的输入或审批 |
| △ triangle | 命令或 Agent 出错（`CommandFinished` exit≠0） |
| ○ dot | 空闲 |

当前聚焦的 session 不显示 ✓（完成提醒只标记**无人照看**的活动）；⠿ 和 ✋
始终显示，因为它们反映进行中的状态。

### 8.3 标题与命名

- Session 标题默认跟随前台程序发出的 OSC 0/1/2；agent session 初始标题为 provider 名（如 `claude`）
- 右键可 **Rename**（固定标题，忽略后续 OSC 更新）或 **Prefix**（前缀 + 动态标题，如 `prod: claude`）
- Workspace 名在创建时指定，随时可改（路径锚定 UUID，改名不影响 work_dir）

### 8.4 Key Interaction Patterns

1. **Prompt → Agent 执行**：聚焦 agent session，在底栏 Composer 输入自然语言，终端画面实时显示执行过程
2. **排队任务**：Agent 忙时新 prompt 进入队列（Details 面板可见、可调整顺序），按序执行
3. **审批内联**：检测到 Agent CLI 的原生确认提示时，终端流内嵌 [Approve]/[Reject]
   （按钮等价于代为按键，见 6.5），同时 session 打上 ✋ 徽标，无客户端在线则发 web push
4. **多项目并行**：侧栏切换 Workspace，各项目下的 session 在后台独立运行，聚合徽标让你不用切过去就知道哪个项目需要你
5. **大纲导航**：Details 面板显示当前 session 的命令 + agent prompt 混合索引
   （数据来自 OSC 133 与 CLI hooks），退出码三色指示，点击跳转
6. **事件回溯**：Details 面板查看该 Workspace 的全部历史事件，支持按类型筛选和搜索

### 8.5 ADE 功能清单（参考 Otty 全景调研）

对 Otty 文档（User Interface / Workflows / Terminal Features 全量）的调研结论——
一个成熟 ADE 需要的功能域，及 Kumokara 的采纳决策：

| 功能域 | Otty 的做法 | Kumokara 采纳 | Phase |
|--------|------------|--------------|-------|
| 终端可观测性 | shell integration 注入 **OSC 133**（命令边界 + 退出码）/ **OSC 7**（cwd），一切智能特性的数据底座 | 服务端向 PTY 注入同等 integration，解析为结构化事件写入 event_log（见 3.6） | 1 |
| 大纲导航 | Outline / Jump To：命令 + agent prompt 混合建索引，退出码三色指示 | Session 大纲：统一时间线索引，Details 面板呈现 | 2 |
| Agent 会话历史 | 识别 `.jsonl` transcript 渲染为对话流（含 tool calls），一键 Resume | History 视图：transcript 渲染 + resume / fork（走 6.2 的 `launch_command`） | 2 |
| 防误触 | 终端里 click 无动作，修饰键 + click 才打开链接 | 同样策略（Web 终端"选择文本 vs 打开链接"冲突相同） | 2 |
| 路径交互 | `file:line:col` 检测，点击跳到内置编辑器对应行 | 终端输出中的路径可点击，打开内置文件视图并定位行号 | 3 |
| 文件 / Git | Details Panel 四页：Info（进程/端口/cwd）/ Outline / Git（内联 diff）/ Files（文件树），跟随聚焦 pane | 同源照搬，面板内容跟随聚焦 session | 3 |
| 全局搜索 | `⇧⌘F` 跨 tab 搜 scrollback，结果聚合 + 点击跳转 | 跨 session 全局搜索（24h 多 agent 输出量更大，价值更高） | 3 |
| 双面板入口 | Open Quickly（跳转到东西）与 Command Palette（执行动作）分离，frecency 排序 | `⌘K` 统一入口：workspace / session / agent 会话跳转 + 动作（Resume/Fork/Copy Session ID），frecency 排序 | 3 |
| 崩溃恢复 | 三件套：tmux 重连 + agent `--resume` + 进程重跑白名单；连续崩溃退化为全新启动 | Server 重启恢复模型（见第 7 章） | 1（基础）/ 3（完整） |
| 会话 IPC | `otty pane send-text / run / exec` 三件套 + 每 pane 注入 `$OTTY_PANE_ID`；run=只要成败、exec=要输出、send=按键 | 每 session 注入 `$KUMOKARA_SESSION_ID`；REST/CLI 暴露同等三件套，agent 可跨 session 协作 | 3 |
| 任务编排 | `otty watch:claude <id>` 阻塞等待 agent 空闲，可串联多 agent | 队列依赖原语：prompt 可声明"等 session X 空闲后执行" | 3 |
| 安全边界 | IPC 默认关闭；SSH / sudo 敏感会话需二次授权 | SSH target / sudo session 默认禁用 IPC 与按键转发（见第 9 章） | 3 |
| 工作区模板 | `.ottyrecipe` 单文件 TOML：布局 + cwd + 启动命令，便携路径变量，SHA-256 信任 + Ask Once | Workspace 模板：声明式定义 sessions 与启动命令，命令重放走信任模型 | 4 |

**明确不采纳**：桌面专属能力（Pin Window、Picture-in-Picture、Finder 集成、
内置 Web 浏览器）；Data Sync（服务端集中存储天然解决，备份 = 备份 `~/.kumokara/`）。

### 8.6 客户端形态演进（长期目标）

浏览器之外，Web 客户端可封装为跨平台 App。**这是长期目标，架构现在预埋，实现列入 Phase 4。**

| 形态 | 覆盖平台 | 连接模式 | 优先级 |
|------|---------|---------|--------|
| Browser | 全部 | 远程 server | ✅ 现在 |
| **Tauri App** | macOS / Linux / Windows | 远程 server，或内嵌 `kumokara-server` sidecar（= Local 模式，见 §2） | Phase 4 |
| **Tauri Mobile** | iOS / Android | 仅远程 server | Phase 4 |
| PWA | 移动端浏览器兜底 | 远程 server | Phase 4 |

**为什么 Tauri**：

- 与服务端同为 Rust 生态，桌面端可将 server 二进制以 sidecar 内嵌——
  一个 App 两种形态（远程客户端 / 本地自包含）；
- 系统 WebView 渲染，包体积极小（~10MB vs Electron ~150MB）；
- Tauri 2.x 同一代码库覆盖桌面 + iOS + Android；
- xterm.js 在 WKWebView / Android WebView 可用（WebGL addon 不兼容时降级 canvas）。

**架构纪律（现在就要遵守）**：

1. **单协议**：App 内同样走 WebSocket + REST 连 server，**不引入 Tauri IPC 作为
   第二传输通道**——所有客户端共享同一协议实现，server 对客户端形态无感知；
2. **移动端不内嵌 server**：iOS 后台执行限制注定移动端是远程客户端——24h Agent
   本就在服务端，手机只是观察窗与审批器，与产品定位天然契合；
3. **平台能力抽象层**：前端核心逻辑不直接调浏览器 API——token 存储
   （localStorage ↔ keychain）、通知（web push ↔ APNs/FCM）、终端渲染器
   （webgl ↔ canvas）均走接口，由壳层注入实现；
4. **协议类型共享**：`kumokara-protocol` 用 `ts-rs` 导出 TypeScript 类型，
   web 与未来 App 共用一份类型安全的协议契约。

---

## 9. Security

| 议题 | 决策 |
|------|------|
| 认证 | Phase 1 起强制：单 token（首次启动生成并打印，存 `config.yaml`）；WebSocket 首条消息 `auth { token}`（不放 URL）。Phase 3 扩展为 API Key + GitHub OAuth |
| 传输 | Local 模式绑定 `127.0.0.1` + 启动器自动注入一次性 token；Remote 模式 Phase 1-2 建议 SSH 端口转发，Phase 3 支持 HTTPS/TLS 后可绑定 `0.0.0.0` |
| 密钥 | workspace env 文件权限 0600；备份即备份 `~/.kumokara/`；系统 keyring / age 加密列入 Phase 4 |
| 敏感会话 | SSH target / sudo session 标记为敏感：默认禁用 IPC 三件套与审批按键转发，需显式授权（借自 Otty 的 IPC Allow Sensitive Sessions） |
| Agent 行为 | 不拦截（见 6.5）；安全边界由 Agent CLI 原生权限系统负责 |
| 通知 | Web push 使用 VAPID；通知内容默认只含状态不含输出正文（防泄密） |

---

## 10. Development Phases

### Phase 0 — Bootstrap (2周)
- [ ] Cargo workspace 搭建（按 4.3 crate 结构，全新实现）
- [ ] kumokara-engine：portable-pty 封装 + tmux control mode 包裹
- [ ] kumokara-server：axum WebSocket + `auth` 首消息认证
- [ ] Workspace CRUD + 持久化（sqlx / SQLite）
- [ ] Event bus + SQLite event log（两层模型，见 3.5）
- [ ] Web 原型：React + Vite + xterm.js，打通最小链路（create workspace → shell session → 终端 IO）
- [ ] 端到端集成测试（启动 server → 创建 workspace → shell session → 终端回显），作为 CI 门禁
- [ ] tmux 环境检测 + 降级友好提示（未安装时走纯 portable-pty，明确告知丧失恢复能力）

> 测试策略：Rust 后端以集成测试为主（PTY、tmux 交互、OSC 解析、事件持久化）；
> 前端用 Vitest + Testing Library 覆盖核心组件。E2E 测试链路在 Phase 0 即搭建，
> 后续每个 Phase 追加回归用例。

### Phase 1 — Agent Runtime (3-4周)
- [ ] AgentProvider trait + Claude Code provider
- [ ] Agent 进程启动/停止/监控/重启
- [ ] Agent session（provider CLI 运行于独立的 PTY session，记录 cli_session_id）
- [ ] Shell integration 注入（OSC 133/7 → 结构化事件，见 3.6）
- [ ] Session PTY 由 tmux 包裹（恢复模型基础，见第 7 章）
- [ ] 最小认证（单 token + `auth` 首消息，见第 9 章）
- [ ] 协议双通道（控制 text frame / 终端 IO binary frame）+ request_id
- [ ] Prompt queue 任务队列
- [ ] Web UI: 两级侧栏（Workspace + Session）、徽标、Composer

> 本 Phase 覆盖 ADE 功能清单（§8.5）中 Phase 1 项目：终端可观测性、崩溃恢复（基础）。
> **注意**：Phase 1 仅通过 tmux 保住 PTY 不丢；agent session 的 `cli_session_id` 已持久化但
> 自动 `--resume` 在 Phase 3 完整恢复模型中实现。Phase 1 末尾做一次性手动 resume 验证链路。

### Phase 2 — Orchestration (3-4周)
- [ ] Agent 状态感知（前台进程检测 + CLI hooks + 输出解析）
- [ ] 审批检测与通知（审批动作仍发生在 CLI 终端内，UI 按钮仅转发按键）
- [ ] 多 Workspace 并行
- [ ] Local 模式启动器（`kumokara` 单命令：拉起 server + 自动打开浏览器 + token 自动注入）
- [ ] 通知系统（web push — Phase 2 覆盖桌面浏览器，移动端推送 APNs/FCM 在 Phase 4 Tauri App 时补齐）
- [ ] Details 面板：Session 大纲（命令 + agent prompt 混合索引，基于 OSC 133）
- [ ] History 视图：transcript（.jsonl）渲染 + resume / fork
- [ ] 终端路径防误触（click 无动作，修饰键 + click 打开）
- [ ] Agent context 构建（从 event_log 注入）

> 本 Phase 覆盖 ADE 功能清单（§8.5）中 Phase 2 项目：大纲导航、Agent 会话历史、防误触。
> ↑ 依赖 Phase 1 的 Claude Code provider（需知道 transcript 路径）；Codex/OpenCode 的
> History 视图在 Phase 3 对应 provider 就绪后补齐。

### Phase 3 — Remote & Access (3-4周)
- [ ] SSH target connector
- [ ] 认证扩展（API Key + GitHub OAuth）
- [ ] HTTPS / TLS
- [ ] Codex / OpenCode provider
- [ ] Session reconnection with delta sync
- [ ] 恢复模型完整版（agent `--resume` + 进程白名单 + 熔断）
- [ ] Details 面板扩展：Info（进程/端口）/ Git diff / Files 文件树
- [ ] 跨 session 全局搜索 scrollback
- [ ] ⌘K 统一入口（跳转 + 动作，frecency 排序）
- [ ] Session IPC 三件套（send-text / run / exec）+ `$KUMOKARA_SESSION_ID` 注入
- [ ] 队列依赖原语（等 session X 空闲后执行）

> 本 Phase 覆盖 ADE 功能清单（§8.5）中 Phase 3 项目：路径交互、文件/Git、全局搜索、
> 双面板入口、崩溃恢复（完整）、会话 IPC、任务编排、安全边界。
> ↑ Session IPC 依赖恢复模型完整版（server 重启后 IPC 能否正常工作取决于 agent 上下文
> 是否已恢复）；队列依赖原语依赖 Session IPC 的 `run` / `exec` 能力。

### Phase 4 — Platform (未来)
- [ ] Kumokara Cloud 托管版
- [ ] 多用户协作
- [ ] 插件/Provider 市场
- [ ] CI/CD 集成
- [ ] 移动端 PWA 优化
- [ ] Tauri 跨平台 App（macOS/Linux/Windows/iOS/Android，见 8.6）
- [ ] Webhook / 外部通知集成（Slack, Discord, Email）
- [ ] Workspace 模板（声明式 TOML：sessions + 启动命令 + 便携路径 + 信任模型）
- [ ] 分屏（单 session 多 pane）
- [ ] 密钥加密存储（keyring / age）

> 本 Phase 覆盖 ADE 功能清单（§8.5）中 Phase 4 项目：工作区模板，以及客户端形态演进
> （§8.6）的全部目标。

---

## 11. Design Decisions Log

| 决策 | 选择 | 理由 |
|------|------|------|
| 核心抽象 | Workspace 为第一公民，而非 Shell Session | 持久化边界=产品价值：PTY 随 Server 重启消亡，能 24h 存活的是文件/env/Agent 配置/任务队列/事件历史；身份必须比进程长寿 |
| Session 模型 | Workspace 下 0..N 个 Session（shell / agent） | Agent 会话以项目目录为单位关联；Otty 的"按项目分组 tab"在 Kumokara 提升为数据模型 |
| 部署形态 | Local 与 Remote 同一套代码，零架构分叉 | Local = server 与 client 同机（Otty 等价物，但关 App 不杀 Agent）；单协议设计使 Local 成为免费的副产品与获客入口（见 §2 部署形态） |
| 客户端 | 仅 Web 技术栈（浏览器 + Tauri 封装） | 桌面/移动原生不是差异化点，Web 覆盖全平台；Tauri 见 8.6 |
| 远程协议 | 直接 SSH | 不自研传输协议，对接成熟生态 |
| Agent 模型 | 封装 CLI 而不是 API | Claude Code / Codex / OpenCode 都是 CLI 工具，直接管理其进程 |
| 安全模型 | 不拦截，只适配 | 安全边界由 Agent CLI 原生权限系统负责；外部拦截不可靠且提供虚假安全感（见 6.5） |
| Agent 集成 | 向 CLI 自身配置安装 hooks/plugin（Otty 模式） | 状态由 Agent 主动汇报，不解析终端输出猜状态（见 6.3） |
| 终端可观测性 | 注入 shell integration（OSC 133/7），服务端解析为结构化事件 | Otty 验证过的数据底座：命令边界/退出码/cwd 协议化，徽标、大纲、通知共用同一数据源（见 3.6） |
| 事件存储 | 两层分离：结构化事件（SQLite 永久）+ 原始输出（环形缓冲易逝） | 原始输出量大易逝只用于回放，结构化事件量小永久用于查询——避免 event_log 膨胀（见 3.5） |
| 崩溃恢复 | tmux 包裹 PTY + agent `--resume` + 进程白名单 + 熔断 | Server 重启不丢终端与 Agent 上下文——24h 在线承诺的技术兑现（见第 7 章） |
| 协议编码 | 控制消息 text frame + 终端 IO binary frame + request_id | 终端 IO 高频大块，不宜 JSON 编码；request_id 支撑并发请求关联 |
| 认证时机 | Phase 1 即强制最小 token 认证 | 暴露 Shell + 存 API Key 的服务，无认证不可发布 |
| 会话持久层 | tmux（control mode） | 最成熟无处不在，Server 重启重连即恢复，用户可手动 attach 自救；不引入 dtach/shpool（见 4.1） |
| 跨平台客户端 | Tauri 2.x（长期，Phase 4） | Rust 同源 + WebView 复用 web 前端 + 可内嵌 server sidecar；纪律：单 WebSocket 协议、移动端不内嵌 server、平台能力抽象（见 8.6） |
| 数据存储 | SQLite (sqlx) | 单机部署，零依赖，足够 |
| 前端框架 | React + Vite + xterm.js | 生态最大；xterm.js 是 Web 终端事实标准（见 4.1） |
| 会话重连 | screen dump + seq 增量回放 | 重连成本与历史长度无关（见 5.3） |
| 容器化 | 可选，非必需 | 初期用文件系统目录隔离，后期可选 Docker sandbox |
| 语言 | Rust (服务端) + TypeScript (前端) | 长驻 daemon 的稳定性 + 单二进制分发；关键依赖：tokio / axum / portable-pty / sqlx / russh（见 4.1）。旧 kara 实现全部丢弃，从头开始 |
| 事件总线 | tokio broadcast channel（per workspace） | 同一进程内，无需外部 broker；慢消费者被踢出后从 event_log 补齐（而非阻塞上游）；广播容量通过 `lagged` 检测——若频繁踢出则增大 buffer 或降级消费者

---

## 12. Competitor Summary

| | Otty | Cursor | Codespaces | Replit | Kumokara |
|---|---|---|---|---|---|
| 形态 | 桌面 App | IDE | Web+IDE | Web IDE | Web 服务 |
| 部署门槛 | 安装即用 | 安装即用 | 零配置 | 零配置 | 自部署（或 Local 一键） |
| 定价 | $10/月 | $20/月 | 按量计费 | $25/月 | 免费开源 |
| Agent | 支持 | Copilot | Copilot | Agent | 核心能力 |
| 24h 在线 | ❌ | ❌ | ⚠️ 空闲自动停止 | ⚠️ 需 Always-On 档 | ✅ |
| 全平台 | ❌ | ❌ | ✅ | ✅ | ✅ |
| 自部署 | ❌ | ❌ | ❌ | ❌ | ✅ |
| 数据位置 | 本地 | 混合 | GitHub | Replit 云 | 你的服务器 |

相邻工具（非完整 ADE）：Vibe Kanban、Conductor、Crystal、Omnara 等提供多 Agent
编排与移动审批，但均不具备"自托管 + Workspace 持久化 + 全设备 Web 终端"的组合。
