# tuack-ng-rpc 协议接口文档 v0.1

`tuack-ng-rpc` 是 tuack-ng 的竞赛工具后端协议（JSON-RPC 2.0），面向 IDE / GUI / 脚本等任意客户端工具。

- 协议定义独立领域类型，不暴露 tuack 内部 Rust 类型。
- 单一后端（tuack-ng），复用 `tuack-lib` / `tuack-utils` / `tuack-config` 的原子能力。
- 业务流程面向「资源 + 事件」；RPC 只暴露**稳定的后端原子能力**，评测策略（测哪些点、顺序、何时停止）由调用者编排。

## 1. 传输层

- 基于 **JSON-RPC 2.0**，**NDJSON** 传输（每行一个 JSON 对象）。
- **硬性规则：单条 JSON message 不得跨行。**
- **所有 Request / Response / Notification 必须携带 `"jsonrpc": "2.0"`。**
- 客户端写请求到服务端 stdin；服务端将**响应**和**事件**逐行写 stdout；stderr 仅供服务端自诊，客户端不应解析。
- 请求必须携带 `id`（数字或字符串）；无 `id` 的消息视为**通知**，服务端不响应。
- **stdin EOF 等价于 `exit` notification**：服务端停止接受新请求并终止进程（客户端崩溃/管道断开时不残留后台进程）。
- **EOF/exit 行为注意**：EOF 或 `exit` 会立即关闭服务端异步运行时，**取消所有在途任务**（`run/create` 的编译准备、正在执行的 `run/judge`、`ren/run` 渲染等）。被取消的底层子进程操作可能上报 `"background task failed"` 等异常错误事件（经 `run/output` / `run/finished`），且事件流可能不完整。**客户端应在目标任务结束后（收到 `run/ready`、`run/finished`、`ren/finished` 等终态事件）再发送 `exit` 或关闭 stdin**；任务进行中退出，应以 `run/get` / `ren/get` 重新确认状态而非依赖事件流。

## 2. 生命周期状态机

```text
Created
   |
   | initialize
   v
Initialized
   |
   | shutdown
   v
Shutdown
   |
   | exit（或 stdin EOF）
   v
Process exit
```

- `initialize` 之前调用任何其他 method -> `-32600`（非法请求）
- 重复 `initialize` -> `-32600`
- `shutdown` 之后调用业务方法 -> `-32600`
- `shutdown` 响应后等待 `exit` 通知（或 EOF），然后进程退出

## 3. 会话（Session）语义

**Session 是 backend 对某个 workspace 的运行时上下文**，而非简单「打开句柄」：

```text
workspace（磁盘上的工程目录）
    |
    +-- session
         |-- loaded config（含 session-global revision）
         |-- languages / assets
         |-- running tasks（run）
         +-- 状态缓存
```

- **每次 `workspace/open` 都创建独立 session**；同一目录可被多次打开，互不共享状态。
- **Run 从属 Session。** `workspace/close` 时：
  1. 取消该 session 下所有未完成 run（发 `run/finished(state:cancelled)`）
  2. 这些 run 随即从服务端移除，此后对其 `run/get`/`run/judge`/`run/score` 等返回 `-32006`
  3. 销毁 session
- 客户端「关闭 UI 但后台继续跑」不属于本协议职责；如需该能力应另建概念。

## 4. 基础类型

