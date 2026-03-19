use chrono::{DateTime, Local};
use colored::*;
use crc32fast::Hasher as Crc32;
use md5::Md5;
use rayon::prelude::*; // 引入并行计算
use sha1::Sha1;
use sha2::{Digest, Sha256, Sha384, Sha512};
use std::env;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::Path;
use std::time::Instant;

const FILE_SIZE_1GB: u64 = 1024 * 1024 * 1024;
const LABEL_WIDTH: usize = 12;
const SEPARATOR: &str = "----------------------------------------";

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    // 如果没有参数或参数为空，显示帮助信息
    if args.is_empty() || args.iter().all(|s| s.trim().is_empty()) {
        print_usage();
        return;
    }

    // 检查是否是生成 SFV 模式
    if args[0] == "--sfv" {
        if args.len() < 2 {
            println!("{}", "错误: 使用 --sfv 参数时必须指定目标文件或目录".red());
            return;
        }
        create_sfv(&args[1..]);
        return;
    }

    // 普通处理模式
    let files: Vec<&String> = args.iter().filter(|s| !s.trim().is_empty()).collect();
    println!("\n正在处理 {} 个项目...\n", files.len());

    for (i, file_path) in files.iter().enumerate() {
        println!("[{}/{}]", i + 1, files.len());
        
        if file_path.to_lowercase().ends_with(".sfv") {
            verify_sfv(file_path);
        } else {
            process_file(file_path);
        }
    }
}

fn print_usage() {
    println!("{}", "CarrotHash CLI v1.0.0 - 快速哈希计算与 SFV 工具".green().bold());
    println!("\n使用方法:");
    println!("  计算哈希:   carrot_hash_cli.exe <文件1> [文件2] ...");
    println!("  验证 SFV:   carrot_hash_cli.exe <校验文件.sfv>");
    println!("  生成 SFV:   carrot_hash_cli.exe --sfv <目录或多个文件>");
    println!("\n支持算法: CRC32, MD5, SHA1, SHA256, SHA384, SHA512");
    println!("\n示例:");
    println!("  carrot_hash_cli.exe myfile.txt");
    println!("  carrot_hash_cli.exe --sfv D:\\Photos");
}

/// 模式 A: 普通文件哈希计算
fn process_file(filepath: &str) {
    let start_time = Instant::now();
    let path = Path::new(filepath);

    println!("{}", SEPARATOR);
    print_info("路径:", path.to_string_lossy().as_ref());

    // 检查文件元数据
    let metadata = match fs::metadata(path) {
        Ok(m) => m,
        Err(e) => {
            print_error(&format!("无法访问文件: {}", e));
            return;
        }
    };

    if !metadata.is_file() {
        print_error("目标路径不是一个文件。");
        return;
    }

    let file_size = metadata.len();

    // 大文件确认
    if file_size > FILE_SIZE_1GB {
        let gb_size = file_size as f64 / FILE_SIZE_1GB as f64;
        print_warning(&format!("大文件 ({:.2} GB) - 计算可能需要较长时间", gb_size));
        
        if !prompt_continue() {
            print_info("操作:", "用户已跳过");
            return;
        }
    }

    print_info("文件名:", path.file_name().unwrap_or_default().to_string_lossy().as_ref());
    print_info("大小:", &format_file_size(file_size));
    
    if let Ok(mod_time) = metadata.modified() {
        let dt: DateTime<Local> = mod_time.into();
        print_info("修改日期:", &dt.format("%Y-%m-%d %H:%M:%S").to_string());
    }

    println!();

    // 多算法同步计算
    match compute_all_hashes(path) {
        Ok(h) => {
            print_info("CRC32:", &h.0);
            print_info("MD5:", &h.1);
            print_info("SHA1:", &h.2);
            print_info("SHA256:", &h.3);
            print_info("SHA384:", &h.4);
            print_info("SHA512:", &h.5);
        }
        Err(e) => print_error(&format!("计算失败: {}", e)),
    }

    let elapsed = start_time.elapsed().as_secs_f64();
    if elapsed > 1.0 {
        print_info("耗时:", &format!("{:.2} 秒", elapsed));
    }
    println!();
}

/// 模式 B: 验证 SFV 文件
fn verify_sfv(sfv_filepath: &str) {
    let start_time = Instant::now();
    let sfv_path = Path::new(sfv_filepath);
    
    println!("{}", SEPARATOR);
    print_info("任务:", "SFV 完整性校验");
    print_info("校验文件:", sfv_path.file_name().unwrap_or_default().to_string_lossy().as_ref());
    println!();

    let file = match File::open(sfv_path) {
        Ok(f) => f,
        Err(e) => {
            print_error(&format!("无法打开 SFV: {}", e));
            return;
        }
    };

    let reader = BufReader::new(file);
    let base_dir = sfv_path.parent().unwrap_or(Path::new("."));

    let (mut total, mut passed, mut failed, mut missing) = (0, 0, 0, 0);

    for line in reader.lines().map_while(Result::ok) {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with(';') { continue; }

        if let Some(idx) = trimmed.rfind(|c: char| c.is_whitespace()) {
            let fname = trimmed[..idx].trim();
            let expected = trimmed[idx..].trim().to_lowercase();
            
            total += 1;
            let t_path = base_dir.join(fname);
            print!("{:<40} ", fname);
            io::stdout().flush().unwrap();

            if !t_path.exists() {
                println!("{}", "不存在".yellow());
                missing += 1;
            } else {
                match compute_only_crc32(&t_path) {
                    Ok(actual) if actual == expected => {
                        println!("{}", "通过".green());
                        passed += 1;
                    }
                    Ok(actual) => {
                        println!("{} (期望: {}, 实际: {})", "失败".red(), expected, actual);
                        failed += 1;
                    }
                    Err(e) => {
                        println!("{} ({})", "读取错误".red(), e);
                        failed += 1;
                    }
                }
            }
        }
    }

    println!("{}", SEPARATOR);
    println!("校验结果: 总计: {}, 通过: {}, 失败: {}, 缺失: {}", 
             total, passed.to_string().green(), failed.to_string().red(), missing.to_string().yellow());
    println!("耗时: {:.2} 秒\n", start_time.elapsed().as_secs_f64());
}

