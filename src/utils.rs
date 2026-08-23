/// 计算第 n 个斐波那契数（n 从 0 开始，fib(0) = 0, fib(1) = 1）。
///
/// 使用迭代方式计算，时间复杂度 O(n)，空间复杂度 O(1)，
/// 避免递归带来的重复计算和栈溢出问题。
pub fn fibonacci(n: u32) -> u64 {
    match n {
        0 => 0,
        1 => 1,
        _ => {
            let mut prev = 0u64;
            let mut curr = 1u64;
            for _ in 2..=n {
                let next = prev + curr;
                prev = curr;
                curr = next;
            }
            curr
        }
    }
}

/// 计算前 n 个斐波那契数，返回 Vec<u64>。
#[allow(dead_code)]
pub fn fibonacci_sequence(n: u32) -> Vec<u64> {
    (0..n).map(fibonacci).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fibonacci() {
        assert_eq!(fibonacci(0), 0);
        assert_eq!(fibonacci(1), 1);
        assert_eq!(fibonacci(2), 1);
        assert_eq!(fibonacci(3), 2);
        assert_eq!(fibonacci(10), 55);
        assert_eq!(fibonacci(20), 6765);
    }

    #[test]
    fn test_fibonacci_sequence() {
        assert_eq!(fibonacci_sequence(7), vec![0, 1, 1, 2, 3, 5, 8]);
    }
}