| 类型                | 说明                                                                                            |
| ------------------- | ----------------------------------------------------------------------------------------------- |
| `sessionId: string` | `workspace/open` 创建，标识一个工作区运行时上下文                                               |
| `runId: string`     | `run/create` 创建，标识一个评测会话                                                             |
| `taskId: string`    | `ren/run` 创建，标识一次渲染任务                                                                |
| `scope: string`     | 配置作用域：`"contest"` / `"<day>"` / `"<day>/<problem>"`。JSON 层为字符串，服务端以强类型解析  |
| `field: string`     | 配置字段路径，JSON Pointer（RFC 6901，`~0`=`~`、`~1`=`/`）。`""` 指向整个 FileView 文档         |
| `uri: string`       | 文件统一资源标识，`file://` 形式                                                                |
| `problemType`       | `"program"` \| `"output"` \| `"interactive"`                                                    |
| `testStatus`        | `"AC"` \| `"WA"` \| `"RE"` \| `"TLE"` \| `"MLE"` \| `"UKE"` \| `"FE"` \| `"PC"`（单点裁决结果） |
| `runState`          | `"preparing"` \| `"ready"` \| `"cancelled"` \| `"error"` \| `"closed"`（run 生命周期状态）      |
| `renState`          | `"running"` \| `"finished"` \| `"cancelled"` \| `"error"`（渲染任务状态）                       |
| `channel`           | `"stdout"` \| `"stderr"` \| `"compiler"` \| `"judge"` \| `"renderer"` \| `"system"`             |
| `seq: number`       | 事件序号，进程级全局单调递增（见 §8）                                                           |

**路径语义（严格区分三种）：**

- `uri`：`file://` 形式，用于标识工作区/绝对位置
- 返回的 `path`：**相对配置工程根（contest 根）的路径**（如 `"day1/conf.json"`、`"day1/p1"`）——客户端不应依赖服务端文件系统绝对命名空间
- contest 根位置由 `workspace/open` 返回的 `contest.uri` 标识；客户端 `join(contest.uri, path)` 即完整位置

**scope 转义（重要）：**

`scope` 由 `/` 分隔层级（`day` 或 `day/problem`）。若 day/problem 的**配置 key 内部本身含 `/`**（如 `subdir` 使用了相对路径 `../problems`），客户端必须把 key 内的分隔字符转义后传入，规则与 JSON Pointer 一致：

| 字符 | 转义 |
| ---- | ---- |
| `/`  | `~1` |
| `~`  | `~0` |

- 转义顺序：先 `~` -> `~0`，再 `/` -> `~1`；服务端按 `/` 切分后逐段还原。
- `problem/list` / `problem/get` 返回的 `path` 已按此规则转义，**可直接作为 `scope` / `problem` 参数回传**。
- `"contest"` 为保留字（表示 Contest 层级）；不建议将 day 命名为 `contest`。

示例：day key 为 `../problems`、problem key 为 `p1` 时：

```
problem/list 返回 path:  ..~1problems/p1
config/get   scope:      ..~1problems
problem/get  problem:    ..~1problems/p1
run/create   problem:    ..~1problems/p1
```

## 5. 错误码

| code     | 含义                                                               |
| -------- | ------------------------------------------------------------------ |
| `-32700` | 解析错误                                                           |
| `-32600` | 非法请求（未 initialize、重复 initialize、shutdown 后调用等）      |
| `-32601` | 方法不存在                                                         |
| `-32602` | 参数非法                                                           |
| `-32000` | 内部错误                                                           |
| `-32001` | 会话不存在                                                         |
| `-32002` | 无效工程（无有效 conf.json）                                       |
| `-32003` | 编译失败                                                           |
| `-32004` | 运行失败                                                           |
| `-32005` | 配置字段非法（指针路径或值校验失败，或引用了不存在的 day/problem） |
| `-32006` | run 不存在                                                         |
| `-32007` | 配置 revision 冲突（乐观并发）                                     |

**错误边界（重要）：**

- `testStatus`（`RE`/`TLE`/`WA`/`UKE`/...）描述**被测对象的裁决结果**——程序崩溃是正常评测结果 `RE`。
- RPC error（`-32003`/`-32004`/`-32000`）描述**服务端无法完成该操作**——评测基础设施故障（checker 崩溃、sandbox 失败、可执行文件缺失等）。

错误响应格式：

```json
{ "jsonrpc": "2.0", "id": 1, "error": { "code": -32001, "message": "会话不存在", "data": null } }
```

## 6. 方法

### 6.1 生命周期

**initialize**

```
-> { "jsonrpc": "2.0", "method": "initialize", "id": 1, "params": { "clientInfo": { "name": "My IDE", "version": "1.0" } } }
<- { "jsonrpc": "2.0", "id": 1, "result": {
    "protocolVersion": "0.1",
    "serverInfo": { "name": "tuack-ng-rpc", "version": "1.1.0-alpha.1" },
    "capabilities": ["workspace", "config", "problem", "run", "ren"]
} }
```