/// 模式 C: 生成 SFV 文件
fn create_sfv(targets: &[String]) {
    let start_time = Instant::now();
    let mut files_to_hash = Vec::new();

    // 收集所有需要处理的文件
    for t in targets {
        let path = Path::new(t);
        if path.is_file() {
            files_to_hash.push(path.to_path_buf());
        } else if path.is_dir() {
            if let Ok(entries) = fs::read_dir(path) {
                for entry in entries.flatten() {
                    if entry.path().is_file() {
                        files_to_hash.push(entry.path());
                    }
                }
            }
        }
    }

    if files_to_hash.is_empty() {
        print_error("未找到有效文件进行生成。");
        return;
    }

    println!("正在为 {} 个文件生成校验数据...", files_to_hash.len());

    // 使用 Rayon 并行计算 CRC32，大幅提升多文件处理速度
    let results: Vec<(String, String)> = files_to_hash.par_iter().map(|p| {
        let fname = p.file_name().unwrap_or_default().to_string_lossy().into_owned();
        match compute_only_crc32(p) {
            Ok(crc) => (fname, crc),
            Err(_) => (fname, "ERROR".to_string()),
        }
    }).collect();

    // 写入 SFV 文件
    let output_name = format!("Generated_{}.sfv", Local::now().format("%Y%m%d_%H%M%S"));
    let mut f = File::create(&output_name).expect("无法创建 SFV 文件");
    
    writeln!(f, "; Generated by CarrotHash CLI").unwrap();
    writeln!(f, "; Date: {}", Local::now().to_rfc2822()).unwrap();
    
    for (name, crc) in results {
        if crc != "ERROR" {
            writeln!(f, "{} {}", name, crc).unwrap();
        }
    }

    println!("{}", SEPARATOR);
    println!("成功! 校验文件已保存至: {}", output_name.green().bold());
    println!("总计处理 {} 个文件，耗时 {:.2} 秒\n", files_to_hash.len(), start_time.elapsed().as_secs_f64());
}

// --- 通用算法函数 ---

fn compute_all_hashes(p: &Path) -> io::Result<(String, String, String, String, String, String)> {
    let mut file = File::open(p)?;
    let (mut c, mut m, mut s1, mut s256, mut s384, mut s512) = 
        (Crc32::new(), Md5::new(), Sha1::new(), Sha256::new(), Sha384::new(), Sha512::new());

    let mut buf = vec![0u8; 1024 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 { break; }
        let chunk = &buf[..n];
        c.update(chunk); m.update(chunk); s1.update(chunk);
        s256.update(chunk); s384.update(chunk); s512.update(chunk);
    }

    Ok((
        format!("{:08x}", c.finalize()),
        format!("{:x}", m.finalize()),
        format!("{:x}", s1.finalize()),
        format!("{:x}", s256.finalize()),
        format!("{:x}", s384.finalize()),
        format!("{:x}", s512.finalize()),
    ))
}

fn compute_only_crc32(p: &Path) -> io::Result<String> {
    let mut file = File::open(p)?;
    let mut c = Crc32::new();
    let mut buf = vec![0u8; 1024 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 { break; }
        c.update(&buf[..n]);
    }
    Ok(format!("{:08x}", c.finalize()))
}

// --- 辅助显示函数 ---

fn print_info(label: &str, val: &str) {
    println!("{:<width$}{}", label, val, width = LABEL_WIDTH);
}

fn print_error(msg: &str) {
    println!("{:<width$}{}", "错误:".red(), msg, width = LABEL_WIDTH);
}

fn print_warning(msg: &str) {
    println!("{:<width$}{}", "警告:".yellow(), msg, width = LABEL_WIDTH);
}

fn format_file_size(b: u64) -> String {
    let units = ["B", "KB", "MB", "GB", "TB"];
    let (mut s, mut i) = (b as f64, 0);
    while s >= 1024.0 && i < units.len() - 1 { s /= 1024.0; i += 1; }
    format!("{:.2} {}", s, units[i])
}

fn prompt_continue() -> bool {
    print!("是否继续? (y/n): ");
    io::stdout().flush().unwrap();
    let mut s = String::new();
    io::stdin().read_line(&mut s).is_ok() && (s.trim().to_lowercase() == "y" || s.trim().to_lowercase() == "yes")
}