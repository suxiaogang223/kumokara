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
    └── AgentAdapter metadata / OSC 26 live state
```

Agent 退出后 Session 仍然存在，并恢复为普通 Shell 状态。

## 3. Agent-neutral 兼容层级

兼容能力分三层，后两层不得成为前一层的使用前提：

1. **Generic PTY**：任何交互式 CLI 都能运行、输入、输出、resize 和重连。
2. **Process discovery**：识别常见 Agent 进程及其 cwd，更新 Session 展示。
3. **Provider adapter / hooks**：通过可注册 `AgentAdapter` 和 OSC 26 提供标题、任务状态、
   审批、resume、模型等增强信息。

当前实现覆盖三层的公共入口。内置 adapter 包括 Claude Code、Codex、OpenCode、Cursor、
Kimi Code、Mimo Code、Pi 和 omp；未知工具始终退化为 Generic PTY。外部 provider 可以
实现 `kumokara_agent::AgentAdapter` 并注册到 `AgentAdapterRegistry`，无需修改 Session、
WebSocket 或 UI 代码。Agent hook/plugin 可发出 OSC 26 metadata，由统一协议入口更新
`SessionInfo.agent`。

## 4. Runtime 架构

```text
Axum WebSocket
          │
          ▼
SessionRegistry  ← 唯一 Session runtime source of truth
          │
          ├── SessionEntry
          │     ├── SessionInfo
          │     ├── PtySession       # PTY、子进程与 I/O 生命周期
          │     ├── OutputHistory    # VecDeque<seq, bytes>
          │     └── broadcast::Sender<OutputChunk>
          │
          └── Process discovery
                ├── shell/descendant process tree
                ├── AgentAdapterRegistry
                │     ├── built-in adapters
                │     └── registered provider plugins
                └── cwd discovery
