#[cfg(target_os = "windows")]
use winresource::WindowsResource;

fn main() {
    // 只在特定文件变化时才重新运行构建脚本，避免触发不必要的全局重编
    println!("cargo:rerun-if-changed=assets/app_icon.ico");

    #[cfg(target_os = "windows")]
    {
        let mut res = WindowsResource::new();

        if std::path::Path::new("assets/app_icon.ico").exists() {
            res.set_icon("assets/app_icon.ico");
        }

        if let Err(e) = res.compile() {
            eprintln!("Windows 资源编译失败: {e}");
            std::process::exit(1);
        }
    }
}
