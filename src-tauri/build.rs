fn main() {
    // tauri-build 只会 rerun-if-changed tauri.conf.json / capabilities，**不**监听
    // icons/ 里的文件：换掉 icons/icon.ico（以及 icon.png）而不动配置时，build script
    // 不会重跑，resource.lib 里的旧图标就一直是旧图 —— 表现是「图标换了文件、任务管理器 /
    // 快捷方式 / 安装器里还是旧图标」，而且普通 rebuild 也救不回来。这里显式声明依赖。
    println!("cargo:rerun-if-changed=icons/icon.ico");
    println!("cargo:rerun-if-changed=icons/icon.png");
    println!("cargo:rerun-if-changed=icons/32x32.png");
    println!("cargo:rerun-if-changed=icons/128x128.png");
    println!("cargo:rerun-if-changed=icons/128x128@2x.png");
    tauri_build::build()
}