**shutdown**（通知服务端进入 Shutdown 状态，随后等待 exit/EOF）

```
-> { "jsonrpc": "2.0", "method": "shutdown", "id": 2 }
<- { "jsonrpc": "2.0", "id": 2, "result": null }
```

**exit**（通知，无响应）

```
-> { "jsonrpc": "2.0", "method": "exit" }
```

### 6.2 会话（工作区）

**workspace/open** — 创建 session。`uri` 指向工程内任意目录（实现沿目录向上查找 conf.json）。**每次调用都创建独立 session**；无有效工程时 `contest` 为 `null`，后续涉及 config/problem/run 的方法返回 `-32002`。

```
-> { "jsonrpc": "2.0", "method": "workspace/open", "id": 3, "params": { "uri": "file:///home/pulsar/contest" } }
<- { "jsonrpc": "2.0", "id": 3, "result": {
    "sessionId": "s-1",
    "workspace": { "uri": "file:///home/pulsar/contest" },
    "contest": {
      "name": "demo",
      "days": ["day1", "day2"],
      "uri": "file:///home/pulsar/contest"
    }
  } }
```

`workspace.uri` 为 `workspace/open` 请求的目录；`contest.uri` 为沿目录向上发现的**工程根**（可能不同于 `workspace.uri`）。所有返回的 `path` 均相对 `contest.uri`。

客户端维护自己的「当前选中位置」，通过 `scope` 显式指定层级；本协议不跟踪 IDE 的 UI 状态。

**workspace/close** — 销毁 session：取消该 session 下所有未完成 run（发 `run/finished(state:cancelled)` 并移除），然后销毁 session。

```
-> { "jsonrpc": "2.0", "method": "workspace/close", "id": 4, "params": { "sessionId": "s-1" } }
<- { "jsonrpc": "2.0", "id": 4, "result": null }
```

**workspace/list** — 返回当前 RPC server 进程中存活的所有 session（同一 uri 可被多次 open）。

```
-> { "jsonrpc": "2.0", "method": "workspace/list", "id": 5 }
<- { "jsonrpc": "2.0", "id": 5, "result": { "sessions": [ { "sessionId": "s-1", "uri": "file:///home/pulsar/contest" } ] } }
```

### 6.3 配置

**config/schema** — 返回三种 conf.json 的 JSON Schema（draft-07），供客户端渲染表单/提示。键名与文件完全一致（`FileView` 视图）。结构见附录。

```
-> { "jsonrpc": "2.0", "method": "config/schema", "id": 6 }
<- { "jsonrpc": "2.0", "id": 6, "result": { "contest": {...}, "day": {...}, "problem": {...} } }
```

**config/get** — 读取指定 scope 的配置（`FileView` JSON，即文件实际内容）。`scope` 缺省为 `"contest"`。返回 **session-global revision**（初始为 `0`，仅表示"未经本 session 修改"）。

```
-> { "jsonrpc": "2.0", "method": "config/get", "id": 7, "params": { "sessionId": "s-1", "scope": "day1" } }
<- { "jsonrpc": "2.0", "id": 7, "result": {
    "revision": 3,
    "config": { "version": 7, "folder": "day", "name": "day1", "title": "...", "compile": { "cpp": "-O2 -std=c++17" }, ... },
    "path": "day1/conf.json",
    "uri": "file:///home/pulsar/contest/day1/conf.json"
} }
```

**config/set** — 修改指定 scope 的任意字段（JSON Pointer 定位，值任意 JSON；`field=""` 替换整个 FileView 文档）。

