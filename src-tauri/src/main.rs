// 发布版不弹出控制台窗口，请勿删除！
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // 助手模式：向指定进程的控制台发送 Ctrl+C 后立即退出，不启动界面
    let args: Vec<String> = std::env::args().collect();
    if args.len() == 3 && args[1] == venvdock_lib::CTRL_C_HELPER_ARG {
        let code = args[2].parse().map_or(1, venvdock_lib::run_ctrl_c_helper);
        std::process::exit(code);
    }
    venvdock_lib::run()
}
