# Kumokara 产品与实现设计

> Status: Working draft

## 1. 产品定义

Kumokara 是一个自部署、Agent-neutral 的持久终端。它解决的不是“创建和管理
Workspace”，而是让 Shell 与其中运行的 Agent 独立于浏览器持续存在，并可从任意
设备重新接入。

用户界面只有两个主要区域：

```text
┌──────────────────┬──────────────────────────────────────┐
│ Session 列表      │ 当前 Session 的 Shell                 │
│                  │                                      │
│ ›_ kumokara      │ $ cd ~/code/project                  │
│ ◆ codex          │ $ codex                              │
│ ◆ claude_code    │                                      │
│                  │                                      │
│       [+]        │                                      │
└──────────────────┴──────────────────────────────────────┘
```

不提供“创建 Workspace → 再创建 Session”的前置流程。用户创建 Shell，`cd` 到工作
目录并启动任意 Agent 即可。

## 2. 核心概念

### 2.1 Session

Session 是唯一对用户可见的一等对象：

```text
Session
├── id
├── PTY process
├── cwd
├── title
├── agent?          # 自动发现后填充
├── output history  # 有界、按 seq 保存、非破坏性读取
└── attachments     # 0..N 个浏览器连接
```

Session 由 Server Runtime 持有。浏览器只是 attachment，关闭页面不会释放 PTY。

### 2.2 Project Context（Session 属性）

项目上下文不是独立资源，也没有生命周期 API。它就是当前 Shell 或 Agent 进程的
canonical cwd：

```text
project_context = canonical(cwd)
```

同一目录下的多个 Session 自然具有相同项目上下文；切换 cwd 后，展示信息随实际
进程状态更新。只有未来出现确切的持久化需求时，才增加 cwd 索引，不预先引入
Context Manager 或配置对象。

### 2.3 Agent Session

Agent Session 不是单独创建的类型，而是普通 Shell Session 的运行时状态：

```text
Shell Session
    │ 用户运行 claude / codex / opencode / ...
    ▼
Agent process discovered
    │
    ├── provider
    ├── agent cwd → project context
    └── optional adapter metadata
```

Agent 退出后 Session 仍然存在，并恢复为普通 Shell 状态。

## 3. Agent-neutral 兼容层级

兼容能力分三层，后两层不得成为前一层的使用前提：

1. **Generic PTY**：任何交互式 CLI 都能运行、输入、输出、resize 和重连。
2. **Process discovery**：识别常见 Agent 进程及其 cwd，更新 Session 展示。
3. **Provider adapter / hooks**：提供任务状态、审批、resume、模型等增强信息。

当前实现覆盖前两层的基础能力。已知进程包括 Claude Code、Codex、OpenCode、Kimi
Code 和 Mimo Code；未知工具始终退化为 Generic PTY，而不是拒绝运行。

## 4. Runtime 架构

```text
Axum WebSocket
          │
          ▼
SessionRegistry  ← 唯一 Session runtime source of truth
          │
          ├── SessionEntry
          │     ├── SessionInfo
          │     ├── PtySession       # 持有 child cleanup 生命周期
          │     ├── OutputHistory    # VecDeque<seq, bytes>
          │     └── broadcast::Sender<OutputChunk>
          │
          └── Process discovery
                ├── shell/descendant process tree
                ├── known provider detection
                └── cwd discovery
```

禁止在 HTTP/WebSocket handler 或其他模块中维护第二份活动 Session 状态。持久元数据
可以从 Runtime 做快照，但不能反向成为并行运行时。

代码边界与产品概念一一对应：

```text
protocol   仅包含 Auth / Session / Terminal wire types
engine     仅负责 PTY 生命周期和 tmux 能力探测
auth       仅负责 token
server     SessionRegistry + output history + process discovery + transport
cli        本地与 daemon 启动入口
```

没有真实运行职责的 Agent、Event、Workspace、SSH、Shell-integration 占位 crate 不保留；
等对应能力进入可运行路径时再按职责增加模块。

## 5. Attach 与输出一致性

输出数据按 chunk 分配单调递增的 `seq`。OutputHistory 只淘汰最旧 chunk，读取不消费
历史，因此多个客户端可以独立重连。