- **revision 为 session-global**：任何 scope 的配置修改都使 session 全局计数 +1。
- 可选携带 `revision`（取自 `config/get`/`reload`）：不匹配则返回 `-32007`；不携带不校验。
- **原子性**：revision 校验 -> 读取配置 -> 修改 -> FileView 反序列化校验 -> 写回文件 -> 重新 `load()` 刷新 session 缓存 -> revision+1，作为一个原子操作（同一 session 内串行执行）。
- **写回副作用**：文件内容按 `FileView` 序列化落盘，数值字段（如 `"memory limit"`）可能被规范化为标准表示（见附录字节格式说明）。

```
-> { "jsonrpc": "2.0", "method": "config/set", "id": 8, "params": {
    "sessionId": "s-1", "scope": "day1/p1",
    "field": "/title", "value": "新标题",
    "revision": 3
} }
<- { "jsonrpc": "2.0", "id": 8, "result": { "revision": 4, "config": { "version": 7, "folder": "problem", "title": "新标题", ... } } }
```

**config/reload** — 重新采纳磁盘上的配置（外部编辑器改动后同步）。**仅当 session 观察到配置语义发生变化时 revision +1；无变化 reload 不递增。**

```
-> { "jsonrpc": "2.0", "method": "config/reload", "id": 9, "params": { "sessionId": "s-1", "scope": "day1" } }
<- { "jsonrpc": "2.0", "id": 9, "result": { "revision": 3, "config": {...}, "path": "day1/conf.json", "uri": "file:///home/pulsar/contest/day1/conf.json" } }
```

返回结构与 `config/get` 一致（`path`/`uri` 同语义）。

**config/migrate** — 强制迁移整个工程配置并写回（成功即视为配置语义变化，revision +1）。

```
-> { "jsonrpc": "2.0", "method": "config/migrate", "id": 10, "params": { "sessionId": "s-1" } }
<- { "jsonrpc": "2.0", "id": 10, "result": { "migrated": true, "notices": [] } }
```

### 6.4 问题

**problem/list** — 列举指定层级下的题目（**ProblemDescriptor 列表**）。`scope` 缺省为 `"contest"`。

```
-> { "jsonrpc": "2.0", "method": "problem/list", "id": 11, "params": { "sessionId": "s-1", "scope": "contest" } }
<- { "jsonrpc": "2.0", "id": 11, "result": { "problems": [
    { "name": "p1", "title": "第一题", "problemType": "program", "path": "day1/p1" },
    { "name": "p2", "title": "第二题", "problemType": "program", "path": "day1/p2" }
] } }
```

**problem/get** — 返回单个题目的 **ProblemDescriptor**（领域概念，而非 tuack 内部 ProblemConfig 的序列化投影）。`problem` 为 `"<day>/<problem>"` 绝对标识。

```
-> { "jsonrpc": "2.0", "method": "problem/get", "id": 12, "params": { "sessionId": "s-1", "problem": "day1/p1" } }
<- { "jsonrpc": "2.0", "id": 12, "result": { "problem": {
    "name": "p1", "title": "第一题", "problemType": "program",
    "timeLimitMs": 2000, "memoryLimitBytes": 268435456, "fileIo": null,
    "data":   [ { "id": 1, "score": 50, "subtask": 0 }, { "id": 2, "score": 50, "subtask": 0 } ],
    "samples": [ { "id": 1, "input": "1.in", "output": "1.ans" } ],
    "checker": null, "validator": null,
    "path": "day1/p1"
} } }
```

- `data` 为**展开后**的数据点列表（bundle 已展开为单点），其 `id` 即 `run/judge` 的 `testId`。
- `memoryLimitBytes` 为配置 `"memory limit"` 解析后的字节数（进制换算见附录）。

### 6.5 评测（run）

Run 是 session 下的**评测会话**。**组织测试方法的能力完全交给调用者**：服务端只提供原子能力（准备 / 单点评测 / 判分汇总），调用者自行编排「测哪些点、什么顺序、何时停止、是否部分测试、是否失败即停」。

```text
run/create（准备：编译 std 代码 + checker，异步）
    |
    +-- run/judge { testId }  --> 单点结果（同步）     # 调用者循环调用、自行决定
    +-- run/judge { testId }  --> 单点结果（同步）
    +-- ...
    |
    +-- run/score             --> 判分汇总（已测点）
```

