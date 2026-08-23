# fibonacci-cli

基于 **axum** 的斐波那契数列 HTTP 服务。提供两个接口：`GET /fibonacci/:n` 返回第 `n` 个斐波那契数（`fib(0) = 0`，`fib(1) = 1`），`GET /fibonacci/sequence/:n` 返回前 `n` 个斐波那契数组成的数组，均返回 JSON。

## 项目结构

```
.
├── Cargo.toml        # 项目配置（axum / tokio / serde / serde_json）
├── Dockerfile        # 多阶段构建：编译阶段（rust:1.97）+ 运行阶段（debian:bookworm-slim）
├── .dockerignore
├── src/
│   ├── main.rs       # HTTP 服务入口：路由、处理器
│   └── utils.rs      # fibonacci / fibonacci_sequence 实现及单元测试
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

## 测试

```bash
# 运行单元测试（utils.rs 中的 fibonacci 逻辑）
cargo test
```

测试覆盖边界值（`fib(0)`、`fib(1)`）、已知结果（`fib(10) = 55`、`fib(20) = 6765`、`fib(93)` 为 `u64` 上限）以及 `fibonacci_sequence` 的序列输出。
