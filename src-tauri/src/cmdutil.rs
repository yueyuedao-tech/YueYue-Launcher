use std::ffi::OsStr;
use std::process::Command;

/// 构造子进程命令：Windows 下加 CREATE_NO_WINDOW，避免每次调用闪出 CMD 黑窗
/// （curl / java / taskkill / tar 等全部走这里）
pub fn no_console(program: impl AsRef<OsStr>) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}