**run/create** — 创建评测会话并异步准备（编译被测代码与 checker）。编译输出经 `run/output(channel:compiler)` 事件送达，准备完成发 `run/ready`；编译失败发 `run/finished(state:error)`。不评测任何点。

```
-> { "jsonrpc": "2.0", "method": "run/create", "id": 13, "params": { "sessionId": "s-1", "problem": "day1/p1", "target": "data" } }
<- { "jsonrpc": "2.0", "id": 13, "result": { "runId": "r-1" } }
```

- `target`：`"data"` / `"sample"`，决定用 `data/` 还是 `sample/` 的数据点。
- `tester`：被测代码标识（题目配置 `tests` 的 key，如 `"std"` / `"brute"`），**可选，缺省 `"std"`**；`"std"` 不存在时回退 `tests` 的第一个条目。显式指定的 `tester` 必须存在，否则 `-32005`。
- **交互题**（`problemType: "interactive"`）：以 NOI 风格 grader 链接方式评测——编译被测代码时一并链接配置的 `interactive.grader` 与 `interactive.header`（样例数据用 `sample_grader`，缺省回退 `grader`）。不支持交互的运行器（如非 C++ 语言）返回错误。
- `testId` 取自 `problem/get` 的 `data`/`samples` 列表中的 `id`（字符串）。

**run/judge** — 对单个数据点评测（**同步**，阻塞至该点完成），返回结果。

- **同一个 run 同时只允许一个 judge 操作。** 协议在 NDJSON 单连接下请求天然串行；实现额外以 per-run 互斥串行化 judge，不会并发执行同一 run 的两次评测。
- 该请求产生的 `run/output` 事件（如 `channel:judge` 的 checker 输出）**必须先于其 RPC response 发送**。
- run 未就绪（`preparing`）-> `-32602`；编译已失败 -> `-32003`；已取消/已关闭 -> `-32602`。

```
-> { "jsonrpc": "2.0", "method": "run/judge", "id": 14, "params": { "sessionId": "s-1", "runId": "r-1", "testId": "1" } }
<- { "jsonrpc": "2.0", "id": 14, "result": {
    "testId": "1", "status": "AC",
    "timeMs": 12, "memoryBytes": 4718592,
    "message": "AC", "score": 1.0, "fullScore": 50
} }
```

- `score` 为归一化得分，`AC=1.0`，`PC∈(0,1)`（比例分/100），其余 `0.0`；`fullScore` 为该点满分。
- `timeMs`/`memoryBytes` 在 `TLE`/`MLE` 时为 `null`。
- `message` 为 checker 报告（如 `"AC"`、`"Wrong answer on test 7"`）或错误诊断。
- 被测对象行为（RE/TLE/WA...）经 `status` 表达；评测基础设施故障返回 RPC error。

**run/score** — 对**已评测**的点做 subtask 判分汇总（同步）。未测点按 `score=0` 参与聚合；组满分按全量配置计算。

```
-> { "jsonrpc": "2.0", "method": "run/score", "id": 15, "params": { "sessionId": "s-1", "runId": "r-1" } }
<- { "jsonrpc": "2.0", "id": 15, "result": {
    "judged": 2, "total": 10,
    "report": {
      "groups": [ { "id": 0, "earned": 100, "full": 100 } ],
      "total": 100, "fullScore": 100
    }
} }
```

`report` 是 tuack-ng 当前评分规则（subtask 按 `sum`/`max`/`min` 聚合）对已评测点执行后的结果；协议不重述评分算法。

**run/cancel** — 取消正在进行的准备或单点评测，run 进入 `cancelled`。

- 取消为**协作式**：已启动的单点评测无法被中断（原子 judge 无取消钩子），取消标记在 judge 返回后生效；准备阶段（编译）可被取消。

```
-> { "jsonrpc": "2.0", "method": "run/cancel", "id": 16, "params": { "sessionId": "s-1", "runId": "r-1" } }
<- { "jsonrpc": "2.0", "id": 16, "result": null }
```

**run/get** — 查询 run 会话状态。**事件是增量通知，非权威；`run/get` 才是 authoritative state**，客户端断线重连后以此恢复。

