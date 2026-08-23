# fibonacci-cli

一个简单的命令行程序，接收一个非负整数 `n`，调用 `utils::fibonacci(n)` 计算并打印第 `n` 个斐波那契数（`fib(0) = 0`，`fib(1) = 1`）。

## 项目结构

```
.
├── Cargo.toml        # 项目配置
├── src/
│   ├── main.rs       # 命令行入口：解析参数、打印结果
│   └── utils.rs      # fibonacci / fibonacci_sequence 实现及单元测试
└── .gitignore
```

## 环境要求

- Rust 工具链（rustc + cargo），推荐通过 [rustup](https://rustup.rs/) 安装

## 运行

```bash
# 计算第 10 个斐波那契数
cargo run -- 10
# 输出: fibonacci(10) = 55

# 计算第 20 个斐波那契数
cargo run -- 20
# 输出: fibonacci(20) = 6765
```

### 参数说明

- 必须提供一个非负整数参数 `n`，否则程序会打印用法说明并以退出码 1 结束。
- 传入非数字参数（如 `cargo run -- abc`）会打印错误信息并以退出码 1 结束。

## 测试

```bash
# 运行全部单元测试
cargo test
```

测试覆盖边界值（`fib(0)`、`fib(1)`）、已知结果（`fib(10) = 55`、`fib(20) = 6765`）以及 `fibonacci_sequence` 的序列输出。
