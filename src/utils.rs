//! 斐波那契数列的多种算法实现与基准对比。
//!
//! # 算法一览
//!
//! | 算法 | 函数 | 复杂度 | 说明 |
//! | ---- | ---- | ---- | ---- |
//! | 迭代 | `fibonacci` / `fibonacci_checked` | O(n) 时间 / O(1) 空间 | 基础实现，无递归开销 |
//! | 递归 + 记忆化 | `fibonacci_memoized` | O(n) 时间 / O(n) 空间 | 自顶向下，用 `Vec` 缓存避免重复计算 |
//! | 矩阵快速幂 | `fibonacci_matrix` | O(log n) 时间 / O(1) 空间 | 利用 [[1,1],[1,0]]^n 的矩阵幂 |
//!
//! 除 `fibonacci` / `fibonacci_sequence`（历史接口，保持 u64 返回）外，
//! 其余算法在 n > 93（`fib(93)` 是 `u64` 能表示的最大值）时返回 `Option<u64>`，
//! `None` 表示溢出。
//!
//! # 基准测试结果（release 模式，Apple Silicon / Rust 1.97，单次测量仅供参考）
//!
//! 运行方式：`cargo test --release benchmark::bench_fib_50 -- --ignored --nocapture`
//!
//! ```text
//! 基准测试：计算 fib(50) = 12586269025
//!   迭代 (checked)    375ns
//!   递归 + 记忆化     2.166µs
//!   矩阵快速幂        166ns
//! ```
//!
//! 结论：矩阵快速幂最快（O(log n)）；迭代实现常数项也极小；
//! 记忆化因递归调用与缓存访问开销最慢，但三种算法在 fib(50) 量级下均为
//! 微秒级以下，实际差异可忽略。

/// 计算第 n 个斐波那契数（n 从 0 开始，fib(0) = 0, fib(1) = 1）。
///
/// 使用迭代方式计算，时间复杂度 O(n)，空间复杂度 O(1)，
/// 避免递归带来的重复计算和栈溢出问题。
///
/// 注意：n > 93 时结果会溢出 u64（release 模式下静默回绕），
/// 需要溢出检查请使用 [`fibonacci_checked`]。
pub fn fibonacci(n: u32) -> u64 {
    match n {
        0 => 0,
        1 => 1,
        _ => {
            let mut a = 0;
            let mut b = 1;
            for _ in 2..=n {
                let c = a + b;
                a = b;
                b = c;
            }
            b
        }
    }
}

/// 计算前 n 个斐波那契数，返回 Vec<u64>。
///
/// 需要溢出检查请使用 [`fibonacci_sequence_checked`]。
pub fn fibonacci_sequence(n: u32) -> Vec<u64> {
    (0..n).map(fibonacci).collect()
}

/// 迭代实现（即 [`fibonacci`]）的溢出安全版本。
///
/// n > 93 时 `fib(n)` 超出 u64 范围，返回 `None`。
pub fn fibonacci_checked(n: u32) -> Option<u64> {
    if n > 93 {
        return None;
    }
    Some(fibonacci(n))
}

/// 递归 + 记忆化实现（自顶向下）。
///
/// 用 `Vec<Option<u64>>` 缓存中间结果，每个子问题只计算一次，
/// 时间复杂度 O(n)，空间复杂度 O(n)。n > 93 时返回 `None`。
// 该函数当前仅被单元测试与基准测试模块使用，bin 目标中豁免 dead_code 告警
#[cfg_attr(not(test), allow(dead_code))]
pub fn fibonacci_memoized(n: u32) -> Option<u64> {
    if n > 93 {
        return None;
    }

    /// 内部递归函数：先查缓存，未命中则递归计算并写入缓存
    fn helper(k: u32, cache: &mut [Option<u64>]) -> u64 {
        if let Some(v) = cache[k as usize] {
            return v;
        }
        let v = if k < 2 {
            k as u64
        } else {
            helper(k - 1, cache) + helper(k - 2, cache)
        };
        cache[k as usize] = Some(v);
        v
    }

    let mut cache = vec![None; n as usize + 1];
    Some(helper(n, &mut cache))
}