```
-> { "jsonrpc": "2.0", "method": "run/get", "id": 17, "params": { "sessionId": "s-1", "runId": "r-1" } }
<- { "jsonrpc": "2.0", "id": 17, "result": {
    "state": "ready",
    "problem": "day1/p1", "target": "data", "tester": "std",
    "judged": [ { "testId": "1", "status": "AC", "timeMs": 12, "memoryBytes": 4718592, "message": null, "score": 1.0, "fullScore": 50 } ],
    "report": null,
    "error": null
} }
```

`runState` 是**生命周期状态**，不描述当前 RPC 操作是否正在执行：

- `preparing`：正在编译准备
- `ready`：已完成准备，可接受 `run/judge`
- `cancelled`：被取消（`run/cancel` 或 `workspace/close`）
- `error`：准备/运行失败（`error` 字段携带信息）
- `closed`：随 session 关闭被取消并从服务端移除，不再可查询（对已移除 run 的任何操作返回 `-32006`）

当前实现中，`run/finished` 事件仅发出 `cancelled` / `error` 两种终态。

## 7. 事件（服务端 -> 客户端，通知）

- **事件是增量/失效通知，不是状态存储。** 客户端不得假设收到全部事件即拥有完整状态；应以 `run/get` 为权威。
- **每个事件携带 `seq`**：进程级全局单调递增，从 1 开始，不因 session 创建/关闭而重置。客户端可借此检测**已接收事件序列中的非连续性**（如 `17 -> 19` 说明丢了 18）；`seq` 不提供可靠持久化或重放语义。
- 命令式请求（`run/create`）与状态式事件（`run/started`）命名区分。

**run/started**

```json
{ "jsonrpc": "2.0", "method": "run/started", "params": { "seq": 1, "sessionId": "s-1", "runId": "r-1", "problem": "day1/p1", "target": "data", "tester": "std" } }
```

**run/output** — `testId` 可空：编译器/系统输出为 `null`，judge 过程的 checker 输出关联对应测试点。`channel` 定义 `stdout`/`stderr`/`compiler`/`judge`/`system`；**当前实现仅发出 `compiler` 与 `judge` 两通道**。

```json
{ "jsonrpc": "2.0", "method": "run/output", "params": { "seq": 2, "sessionId": "s-1", "runId": "r-1", "testId": null, "channel": "compiler", "text": "g++ -O2 -std=c++17 ..." } }
```

**run/ready** — 准备完成，可开始 `run/judge`。

```json
{ "jsonrpc": "2.0", "method": "run/ready", "params": { "seq": 3, "sessionId": "s-1", "runId": "r-1" } }
```

**run/finished** — `state` 使用 `runState`（`cancelled` / `error` / `closed`）。

```json
{ "jsonrpc": "2.0", "method": "run/finished", "params": { "seq": 4, "sessionId": "s-1", "runId": "r-1", "state": "error", "error": "编译错误：..." } }
```

## 8. 完整时序

**评测（调用者组织）：**

