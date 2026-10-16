//! 开机自启：往 `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` 写一条值，
//! 让当前用户登录时把 PPClip 拉起来。
//!
//! 为什么不用「写文件到启动文件夹」或计划任务：注册表 Run 键是 Windows 上最常规、
//! 权限最低（HKCU 不需要管理员）的做法，卸载时也只要删掉这一条值。
//!
//! **注册表是唯一的真相**：设置里不存 `autostart` 字段，界面上的开关每次都是
//! 现场读注册表问出来的，避免「设置说开着、注册表里没有」这种两处状态打架。

#[cfg(windows)]
mod imp {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE};
    use winreg::RegKey;

    const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    /// 注册表里的值名（任务管理器「启动」列表里显示的就是它）
    const VALUE_NAME: &str = "PPClip";

    /// 当前 exe 的完整路径，带引号（路径里有空格时必须引起来）
    fn exe_command() -> Result<String, String> {
        let exe = std::env::current_exe().map_err(|e| format!("读取 exe 路径失败: {e}"))?;
        Ok(format!("\"{}\"", exe.display()))
    }

    /// 注册表里登记的自启命令（None = 没登记）
    ///
    /// 只要 `KEY_QUERY_VALUE`：`KEY_READ` 会顺带要 `READ_CONTROL`，在收紧过 ACL 的
    /// 环境（受限令牌 / 企业策略）下会被拒，而读取一个值根本不需要那个权限。
    fn registered() -> Result<Option<String>, String> {
        let key = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags(RUN_KEY, KEY_QUERY_VALUE)
            .map_err(|e| format!("打开 Run 键失败: {e}"))?;
        match key.get_value::<String, _>(VALUE_NAME) {
            Ok(value) => Ok(Some(value)),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(format!("读取自启项失败: {err}")),
        }
    }

    /// 当前是否开机自启
    pub fn is_enabled() -> bool {
        match registered() {
            // 登记过就算开着：exe 换过路径（重装 / 换目录）时不应该在界面上骗用户说「已关闭」
            Ok(Some(_)) => true,
            Ok(None) => false,
            Err(err) => {
                log::warn!("查询开机自启失败: {err}");
                false
            }
        }
    }

    /// 打开 / 关闭开机自启
    ///
    /// 只要 `KEY_SET_VALUE`（`KEY_WRITE` 同样会顺带要 `READ_CONTROL`）。
    pub fn set(enabled: bool) -> Result<(), String> {
        let key = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags(RUN_KEY, KEY_SET_VALUE)
            .map_err(|e| format!("打开 Run 键失败: {e}"))?;

        if enabled {
            let command = exe_command()?;
            key.set_value(VALUE_NAME, &command)
                .map_err(|e| format!("写入自启项失败: {e}"))?;
            log::info!("已开启开机自启: {command}");
        } else {
            match key.delete_value(VALUE_NAME) {
                Ok(()) => log::info!("已关闭开机自启"),
                // 本来就没有 = 已经处于目标状态
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => return Err(format!("删除自启项失败: {err}")),
            }
        }
        Ok(())
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn is_enabled() -> bool {
        false
    }

    pub fn set(_enabled: bool) -> Result<(), String> {
        Err("开机自启目前只支持 Windows".into())
    }
}

pub use imp::{is_enabled, set};