Attach 顺序：

1. 先订阅 live broadcast；
2. 再读取 `last_seq` 之后的历史快照；
3. 记录快照后的 `live_from_seq`；
4. 回放快照；
5. 转发 `seq >= live_from_seq` 的 live chunk。

这样同时避免“先回放后订阅”造成的数据丢失，以及“先订阅后回放”造成的重复输出。
如果 `last_seq` 已被淘汰，返回 gap notification，客户端可提示历史不完整。

当前 history 保存原始终端字节。后续如需严格 screen dump，应引入服务端 VT 状态机，
不能把原始日志文本伪装成屏幕快照。

## 6. 生命周期

### Server 启动

- Registry 为空时自动创建一个 Shell Session；
- 初始 cwd 为 Server 启动目录，默认尺寸为 100 × 30；
- 浏览器认证后通过 `session_list` 获取并自动选中它，不在前端重复创建。

### 浏览器断开

- attachment 结束；
- PTY 和输出历史继续由 SessionRegistry 持有；
- 重连后重新 list + attach。

### 用户关闭 Session

- 从 Registry 移除 SessionEntry；
- Drop PtySession；
- kill/wait child；
- live channel 关闭。

### Server 重启

当前 portable-pty Session 会终止。完整恢复需要 tmux control-mode backend：启动时枚举
带 Kumokara metadata 的 tmux session，重建 Registry，再恢复输出订阅。未完成前不得
宣称支持 Server crash recovery；当前重启后只会创建一个新的默认 Session。

## 7. cwd 与项目上下文绑定

新 Session 默认从 Server 启动目录开始，也可由受信客户端传入初始 cwd。Server 对
路径做 canonicalize 并确认其为目录。

macOS/Linux 上定期检查 PTY shell 及其子进程：

- 无 Agent 时使用 Shell cwd；
- 发现 Agent 时使用 Agent process cwd；
- Session 列表只展示 cwd 和 Agent 状态；
- 项目上下文等于 canonical cwd。

Process discovery 是 best-effort。Provider hooks 将用于更精确的 session id、任务状态
和审批事件，但不能改变用户“在 Shell 里直接启动 Agent”的路径。

## 8. 协议

主要控制消息：

```text
session_create  { request_id, cwd?, cols, rows }
session_list    { request_id }
session_attach  { request_id, session_id, last_seq? }
session_destroy { request_id, session_id }
terminal_input  { session_id, data_base64 }
terminal_resize { session_id, cols, rows }
```

`terminal_output` 同样使用 `data_base64`，避免 PTY 字节跨 chunk 拆分 UTF-8 字符时发生
有损转换。二进制 input frame 仍用于不需要 JSON 的客户端。

协议不包含 `workspace_id`、Workspace 消息或 Workspace REST API。Agent 也不通过协议
单独启动；用户始终在普通 Shell 中运行 Agent。

Server 默认使用无鉴权开发模式：连接建立后主动发送 `auth_ok`。使用
`--require-token` 启动时，首条客户端消息必须为 `auth`，验证通过后才允许控制 Session。

## 9. 安全边界

- 默认无鉴权模式只用于本机开发；监听非受信网络时必须启用 `--require-token`；
- token 模式下 WebSocket 首条客户端消息必须通过验证；
- health check 和静态前端不涉及控制能力，Session 控制只通过已认证的 WebSocket；
- 静态文件使用 `ServeDir`，不得手工拼接 URL path；
- Remote 模式应放在 TLS/reverse proxy 后；
- cwd 和进程访问默认采用单用户自部署信任模型；多用户部署前必须增加 OS 级隔离。

## 10. 下一阶段

按以下顺序继续，避免再次铺设未接通的占位模块：

1. tmux backend 与 Server restart recovery；
2. 按 canonical cwd 持久化确有需要的 Session 元数据；
3. Claude Code、Codex、OpenCode hooks/adapters；
4. 服务端 VT screen snapshot；
5. SSH target；
6. OAuth、多用户和资源隔离。

每阶段必须包含真实 PTY 生命周期测试：create、input/output、browser detach、reattach、
resize、destroy，以及对应的鉴权和恢复测试。