```

禁止在 HTTP/WebSocket handler 或其他模块中维护第二份活动 Session 状态。持久元数据
可以从 Runtime 做快照，但不能反向成为并行运行时。

代码边界与产品概念一一对应：

```text
protocol   仅包含 Auth / Session / Terminal wire types
agent      AgentAdapter trait、registry 与内置 provider plugins
engine     单一具体 PtySession、子进程与 I/O 生命周期
auth       仅负责 token
server     SessionRegistry + output history + process discovery + transport
cli        本地与 daemon 启动入口
```

没有真实运行职责的 Agent、Event、Workspace、SSH、Shell-integration 占位 crate 不保留；
等对应能力进入可运行路径时再按职责增加模块。

## 5. 外观设置

Appearance 是浏览器本地偏好，不属于 Session、Project Context 或服务端协议：

- `Auto` 跟随系统 `prefers-color-scheme`，同时保存独立的 Light/Dark 主题选择；
- 一个主题同时定义应用 UI palette 和 xterm ANSI palette，切换时保持外壳与终端一致；
- 字体族和字号显式由用户设置，并动态应用到所有终端；
- 默认使用可移植的系统等宽字体栈，不探测、不下载，也不优先选择 Nerd Font；
- Oh My Posh 等提示符需要图标时，由用户安装相应 Nerd Font 并填写准确的 font-family；
- 当前偏好保存在浏览器 `localStorage`，不引入服务端用户设置模型。

内置主题只保存颜色数据，Settings 面板根据同一份数据生成预览，避免维护一套与实际
终端主题不一致的展示配置。

## 6. Attach 与输出一致性

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

## 7. 生命周期

### Server 启动

- Registry 为空时自动创建一个 Shell Session；
- 初始 cwd 为 Server 启动目录，默认尺寸为 100 × 30；
- 浏览器认证后通过 `session_list` 获取并自动选中它，不在前端重复创建。

### 浏览器断开

- attachment 结束；
- PTY 和输出历史继续由 SessionRegistry 持有；
- 重连后重新 list + attach。

### 多浏览器尺寸

- 每个 attachment 在本地独立执行 xterm fit；只有当前获得焦点的可见页面会在布局稳定后发送
  `active=true` 的 resize，让全屏 TUI 匹配这个浏览器窗口；
- 后台页面的 ResizeObserver 只更新本地 xterm，不修改共享 PTY；
- 当前窗口发生真实输入时，客户端仍把它的 `cols/rows` 与 input 一起发送；
- Server 把 resize 和 input 放入同一个有序 PTY command queue，保证尺寸先于对应输入生效；
- 未携带 `active=true` 的旧 `terminal_resize` 仍作为 viewport-local hint 忽略；
- 单个 PTY 进程物理上仍只有一套活动尺寸，因此发生输入的 attachment 是当次交互的临时
  controller；所有 attachment 只同步同一份原始输出字节，不同步彼此的 UI 宽高。

### 用户关闭 Session

- 从 Registry 移除 SessionEntry；
- Drop PtySession 终止并回收 Shell 子进程；
- live channel 关闭。

### Server 重启

Server 直接拥有 PTY。服务退出时这些 PTY 和对应的 Kumokara Session 一同结束，
重启后只创建新的默认 Shell，不重建旧终端屏幕。Claude Code、Codex、OpenCode 等 Agent 的
长期上下文由它们各自写入本地的 session 数据保存，用户在新 Shell 中通过 Agent 自身的
`resume` 能力继续工作。

这个边界刻意区分两种持久化：浏览器断开不会影响 Server 中的 Session；Server 重启则是
明确的运行时边界。当前不引入 tmux、后台守护进程或第二套 Session metadata 来跨越它。
`kumokara-engine` 因此只公开具体的 `PtySession`，不保留 backend trait、backend enum 或
仅用于转发到 `portable-pty` 的实现模块。

## 8. cwd 与项目上下文绑定

新 Session 默认从 Server 启动目录开始，也可由受信客户端传入初始 cwd。Server 对
路径做 canonicalize 并确认其为目录。

macOS/Linux 上定期检查 PTY shell 及其子进程：

- 无 Agent 时使用 Shell cwd；
- 发现 Agent 时使用 Agent process cwd；
- Session 列表只展示 cwd 和 Agent 状态；
- 项目上下文等于 canonical cwd。

Process discovery 是 best-effort。Provider hooks 将用于更精确的 session id、任务状态
和审批事件，但不能改变用户“在 Shell 里直接启动 Agent”的路径。

## 9. 协议

主要控制消息：

```text
session_create  { request_id, cwd?, cols, rows }
session_list    { request_id }
session_attach  { request_id, session_id, last_seq? }
session_destroy { request_id, session_id }
terminal_resize { session_id, cols, rows, active } # only active viewport controls PTY
terminal_title  { session_id, title }       # xterm OSC 0/2
agent_update    { session_id, code_agent, session_title?, status?, detail?, mode?, task_progress? }
```

PTY payload 双向统一使用 binary WebSocket frame，不保留 JSON/base64 I/O 路径。前 16 bytes
为 Session UUID，随后 8 bytes 在 server-to-client output 中为 big-endian sequence，在
client-to-server input 中保留为零，最后是 raw PTY bytes。输入前如需改变终端尺寸，客户端先
发送 `terminal_resize`；WebSocket 消息顺序保证 resize 先于紧随其后的 binary input 生效。

Web 前端以 animation frame 合并连续 output，并且只有在 xterm 完成上一批解析后才提交下一批；
WebGL2 是默认 renderer。初始化失败时明确显示 compatibility renderer 状态；context lost 只重试
一次，失败后保持 compatibility renderer，不引入多轮恢复状态机。

协议不包含 `workspace_id`、Workspace 消息或 Workspace REST API。Agent 也不通过协议
单独启动；用户始终在普通 Shell 中运行 Agent。

Server 默认使用无鉴权开发模式：连接建立后主动发送 `auth_ok`。使用
`--require-token` 启动时，首条客户端消息必须为 `auth`，验证通过后才允许控制 Session。

## 10. 安全边界

- 默认无鉴权模式只用于本机开发；监听非受信网络时必须启用 `--require-token`；
- token 模式下 WebSocket 首条客户端消息必须通过验证；
- health check 和静态前端不涉及控制能力，Session 控制只通过已认证的 WebSocket；
- 静态文件使用 `ServeDir`，不得手工拼接 URL path；
- Remote 模式应放在 TLS/reverse proxy 后；
- cwd 和进程访问默认采用单用户自部署信任模型；多用户部署前必须增加 OS 级隔离。

## 11. 下一阶段

按以下顺序继续，避免再次铺设未接通的占位模块：

1. Claude Code、Codex、OpenCode hooks/adapters 与 resume 入口；
2. 面向 AI/自动化的 Kumokara Self CLI 与稳定的结构化输出；
3. 集中式快捷键路由、Natural Text Editing（包含 Hungry Delete）与可配置 keybindings；
4. 浏览器本地 Split pane 布局与多 Session 同屏；
5. 服务端精确 VT screen model 与更完整的进程内 scrollback；
6. SSH target；
7. OAuth、多用户和资源隔离。

每阶段必须包含真实 PTY 生命周期测试：create、input/output、browser detach、reattach、
resize、destroy，以及对应的鉴权和服务重启边界测试。

### Kumokara Self CLI / AI 控制接口计划

Self CLI 让运行在本机或 Kumokara Session 内的 AI 通过 `kumokara` 命令管理 Session。
它是现有 WebSocket control protocol 的命令行客户端，不直接访问 `SessionRegistry`，也不增加
第二套 HTTP Session API、状态缓存或后台 daemon。

第一阶段命令面：

```text
kumokara session list [--json]
kumokara session inspect <session-id> [--json]
kumokara session create [--cwd <path>] [--cols <n>] [--rows <n>] [--json]
kumokara session close <session-id> --confirm <session-id> [--json]
kumokara session send <session-id> (--text <text> | --stdin) [--enter] [--json]
kumokara session output <session-id> [--since <seq>] [--follow] [--raw | --jsonl]
kumokara session resize <session-id> --cols <n> --rows <n> [--active]
kumokara session current [--json]
kumokara capabilities [--json]
```

- `list/create/close/send/output/resize` 分别复用现有 `session_list`、`session_create`、
  `session_destroy`、binary input frame、`session_attach/detach`、`terminal_resize`；
- `inspect` 第一阶段通过 `session_list` 在客户端过滤，避免为了单条查询扩展服务端协议；
- `current` 从 PTY 已有的 `KUMOKARA_SESSION_ID` 读取当前 Session，并通过服务端 list 验证它
  仍然存在；不在 Kumokara PTY 中运行时返回明确的 non-zero exit code；
- `send` 发送准确字节，不做 shell quoting、不自动添加换行；只有显式 `--enter` 才附加 `\r`。
  它不能命名为 `exec`，因为现有协议无法证明 shell 当前位于 prompt、命令何时完成或退出码；
- `output` 的 `--raw` 原样写终端字节，`--jsonl` 逐 chunk 输出 `session_id/seq/data_base64`。
  在服务端 VT screen model 完成前，不得剥离 ANSI 后伪装成可靠 screen snapshot；
- `resize` 默认只发送 viewport-local hint，不争抢浏览器 controller；AI 明确传入 `--active`
  时才改变 PTY 活动尺寸，`send` 也只有显式提供 size 时才携带 `cols/rows`；
- `capabilities` 输出 server/protocol version、支持的 command 和 feature flags，让 AI 先探测
  能力再调用，避免依赖错误文本或 CLI 帮助文案判断版本。

连接与鉴权：

- endpoint 解析顺序为 `--server` → `KUMOKARA_SERVER` → 本机默认地址；HTTP(S) 地址统一转换为
  WS(S) control endpoint。自定义 bind/port 启动时应把可连接地址注入新 Session 的
  `KUMOKARA_SERVER`；
- token 解析顺序为标准输入/受限权限配置 → `KUMOKARA_TOKEN` → `--token`。不得把 token
  自动注入所有 PTY，也不得在日志、JSON error 或进程参数回显中泄露；
- CLI 使用与浏览器完全相同的首帧鉴权和 TLS 边界。远程明文 WebSocket 默认拒绝，除非用户
  显式允许不安全连接；
- 每次 CLI 调用建立独立 WebSocket，使用 `request_id` 关联响应并在退出前 detach。独立连接的
  output attachment 不会覆盖浏览器连接的 attachment；`--follow` 必须正确处理 history gap、
  Ctrl-C 和服务端断连。

面向 AI 的输出契约：

- `--json` stdout 只输出一个稳定、带 `schema_version` 的 JSON document；streaming 使用
  JSON Lines。日志、进度和人类提示只写 stderr；
- 成功、参数错误、鉴权失败、Session 不存在、冲突、超时和连接失败使用稳定且文档化的
  exit code；JSON error 同时提供机器可读 `code`、`message`、`request_id`；
- Session ID 必须完整输出和显式传入，不允许 AI 依赖模糊标题执行破坏操作；`close` 需要
  `--confirm <session-id>`，批量关闭另设命令并要求更强确认，不能复用模糊匹配；
- 所有 mutation 支持客户端生成的 request id，并为未来幂等 create/destroy 留出协议字段；
  第一阶段不承诺跨进程重试幂等，失败后 AI 必须先 list/inspect 再决定是否重试。

实现保持简单：先在 `kumokara-cli` 内增加 `client` module 和 `session` subcommands，共享
`kumokara-protocol` wire types；只有出现第二个 Rust control client 后才提取
`kumokara-client` crate。不得让 CLI import server 内部 `SessionRegistry` 或复制协议 struct。

第二阶段在基础协议具备可靠完成标记后再增加：等待 Agent status、按状态筛选、超时等待、
可靠 screen snapshot、创建专用 Session 后运行单条命令并返回 exit status。命令完成应依赖
明确的 shell integration/协议事件，不能通过提示符文本或输出静默时间猜测。

测试至少覆盖：无鉴权与 token 模式、JSON/JSONL schema、原始非 UTF-8 chunk、history gap、
`--follow` 取消、send 不隐式回车、close 确认、current Session 解析、浏览器与 CLI 同时 attach、
非 active resize 不改变共享 PTY，以及远程 TLS/不安全连接策略。

### 快捷键与键盘路由计划

快捷键语义遵循常见原生终端和 macOS 文本编辑习惯，但不能直接复制原生应用的默认按键。
普通浏览器会优先处理 `⌘T`、`⌘W`、`⌘N`、`⌘1`–`⌘9`、页面缩放等组合键；页面收不到的
按键不能被声明为可用快捷键。

实现时先分离 action 与 chord：

```text
ShortcutAction
├── id                 # session.new / pane.split_right / text.delete_word_left / ...
├── scope              # app / pane / terminal
├── default bindings   # web 与未来 desktop profile 分开
└── behavior           # UI action 或发送给 PTY 的字节序列
```

- 只维护一份 action registry 和一个键盘 dispatcher，不在各 React 组件散落全局
  `keydown` listener；终端侧使用 xterm 官方 `attachCustomKeyEventHandler` 决定拦截或透传；
- 路由顺序为：原生 `input`/`select`/`dialog` 编辑行为 → 精确匹配的 Kumokara action →
  原样交给 xterm/PTY。不得按宽泛的 `metaKey`、`ctrlKey` 条件吞掉 Agent TUI 输入；
- 提供 `web` 默认映射，并为未来 desktop/PWA profile 保留独立映射。被浏览器保留的组合键
  在 web profile 中保持未绑定，通过按钮或未来 Command Palette 访问；
- 用户自定义 keybindings 属于浏览器本地偏好，保存到 `localStorage`，支持按 action/chord
  搜索、冲突检测、解绑和恢复默认；第一阶段先完成 registry 与固定默认映射，再增加设置 UI；
- 文本编辑动作只发送 shell/readline 序列，不解析 xterm 屏幕，也不在前端维护第二份命令行：
  `⌘←/⌘→` → `Ctrl-A/Ctrl-E`，`⌥←/⌥→` → `Esc-b/Esc-f`，Hungry Delete
  `⌥⌫/⌥⌦` → `Ctrl-W/Esc-d`，删除到行首/行尾 `⌘⌫/⌘⌦` → `Ctrl-U/Ctrl-K`；
- Natural Text Editing 默认只在 normal buffer 生效；alternate-screen Agent/TUI 优先透传。
  用户可以显式重绑，但不能依靠猜测进程名称决定键盘行为；
- 剪贴板优先使用浏览器 Clipboard API 和 xterm `paste()`，查找使用官方
  `@xterm/addon-search`，滚动和清屏调用 xterm API，不自建文本模型或 DOM 滚动实现。

第一阶段 action 范围：

- Session：新建、关闭确认、上一个/下一个、按序号跳转、切换 Tabs panel；
- Pane：创建 Split、关闭/聚焦/缩放 Pane、移动 divider、equalize；
- Terminal：复制、粘贴、查找、滚动到顶部/底部、字体增减/恢复、清屏；
- Text Editing：行首/行尾、按词移动、Hungry Delete、删除到行首/行尾；
- General：打开 Settings；Command Palette 等有真实 action 数量后再实现，不先铺空壳。

测试必须覆盖 chord 标准化、冲突检测、浏览器保留键不误报、原生表单控件不被拦截、
normal/alternate buffer 路由、PTY 收到的精确字节，以及 macOS/Windows/Linux 键盘事件差异。

### Split pane 实现计划

Split 保持 Kumokara 的 Session-first 模型：Session 仍是服务端 PTY 资源，Pane 只是当前
浏览器中查看和操作 Session 的本地视图，不新增 Workspace、Pane API 或服务端布局状态。

浏览器保存一棵最小布局树：

```text
PaneLayout = Leaf { pane_id, session_id }
           | Split { axis, ratio, first, second }
