use std::env;

fn main() {
    // 只有在 Windows 下才执行资源注入
    if env::var("CARGO_CFG_TARGET_OS").unwrap() == "windows" {
        // let out_dir = env::var("OUT_DIR").unwrap();
        
        // 方案：为不同的二进制文件准备不同的资源编译
        // 我们手动定义我们要生成的 Bin 列表
        let bins = vec!["carrot-hash"];

        for bin in bins {
            let mut res = winres::WindowsResource::new();
            res.set_language(0x0804); // 简体中文
            
            let version = env!("CARGO_PKG_VERSION");
            res.set("FileVersion", version);
            res.set("ProductVersion", version);
            res.set("CompanyName", "Carrot Studio");
            res.set("LegalCopyright", "Copyright © 2026 Carrot Studio");

            // 根据 bin 名称设置特定描述
            if bin == "carrot-hash" {
                res.set("FileDescription", "CarrotHash - Fast File Hashing Tool");
                res.set("ProductName", "CarrotHash");
            } else if bin == "carrot-sfv" {
                res.set("FileDescription", "CarrotSFV - Fast File Verification Tool");
                res.set("ProductName", "CarrotSFV");
            }

            // 关键：告诉 winres 编译后的资源文件名，避免冲突
            // 这会生成类似 carrot-hash.res 的文件
            res.compile().expect(&format!("Can not compile resource for {}", bin));
        }
    }
}