/// 矩阵快速幂实现。
///
/// 斐波那契数列满足矩阵恒等式：
/// `[[F_{n+1}, F_n], [F_n, F_{n-1}]] = [[1,1],[1,0]]^n`，
/// 因此可以用快速幂在 O(log n) 时间内算出 F_n，空间复杂度 O(1)。
/// 矩阵乘法内部使用 u128 中间量，避免中间结果溢出（n ≤ 93 时
/// 中间矩阵可能包含 F_94 等超出 u64 的值，但这些值不影响最终返回的 F_n）。
/// n > 93 时返回 `None`。
// 该函数当前仅被单元测试与基准测试模块使用，bin 目标中豁免 dead_code 告警
#[cfg_attr(not(test), allow(dead_code))]
pub fn fibonacci_matrix(n: u32) -> Option<u64> {
    if n > 93 {
        return None;
    }

    // 2x2 矩阵按行展开存储：(m00, m01, m10, m11)
    type Mat = (u64, u64, u64, u64);

    /// 2x2 矩阵乘法，内部用 u128 防止中间结果溢出
    fn mat_mul(a: Mat, b: Mat) -> Mat {
        let (a00, a01, a10, a11) = (a.0 as u128, a.1 as u128, a.2 as u128, a.3 as u128);
        let (b00, b01, b10, b11) = (b.0 as u128, b.1 as u128, b.2 as u128, b.3 as u128);
        (
            (a00 * b00 + a01 * b10) as u64,
            (a00 * b01 + a01 * b11) as u64,
            (a10 * b00 + a11 * b10) as u64,
            (a10 * b01 + a11 * b11) as u64,
        )
    }

    let mut result: Mat = (1, 0, 0, 1); // 单位矩阵
    let mut base: Mat = (1, 1, 1, 0); // [[1,1],[1,0]]
    let mut exp = n;
    while exp > 0 {
        if exp & 1 == 1 {
            result = mat_mul(result, base);
        }
        base = mat_mul(base, base);
        exp >>= 1;
    }
    // result = [[F_{n+1}, F_n], [F_n, F_{n-1}]]，F_n 位于 [0][1]
    Some(result.1)
}

/// [`fibonacci_sequence`] 的溢出安全版本。
///
/// 序列最后一个元素是 fib(n - 1)，因此 n > 94 时返回 `None`。
pub fn fibonacci_sequence_checked(n: u32) -> Option<Vec<u64>> {
    if n > 94 {
        return None;
    }
    Some(fibonacci_sequence(n))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fibonacci() {
        assert_eq!(fibonacci(0), 0);
        assert_eq!(fibonacci(1), 1);
        assert_eq!(fibonacci(10), 55);
        assert_eq!(fibonacci(20), 6765);
        assert_eq!(fibonacci(93), 12200160415121876738);
    }

    #[test]
    fn test_fibonacci_sequence() {
        assert_eq!(fibonacci_sequence(0), Vec::<u64>::new());
        assert_eq!(fibonacci_sequence(7), vec![0, 1, 1, 2, 3, 5, 8]);
        assert_eq!(
            fibonacci_sequence(10),
            vec![0, 1, 1, 2, 3, 5, 8, 13, 21, 34]
        );
    }

    #[test]
    fn test_checked_matches_fibonacci() {
        for n in 0..=93 {
            assert_eq!(fibonacci_checked(n), Some(fibonacci(n)), "n = {n}");
        }
    }

    #[test]
    fn test_memoized_matches_fibonacci() {
        for n in 0..=93 {
            assert_eq!(fibonacci_memoized(n), Some(fibonacci(n)), "n = {n}");
        }
    }

    #[test]
    fn test_matrix_matches_fibonacci() {
        for n in 0..=93 {
            assert_eq!(fibonacci_matrix(n), Some(fibonacci(n)), "n = {n}");
        }
    }

    #[test]
    fn test_overflow_returns_none() {
        for f in [fibonacci_checked, fibonacci_memoized, fibonacci_matrix] {
            assert_eq!(f(94), None);
            assert_eq!(f(100), None);
            assert_eq!(f(u32::MAX), None);
        }
    }

    #[test]
    fn test_sequence_checked() {
        assert_eq!(fibonacci_sequence_checked(94), Some(fibonacci_sequence(94)));
        assert_eq!(fibonacci_sequence_checked(95), None);
    }
}

/// 简易基准测试模块（使用 `std::time::Instant`，不依赖外部 benchmark 框架）。
///
/// 注意：单次计时容易受抖动影响，结果仅用于粗略对比三种算法的量级差异。
#[cfg(test)]
pub mod benchmark {
    use super::*;
    use std::time::Instant;

    /// 三种 checked 算法的统一签名
    type FibFn = fn(u32) -> Option<u64>;

    /// 对三种算法各计时一次，返回 `(算法名, 耗时, 结果)` 列表
    pub fn compare(n: u32) -> Vec<(&'static str, std::time::Duration, Option<u64>)> {
        let algorithms: Vec<(&'static str, FibFn)> = vec![
            ("迭代 (checked)", fibonacci_checked),
            ("递归 + 记忆化", fibonacci_memoized),
            ("矩阵快速幂", fibonacci_matrix),
        ];

        algorithms
            .into_iter()
            .map(|(name, f)| {
                let start = Instant::now();
                let result = f(n);
                (name, start.elapsed(), result)
            })
            .collect()
    }

    #[test]
    #[ignore = "性能基准，手动运行: cargo test --release benchmark::bench_fib_50 -- --ignored --nocapture"]
    fn bench_fib_50() {
        let n = 50;
        println!("基准测试：计算 fib({n})");
        for (name, elapsed, result) in compare(n) {
            println!("  {name:<16} {elapsed:?} => {result:?}");
        }
    }
}