```

- `axis` 只取 horizontal/vertical，`ratio` 限制在安全范围；关闭一个 Pane 时折叠其父节点，
  不留下单子节点 Split；
- 布局和 focused pane 使用 `sessionStorage`，因此刷新当前页面可恢复，但不同浏览器窗口互不
  同步；服务重启后引用已消失 Session 的 Leaf 会被丢弃并回到可用默认布局；
- 新建 Split 默认创建一个继承当前 Session cwd 的新 Shell Session，再把它绑定到新 Leaf；
  关闭 Pane 只移除浏览器视图，显式 Close Session 才销毁 PTY，避免误杀长时间运行的 Agent；
- v1 同一页面的一棵布局树中，一个 Session 最多出现一次。当前 WebSocket attachment 以
  `session_id` 为 key，同一连接重复 attach/detach 会互相覆盖；若未来需要镜像同一 Session，
  再通过 `attachment_id` 或客户端引用计数扩展协议，不在前端绕过；
- 每个 Leaf 拥有独立 xterm + `ResizeObserver`。Pane 获得焦点后才接收快捷键和输入；标题栏、
  查找、复制、粘贴等 pane-scoped action 始终路由到 focused pane；
- Resize 继续遵守现有多浏览器 controller 规则：本地所有 Pane 独立 fit，只有可见且有控制权
  的页面向对应 Session 发送 settled grid，真实输入仍携带该 Pane 的 `cols/rows`；
- 浏览器没有原生 split-view 控件。布局使用 CSS Grid/Flex，divider 只实现必要的 Pointer
  Capture；divider 同时使用 `role="separator"`、`aria-orientation`、`aria-valuenow` 并支持
  方向键调整。不得引入自绘窗口系统或第二套像素布局引擎；
- 第一阶段只支持二叉 Split、拖动 resize、键盘 resize、方向/循环聚焦、equalize 和单 Pane
  zoom。Pane 拖拽重排、任意边缘 drop、Recipe/layout 分享等延后到基本生命周期稳定之后。

目标 action 包括 Split right/left/down/up、focus next/previous、按方向 focus、按方向移动
divider、equalize、zoom/unzoom。默认 chord 必须通过前述 web reserved-key 审计；`⌘D`、
`⌘W` 等原生应用常用组合键不能假设普通浏览器一定会交给页面。

Split 的端到端测试至少覆盖：创建两个/嵌套 Pane、不同 Session 同时输出、焦点输入隔离、
独立 resize、关闭后树折叠、刷新恢复、Session 被其他窗口删除后的布局清理，以及两个浏览器
窗口拥有不同 Split/尺寸时不互相同步 UI 状态。

### Agent Adapter 扩展边界

`kumokara-agent` 已提供 `AgentAdapter` trait 与有序 registry。adapter 当前负责 process
detection、稳定 provider id、显示名、通用 Unicode 图标和可选 title hint；hook/plugin
通过通用 OSC 26 入口提供运行时 metadata。新增 provider 不得在 `SessionRegistry`、
`process_discovery` 或 React 组件中增加 provider-specific 分支。

后续每个深度适配仍需明确其全部实际行为，包括：

- 进程与 CLI session ID 发现；
- running、waiting、approval 等状态表达；
- resume/reconnect 行为；
- hooks、事件和审批交互；
- 模型、任务等可选元数据；
- Agent 退出后退化回普通 Shell 的生命周期。

只有上述行为经过真实使用并形成稳定规范后，才提取公共 `AgentAdapter` 接口、能力声明
和注册机制。其他 Agent 在此之前继续通过 Generic PTY 工作，避免根据未知需求提前设计
抽象层。
