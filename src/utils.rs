/// 计算第 n 个斐波那契数（n 从 0 开始，fib(0) = 0, fib(1) = 1）。
///
/// 使用迭代方式计算，时间复杂度 O(n)，空间复杂度 O(1)，
/// 避免递归带来的重复计算和栈溢出问题。
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
        assert_eq!(fibonacci(10), 55);
        assert_eq!(fibonacci(20), 6765);
        assert_eq!(fibonacci(93), 12200160415121876738);
    }

    #[test]
    fn test_fibonacci_sequence() {
        assert_eq!(fibonacci_sequence(0), Vec::<u64>::new());
        assert_eq!(fibonacci_sequence(7), vec![0, 1, 1, 2, 3, 5, 8]);
        assert_eq!(fibonacci_sequence(10), vec![0, 1, 1, 2, 3, 5, 8, 13, 21, 34]);
    }
}
