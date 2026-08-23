mod utils;

use std::env;
use std::process::exit;

fn main() {
    // 收集命令行参数（第一个参数是程序名本身，忽略）
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("用法: cargo run -- <n>");
        eprintln!("示例: cargo run -- 10");
        exit(1);
    }

    // 解析参数为 u32，非数字输入给出友好报错
    let n: u32 = match args[1].parse() {
        Ok(n) => n,
        Err(_) => {
            eprintln!("错误: \"{}\" 不是有效的非负整数", args[1]);
            exit(1);
        }
    };

    let result = utils::fibonacci(n);
    println!("fibonacci({n}) = {result}");
}