```
client -> { "jsonrpc": "2.0", "method": "run/create", "id": 13, "params": { "sessionId": "s-1", "problem": "day1/p1", "target": "data" } }
server <- { "jsonrpc": "2.0", "id": 13, "result": { "runId": "r-1" } }
server <- { "jsonrpc": "2.0", "method": "run/started",  "params": { "seq": 1, "sessionId": "s-1", "runId": "r-1", "problem": "day1/p1", "target": "data", "tester": "std" } }
server <- { "jsonrpc": "2.0", "method": "run/output",   "params": { "seq": 2, ..., "channel": "compiler", "text": "..." } }
server <- { "jsonrpc": "2.0", "method": "run/ready",    "params": { "seq": 3, "sessionId": "s-1", "runId": "r-1" } }

client -> { "jsonrpc": "2.0", "method": "run/judge", "id": 14, "params": { "sessionId": "s-1", "runId": "r-1", "testId": "1" } }
server <- { "jsonrpc": "2.0", "method": "run/output", "params": { "seq": 4, ..., "testId": "1", "channel": "judge", "text": "..." } }   # 先于响应
server <- { "jsonrpc": "2.0", "id": 14, "result": { "testId": "1", "status": "AC", "score": 1.0, "fullScore": 50, ... } }

client -> { "jsonrpc": "2.0", "method": "run/judge", "id": 15, "params": { "sessionId": "s-1", "runId": "r-1", "testId": "2" } }
server <- { "jsonrpc": "2.0", "id": 15, "result": { "testId": "2", "status": "WA", "score": 0.0, "fullScore": 50, ... } }
# 调用者自行决定：只测到这里（部分测试），或继续，或就此判分
client -> { "jsonrpc": "2.0", "method": "run/score", "id": 16, "params": { "sessionId": "s-1", "runId": "r-1" } }
server <- { "jsonrpc": "2.0", "id": 16, "result": { "judged": 2, "total": 10, "report": { "groups": [ { "id": 0, "earned": 50, "full": 100 } ], "total": 50, "fullScore": 100 } } }
```

## 9. 渲染（ren）

渲染复用评测会话的异步 task + 事件模型。**产物落临时目录，由调用者决定处理方法**（移动到 `statements/`、直接预览或删除）。`template` 对应 `assets/templates/{template}.json` 清单。

### 9.1 ren/preview（同步）

返回**模板展开后**的题面 Markdown（移除 HTML 注释 + MiniJinja 展开，未做 AST 解析/渲染），供客户端实时预览/编辑。`scope` 必须定位到单个题目；`template` 缺省时使用默认模板参数（Markdown 目标、默认继承链）。

```
-> { "jsonrpc": "2.0", "method": "ren/preview", "id": 16, "params": { "sessionId": "s-1", "scope": "day1/p1", "template": "cnoi" } }
<- { "jsonrpc": "2.0", "id": 16, "result": { "markdown": "# 第一题\n\n从文件 _p1.in_ 中读入数据。...", "warnings": [], "lineMap": [ { "source": 3, "rendered": 1 } ] } }
```

**`lineMap`（渲染前后行映射）**：`[{ "source": <模板行号>, "rendered": <渲染后行号> }]`

- `source` 为**原始 `statement.md` 文件行号**。HTML 注释被移除；注释行的行首哨兵（在 `<!--` 标记之前）保留，映射到其留下的空行，注释体内的行无映射
- `rendered` 为展开后 Markdown 的行号（1 起）
- 同一 `source` 渲染多次（如 `{% for %}` 循环体内的模板行）时，仅记录**第一次出现**的 `rendered`
- 纯 jinja 语句行（`{% for %}`/`{% endfor %}` 等）渲染后产生空行并映射到该空行（minijinja 文本块/语句块语义的直接结果，属预期行为）
- 返回的 `markdown` 已移除哨兵，无残留

客户端可据此建立双向索引（点击渲染行定位模板行、编辑模板行定位预览行）。

### 9.2 ren/run（异步）

渲染题面到**临时目录**并返回该目录（异步，立即返回 `taskId`）。`scope` 指定渲染层级（`"contest"` -> 全部 day；`"<day>"` -> 该 day；`"<day>/<problem>"` -> 单题），缺省为 `"contest"`。模板不存在 -> `-32005`。

```
-> { "jsonrpc": "2.0", "method": "ren/run", "id": 17, "params": { "sessionId": "s-1", "template": "cnoi", "scope": "day1" } }
<- { "jsonrpc": "2.0", "id": 17, "result": { "taskId": "t-1" } }
```

### 9.3 ren/cancel / ren/get

```
-> { "jsonrpc": "2.0", "method": "ren/cancel", "id": 18, "params": { "sessionId": "s-1", "taskId": "t-1" } }
<- { "jsonrpc": "2.0", "id": 18, "result": null }

-> { "jsonrpc": "2.0", "method": "ren/get", "id": 19, "params": { "sessionId": "s-1", "taskId": "t-1" } }
<- { "jsonrpc": "2.0", "id": 19, "result": {
    "state": "finished",
    "template": "cnoi",
    "progress": { "done": 2, "total": 2 },
    "tmpDir": "/tmp/tuack-ng-ren-XXXX",
    "files": [ { "path": "day1.pdf" } ],
    "warnings": [],
    "error": null
} }
```

