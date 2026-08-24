# fibonacci-cli

基于 **axum** 的斐波那契数列与并发任务池 HTTP 服务，提供：

- `GET /fibonacci/:n` —— 返回第 `n` 个斐波那契数（`fib(0) = 0`，`fib(1) = 1`）
- `GET /fibonacci/sequence/:n` —— 返回前 `n` 个斐波那契数组成的数组
- `POST /task` —— 提交一个模拟耗时任务，返回 `task_id`
- `GET /task/:id` —— 查询任务状态与结果

内置 OpenAPI 文档（Swagger UI）与 `tracing` 结构化日志。

## 项目结构

```
.
├── Cargo.toml                  # 项目配置（axum / tokio / serde / utoipa / tracing）
├── Dockerfile                  # 多阶段构建：编译阶段（rust:1.97）+ 运行阶段（debian:bookworm-slim）
├── .dockerignore
├── src/
│   ├── main.rs                 # HTTP 服务入口：路由、处理器、OpenAPI、日志中间件
│   ├── concurrent_task_pool.rs # 基于 tokio 的并发任务池（spawn / cancel / await / 状态查询）
│   └── utils.rs                # fibonacci / fibonacci_sequence 实现及单元测试
└── .gitignore
```

## 环境要求

- Rust 工具链（rustc + cargo），推荐通过 [rustup](https://rustup.rs/) 安装

## 运行

```bash
# 编译并启动服务（默认监听 http://127.0.0.1:3000）
cargo run
```

监听地址可通过环境变量 `ADDR` 覆盖（例如 `ADDR=0.0.0.0:8080 cargo run`）。

## 使用 Docker 运行

项目提供了多阶段构建的 `Dockerfile`（编译阶段基于 `rust:1.97`，运行阶段基于 `debian:bookworm-slim`），最终镜像内监听 `0.0.0.0:3000`。

```bash
# 构建镜像
docker build -t fibonacci-cli .

# 运行容器，将宿主机的 3000 端口映射到容器
docker run -p 3000:3000 fibonacci-cli
```

## 接口

### GET /fibonacci/:n

计算第 `n` 个斐波那契数。

**参数**

| 参数 | 类型 | 说明 |
| ---- | ---- | ---- |
| `n` | 非负整数 | 取值范围 0 ~ 93（`fib(93)` 是 `u64` 能表示的最大值） |

**成功响应**（`200 OK`）

```json
{"n": 10, "result": 55}
```

**错误响应**

- `n` 非数字（如 `/fibonacci/abc`）→ `400 Bad Request`
- `n` 超出 `u64` 范围（如 `/fibonacci/100`）→ `400 Bad Request`，带错误信息：

```json
{"error": "n 过大：第 100 个斐波那契数超出 u64 范围（最大支持 n = 93）"}
```

### GET /fibonacci/sequence/:n

返回前 `n` 个斐波那契数组成的数组（复用 `utils::fibonacci_sequence`）。

**参数**

| 参数 | 类型 | 说明 |
| ---- | ---- | ---- |
| `n` | 非负整数 | 取值范围 0 ~ 94（序列最后一个元素是 `fib(n-1)`，`fib(93)` 是 `u64` 能表示的最大值） |

**成功响应**（`200 OK`）

```bash
curl http://127.0.0.1:3000/fibonacci/sequence/10
# [0,1,1,2,3,5,8,13,21,34]
```

- `n = 0` → `[]`
- `n = 1` → `[0]`

**错误响应**

- `n` 非数字（如 `/fibonacci/sequence/abc`）→ `400 Bad Request`
- `n` 超出 `u64` 范围（如 `/fibonacci/sequence/95`）→ `400 Bad Request`，带错误信息：

```json
{"error": "n 过大：前 95 个斐波那契数超出 u64 范围（最大支持 n = 94）"}
```

### POST /task

提交一个模拟耗时任务（内部 `tokio::time::sleep` 指定秒数），任务由全局并发任务池管理。

**请求体**（`application/json`）

| 字段 | 类型 | 说明 |
| ---- | ---- | ---- |
| `duration_secs` | 非负整数 | 任务模拟运行时长（秒） |

```bash
curl -X POST http://127.0.0.1:3000/task \
  -H 'Content-Type: application/json' \
  -d '{"duration_secs": 3}'
```

**成功响应**（`201 Created`）

```json
{"task_id": 0}
```

### GET /task/:id

查询指定任务的状态与结果。

```bash
curl http://127.0.0.1:3000/task/0
```

**响应**

- 运行中（`200 OK`）：

```json
{"status": "running"}
```

- 已完成（`200 OK`，`result` 为任务模拟的持续时长）：

```json
{"status": "completed", "result": 3}
```

- 不存在（`404 Not Found`）：

```json
{"status": "not_found"}
```

## OpenAPI 文档（Swagger UI）

服务启动后，可通过以下地址访问接口文档：

- **交互式 Swagger UI**：<http://127.0.0.1:3000/swagger-ui>
- **OpenAPI JSON**：<http://127.0.0.1:3000/api-docs/openapi.json>

Swagger UI 页面可在线浏览所有接口、填写参数并直接发起调试请求。

## tracing 日志

服务启动时初始化 `tracing-subscriber`，默认以 `INFO` 级别输出到控制台，`fmt` 格式形如：

```
2025-01-01T00:00:00.000000Z  INFO fibonacci_cli: HTTP 请求处理完成 method=GET path=/fibonacci/10 status=200 elapsed_ms=0
2025-01-01T00:00:01.000000Z  INFO fibonacci_cli: 提交模拟任务 task_id=0 duration_secs=3
```

每个 HTTP 请求由日志中间件记录：请求方法、路径、状态码、处理耗时；任务提交/查询处理器额外记录任务相关字段。

通过环境变量 `RUST_LOG` 可调整日志级别或过滤范围：

```bash
# 输出 DEBUG 及以上级别
RUST_LOG=debug cargo run

# 只看当前 crate 的 INFO 日志
RUST_LOG=fibonacci_cli=info cargo run

# 完全关闭日志
RUST_LOG=off cargo run
```

## 测试

```bash
# 运行单元测试（utils.rs 的 fibonacci 逻辑 + concurrent_task_pool.rs 的任务池逻辑）
cargo test

# 静态检查（把 warning 视为错误）
cargo clippy -- -D warnings

# 格式化代码
cargo fmt
```

测试覆盖：

- `utils.rs`：边界值（`fib(0)`、`fib(1)`）、已知结果（`fib(10) = 55`、`fib(20) = 6765`、`fib(93)` 为 `u64` 上限）以及序列输出。
- `concurrent_task_pool.rs`：任务完成取结果、取消返回 `None` 并移除、结果可重复查询、多任务并发、`cancel_all` 清空。