- `ren/get` 是 authoritative state；`state` 使用 `renState`。
- `files[].path` 为**相对临时目录**的路径，客户端 `join(tmpDir, path)` 即完整路径。
- `ren/cancel` 为协作式：正在渲染的 day 无法中断，取消标记在下一个 day 前生效。
- `workspace/close` 会取消该 session 的渲染任务并从服务端移除。

### 9.4 ren 事件

**ren/started**

```json
{ "jsonrpc": "2.0", "method": "ren/started", "params": { "seq": 5, "sessionId": "s-1", "taskId": "t-1", "template": "cnoi", "scope": "day1" } }
```

**ren/output** — `channel` 使用 `renderer`（typst 编译等渲染命令输出）/ `system`（服务日志）。

```json
{ "jsonrpc": "2.0", "method": "ren/output", "params": { "seq": 6, "sessionId": "s-1", "taskId": "t-1", "channel": "renderer", "text": "..." } }
```

**ren/progress** — 每个 day 渲染完成时发送。

```json
{ "jsonrpc": "2.0", "method": "ren/progress", "params": { "seq": 7, "sessionId": "s-1", "taskId": "t-1", "done": 1, "total": 2, "item": "day1" } }
```

**ren/finished** — `status` 使用 `renState` 的终态（`finished` / `error` / `cancelled`）。

```json
{ "jsonrpc": "2.0", "method": "ren/finished", "params": { "seq": 8, "sessionId": "s-1", "taskId": "t-1", "status": "finished", "tmpDir": "/tmp/tuack-ng-ren-XXXX", "files": [ { "path": "day1.pdf" } ], "warnings": [], "error": null } }
```

## 10. 附录：config/schema 内容（三份 JSON Schema）

键名/结构对应 `FileView`（与 conf.json 文件一致）。

### contest

`required: [version, folder, name, subdir, title, "short title"]`

- `version: integer`
- `folder: const "contest"`
- `name: string`、`title: string`、`"short title": string`
- `subdir: string[]`
- 可选：`use_pretest`、`noi_style`、`file_io`（`boolean`）

### day

`required: [version, folder, name, subdir, title, compile]`

- `folder: const "day"`
- `compile: { string: string }`
- `"start time"`、`"end time"`：`integer[6]`（年/月/日/时/分/秒）
- 可选：`use_pretest`、`noi_style`、`file_io`（`boolean`）

### problem

`required: [version, folder, type, name, title, "time limit", "memory limit", dmk]`

- `folder: const "problem"`；`type: enum ["program","output","interactive"]`
- `"time limit": number`（秒）；`"memory limit": string`
- **字节格式说明**：`"memory limit"` 为字节大小字符串。`config/get`/`set` 返回的 `FileView` 中该字段按 bytesize 显示格式输出（二进制单位，如 `"244.1 MiB"`）；`config/set` 的 `value` 接受十进制（`"256MB"`）或二进制（`"244.1 MiB"`）单位写法。解析与显示存在十进制/二进制换算，`problem/get` 的 `memoryLimitBytes` 可能因换算与直觉的十进制值有轻微偏差。
- `dmk: enum ["skip","input","output","on"]`
- `args: { string: integer|number|string|boolean }`
- `interactive: { grader, header, sample_grader?, dmk_grader? }`
- `generator` / `checker` / `validator`：`{ data: { source, deps? }, sample?: 同 }`（generator 的 data 另含 `validate?: boolean`）
- `samples: [{ id, input?, output?, args?, dmk? }]`
- `data`：单点 `{ id: integer, score, subtask?, input?, output?, args?, dmk? }` 或 bundle `{ id: integer[], score, subtask?, args?, dmk? }`
- `subtasks: { integer: enum ["sum","max","min"] }`
- `tests: { string: { expected: string|string[], path: string } }`
