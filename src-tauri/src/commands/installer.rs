use crate::utils::{platform, shell};
use serde::{Deserialize, Serialize};
use tauri::command;
use log::{info, warn, error, debug};

/// 环境检查结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentStatus {
    /// Node.js 是否安装
    pub node_installed: bool,
    /// Node.js 版本
    pub node_version: Option<String>,
    /// Node.js 版本是否满足要求 (>=22)
    pub node_version_ok: bool,
    /// OpenClaw 是否安装
    pub openclaw_installed: bool,
    /// OpenClaw 版本
    pub openclaw_version: Option<String>,
    /// 配置目录是否存在
    pub config_dir_exists: bool,
    /// 是否全部就绪
    pub ready: bool,
    /// 操作系统
    pub os: String,
}

/// 安装进度
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallProgress {
    pub step: String,
    pub progress: u8,
    pub message: String,
    pub error: Option<String>,
}

/// 安装结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallResult {
    pub success: bool,
    pub message: String,
    pub error: Option<String>,
}

/// 检查环境状态
#[command]
pub async fn check_environment() -> Result<EnvironmentStatus, String> {
    info!("[环境检查] 开始检查系统环境...");
    
    let os = platform::get_os();
    info!("[环境检查] 操作系统: {}", os);
    
    // 检查 Node.js
    info!("[环境检查] 检查 Node.js...");
    let node_version = get_node_version();
    let node_installed = node_version.is_some();
    let node_version_ok = check_node_version_requirement(&node_version);
    info!("[环境检查] Node.js: installed={}, version={:?}, version_ok={}", 
        node_installed, node_version, node_version_ok);
    
    // 检查 OpenClaw
    info!("[环境检查] 检查 OpenClaw...");
    let openclaw_version = get_openclaw_version();
    let openclaw_installed = openclaw_version.is_some();
    info!("[环境检查] OpenClaw: installed={}, version={:?}", 
        openclaw_installed, openclaw_version);
    
    // 检查配置目录
    let config_dir = platform::get_config_dir();
    let config_dir_exists = std::path::Path::new(&config_dir).exists();
    info!("[环境检查] 配置目录: {}, exists={}", config_dir, config_dir_exists);
    
    let ready = node_installed && node_version_ok && openclaw_installed;
    info!("[环境检查] 环境就绪状态: ready={}", ready);
    
    Ok(EnvironmentStatus {
        node_installed,
        node_version,
        node_version_ok,
        openclaw_installed,
        openclaw_version,
        config_dir_exists,
        ready,
        os,
    })
}

/// 获取 Node.js 版本
/// 检测多个可能的安装路径，因为 GUI 应用不继承用户 shell 的 PATH
fn get_node_version() -> Option<String> {
    if platform::is_windows() {
        // Windows: 先尝试直接调用（如果 PATH 已更新）
        if let Ok(v) = shell::run_cmd_output("node --version") {
            let version = v.trim().to_string();
            if !version.is_empty() && version.starts_with('v') {
                info!("[环境检查] 通过 PATH 找到 Node.js: {}", version);
                return Some(version);
            }
        }
        
        // Windows: 检查常见的安装路径
        let possible_paths = get_windows_node_paths();
        for path in possible_paths {
            if std::path::Path::new(&path).exists() {
                // 使用完整路径执行
                let cmd = format!("\"{}\" --version", path);
                if let Ok(output) = shell::run_cmd_output(&cmd) {
                    let version = output.trim().to_string();
                    if !version.is_empty() && version.starts_with('v') {
                        info!("[环境检查] 在 {} 找到 Node.js: {}", path, version);
                        return Some(version);
                    }
                }
            }
        }
        
        None
    } else {
        // 先尝试直接调用
        if let Ok(v) = shell::run_command_output("node", &["--version"]) {
            return Some(v.trim().to_string());
        }
        
        // 检测常见的 Node.js 安装路径（macOS/Linux）
        let possible_paths = get_unix_node_paths();
        for path in possible_paths {
            if std::path::Path::new(&path).exists() {
                if let Ok(output) = shell::run_command_output(&path, &["--version"]) {
                    info!("[环境检查] 在 {} 找到 Node.js: {}", path, output.trim());
                    return Some(output.trim().to_string());
                }
            }
        }
        
        // 尝试通过 shell 加载用户环境来检测
        if let Ok(output) = shell::run_bash_output("source ~/.zshrc 2>/dev/null || source ~/.bashrc 2>/dev/null; node --version 2>/dev/null") {
            if !output.is_empty() && output.starts_with('v') {
                info!("[环境检查] 通过用户 shell 找到 Node.js: {}", output.trim());
                return Some(output.trim().to_string());
            }
        }
        
        None
    }
}

/// 获取 Unix 系统上可能的 Node.js 路径
fn get_unix_node_paths() -> Vec<String> {
    let mut paths = Vec::new();
    
    // Homebrew (macOS)
    paths.push("/opt/homebrew/bin/node".to_string()); // Apple Silicon
    paths.push("/usr/local/bin/node".to_string());     // Intel Mac
    
    // 系统安装
    paths.push("/usr/bin/node".to_string());
    
    // nvm (检查常见版本)
    if let Some(home) = dirs::home_dir() {
        let home_str = home.display().to_string();
        
        // nvm 默认版本
        paths.push(format!("{}/.nvm/versions/node/v22.0.0/bin/node", home_str));
        paths.push(format!("{}/.nvm/versions/node/v22.1.0/bin/node", home_str));
        paths.push(format!("{}/.nvm/versions/node/v22.2.0/bin/node", home_str));
        paths.push(format!("{}/.nvm/versions/node/v22.11.0/bin/node", home_str));
        paths.push(format!("{}/.nvm/versions/node/v22.12.0/bin/node", home_str));
        paths.push(format!("{}/.nvm/versions/node/v23.0.0/bin/node", home_str));
        
        // 尝试 nvm alias default（读取 nvm 的 default alias）
        let nvm_default = format!("{}/.nvm/alias/default", home_str);
        if let Ok(version) = std::fs::read_to_string(&nvm_default) {
            let version = version.trim();
            if !version.is_empty() {
                paths.insert(0, format!("{}/.nvm/versions/node/v{}/bin/node", home_str, version));
            }
        }
        
        // fnm
        paths.push(format!("{}/.fnm/aliases/default/bin/node", home_str));
        
        // volta
        paths.push(format!("{}/.volta/bin/node", home_str));
        
        // asdf
        paths.push(format!("{}/.asdf/shims/node", home_str));
        
        // mise (formerly rtx)
        paths.push(format!("{}/.local/share/mise/shims/node", home_str));
    }
    
    paths
}

/// 获取 Windows 系统上可能的 Node.js 路径
fn get_windows_node_paths() -> Vec<String> {
    let mut paths = Vec::new();
    
    // 1. 标准安装路径 (Program Files)
    paths.push("C:\\Program Files\\nodejs\\node.exe".to_string());
    paths.push("C:\\Program Files (x86)\\nodejs\\node.exe".to_string());
    
    // 2. nvm for Windows (nvm4w) - 常见安装位置
    paths.push("C:\\nvm4w\\nodejs\\node.exe".to_string());
    
    // 3. 用户目录下的各种安装
    if let Some(home) = dirs::home_dir() {
        let home_str = home.display().to_string();
        
        // nvm for Windows 用户安装
        paths.push(format!("{}\\AppData\\Roaming\\nvm\\current\\node.exe", home_str));
        
        // fnm (Fast Node Manager) for Windows
        paths.push(format!("{}\\AppData\\Roaming\\fnm\\aliases\\default\\node.exe", home_str));
        paths.push(format!("{}\\AppData\\Local\\fnm\\aliases\\default\\node.exe", home_str));
        paths.push(format!("{}\\.fnm\\aliases\\default\\node.exe", home_str));
        
        // volta
        paths.push(format!("{}\\AppData\\Local\\Volta\\bin\\node.exe", home_str));
        // volta 通过 shim 调用，检查 bin 目录即可
        
        // scoop 安装
        paths.push(format!("{}\\scoop\\apps\\nodejs\\current\\node.exe", home_str));
        paths.push(format!("{}\\scoop\\apps\\nodejs-lts\\current\\node.exe", home_str));
        
        // chocolatey 安装
        paths.push("C:\\ProgramData\\chocolatey\\lib\\nodejs\\tools\\node.exe".to_string());
    }
    
    // 4. 从注册表读取的安装路径（通过环境变量间接获取）
    if let Ok(program_files) = std::env::var("ProgramFiles") {
        paths.push(format!("{}\\nodejs\\node.exe", program_files));
    }
    if let Ok(program_files_x86) = std::env::var("ProgramFiles(x86)") {
        paths.push(format!("{}\\nodejs\\node.exe", program_files_x86));
    }
    
    // 5. nvm-windows 的符号链接路径（NVM_SYMLINK 环境变量）
    if let Ok(nvm_symlink) = std::env::var("NVM_SYMLINK") {
        paths.insert(0, format!("{}\\node.exe", nvm_symlink));
    }
    
    // 6. nvm-windows 的 NVM_HOME 路径下的当前版本
    if let Ok(nvm_home) = std::env::var("NVM_HOME") {
        // 尝试读取当前激活的版本
        let settings_path = format!("{}\\settings.txt", nvm_home);
        if let Ok(content) = std::fs::read_to_string(&settings_path) {
            for line in content.lines() {
                if line.starts_with("current:") {
                    if let Some(version) = line.strip_prefix("current:") {
                        let version = version.trim();
                        if !version.is_empty() {
                            paths.insert(0, format!("{}\\v{}\\node.exe", nvm_home, version));
                        }
                    }
                }
            }
        }
    }
    
    paths
}

/// 获取 OpenClaw 版本
fn get_openclaw_version() -> Option<String> {
    // 使用 run_openclaw 统一处理各平台
    shell::run_openclaw(&["--version"])
        .ok()
        .map(|v| v.trim().to_string())
}

/// 检查 Node.js 版本是否 >= 22
fn check_node_version_requirement(version: &Option<String>) -> bool {
    if let Some(v) = version {
        // 解析版本号 "v22.1.0" -> 22
        let major = v.trim_start_matches('v')
            .split('.')
            .next()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);
        major >= 22
    } else {
        false
    }
}

/// 安装 Node.js
#[command]
pub async fn install_nodejs() -> Result<InstallResult, String> {
    info!("[安装Node.js] 开始安装 Node.js...");
    let os = platform::get_os();
    info!("[安装Node.js] 检测到操作系统: {}", os);
    
    let result = match os.as_str() {
        "windows" => {
            info!("[安装Node.js] 使用 Windows 安装方式...");
            install_nodejs_windows().await
        },
        "macos" => {
            info!("[安装Node.js] 使用 macOS 安装方式 (Homebrew)...");
            install_nodejs_macos().await
        },
        "linux" => {
            info!("[安装Node.js] 使用 Linux 安装方式...");
            install_nodejs_linux().await
        },
        _ => {
            error!("[安装Node.js] 不支持的操作系统: {}", os);
            Ok(InstallResult {
                success: false,
                message: "不支持的操作系统".to_string(),
                error: Some(format!("不支持的操作系统: {}", os)),
            })
        },
    };
    
    match &result {
        Ok(r) if r.success => info!("[安装Node.js] ✓ 安装成功"),
        Ok(r) => warn!("[安装Node.js] ✗ 安装失败: {}", r.message),
        Err(e) => error!("[安装Node.js] ✗ 安装错误: {}", e),
    }
    
    result
}

/// Windows 安装 Node.js
async fn install_nodejs_windows() -> Result<InstallResult, String> {
    info!("[安装Node.js] 开始 Windows Node.js 安装流程...");

    // 先检查是否已安装
    if let Ok(version) = shell::run_cmd_output("node --version") {
        info!("[安装Node.js] Node.js 已安装: {}", version);
        return Ok(InstallResult {
            success: true,
            message: format!("Node.js 已安装: {}", version),
            error: None,
        });
    }

    // 使用 cmd.exe 调用 winget（避免 PowerShell 执行策略问题）
    info!("[安装Node.js] 尝试使用 winget 安装...");

    // 先检查 winget 是否可用
    if shell::run_cmd_output("winget --version").is_ok() {
        info!("[安装Node.js] winget 可用，开始安装 Node.js LTS...");

        let install_cmd = "winget install --id OpenJS.NodeJS.LTS --accept-source-agreements --accept-package-agreements";
        match shell::run_cmd_output(install_cmd) {
            Ok(output) => {
                info!("[安装Node.js] winget 输出: {}", output);

                // 等待安装完成
                std::thread::sleep(std::time::Duration::from_secs(2));

                // 验证安装
                if get_node_version().is_some() {
                    return Ok(InstallResult {
                        success: true,
                        message: "Node.js 安装成功！请重启应用以使环境变量生效。".to_string(),
                        error: None,
                    });
                } else {
                    return Ok(InstallResult {
                        success: false,
                        message: "安装完成，但需要重启应用以使环境变量生效".to_string(),
                        error: Some("请重启应用".to_string()),
                    });
                }
            }
            Err(e) => {
                warn!("[安装Node.js] winget 安装失败: {}", e);
                // 继续尝试其他方法
            }
        }
    } else {
        warn!("[安装Node.js] winget 不可用");
    }

    // winget 失败，返回错误并建议手动安装
    Ok(InstallResult {
        success: false,
        message: "自动安装失败，请手动安装 Node.js".to_string(),
        error: Some("建议从 https://nodejs.org 下载安装".to_string()),
    })
}

/// macOS 安装 Node.js
async fn install_nodejs_macos() -> Result<InstallResult, String> {
    // 使用 Homebrew 安装
    let script = r#"
# 检查 Homebrew
if ! command -v brew &> /dev/null; then
    echo "安装 Homebrew..."
    /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
    
    # 配置 PATH
    if [[ -f /opt/homebrew/bin/brew ]]; then
        eval "$(/opt/homebrew/bin/brew shellenv)"
    elif [[ -f /usr/local/bin/brew ]]; then
        eval "$(/usr/local/bin/brew shellenv)"
    fi
fi

echo "安装 Node.js 22..."
brew install node@22
brew link --overwrite node@22

# 验证安装
node --version
"#;
    
    match shell::run_bash_output(script) {
        Ok(output) => Ok(InstallResult {
            success: true,
            message: format!("Node.js 安装成功！{}", output),
            error: None,
        }),
        Err(e) => Ok(InstallResult {
            success: false,
            message: "Node.js 安装失败".to_string(),
            error: Some(e),
        }),
    }
}

/// Linux 安装 Node.js
async fn install_nodejs_linux() -> Result<InstallResult, String> {
    // 使用 NodeSource 仓库安装
    let script = r#"
# 检测包管理器
if command -v apt-get &> /dev/null; then
    echo "检测到 apt，使用 NodeSource 仓库..."
    curl -fsSL https://deb.nodesource.com/setup_22.x | sudo -E bash -
    sudo apt-get install -y nodejs
elif command -v dnf &> /dev/null; then
    echo "检测到 dnf，使用 NodeSource 仓库..."
    curl -fsSL https://rpm.nodesource.com/setup_22.x | sudo bash -
    sudo dnf install -y nodejs
elif command -v yum &> /dev/null; then
    echo "检测到 yum，使用 NodeSource 仓库..."
    curl -fsSL https://rpm.nodesource.com/setup_22.x | sudo bash -
    sudo yum install -y nodejs
elif command -v pacman &> /dev/null; then
    echo "检测到 pacman..."
    sudo pacman -S nodejs npm --noconfirm
else
    echo "无法检测到支持的包管理器"
    exit 1
fi

# 验证安装
node --version
"#;
    
    match shell::run_bash_output(script) {
        Ok(output) => Ok(InstallResult {
            success: true,
            message: format!("Node.js 安装成功！{}", output),
            error: None,
        }),
        Err(e) => Ok(InstallResult {
            success: false,
            message: "Node.js 安装失败".to_string(),
            error: Some(e),
        }),
    }
}

/// 安装 OpenClaw
#[command]
pub async fn install_openclaw() -> Result<InstallResult, String> {
    info!("[安装OpenClaw] 开始安装 OpenClaw...");
    let os = platform::get_os();
    info!("[安装OpenClaw] 检测到操作系统: {}", os);
    
    let result = match os.as_str() {
        "windows" => {
            info!("[安装OpenClaw] 使用 Windows 安装方式...");
            install_openclaw_windows().await
        },
        _ => {
            info!("[安装OpenClaw] 使用 Unix 安装方式 (npm)...");
            install_openclaw_unix().await
        },
    };
    
    match &result {
        Ok(r) if r.success => info!("[安装OpenClaw] ✓ 安装成功"),
        Ok(r) => warn!("[安装OpenClaw] ✗ 安装失败: {}", r.message),
        Err(e) => error!("[安装OpenClaw] ✗ 安装错误: {}", e),
    }
    
    result
}

/// 检查 Visual Studio Build Tools 是否已安装
fn check_build_tools_installed() -> bool {
    info!("[依赖检查] 检查 Visual Studio Build Tools...");

    // 方法1: 检查 vswhere.exe（Visual Studio 安装器）
    let vswhere_check = shell::run_cmd_output(
        r#""C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe" -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath"#
    );

    if vswhere_check.is_ok() && !vswhere_check.unwrap().trim().is_empty() {
        info!("[依赖检查] ✓ 通过 vswhere 检测到 Build Tools");
        return true;
    }

    // 方法2: 检查常见的 MSBuild 路径
    let msbuild_paths = vec![
        r"C:\Program Files\Microsoft Visual Studio\2022\BuildTools\MSBuild\Current\Bin\MSBuild.exe",
        r"C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\MSBuild\Current\Bin\MSBuild.exe",
        r"C:\Program Files\Microsoft Visual Studio\2022\Community\MSBuild\Current\Bin\MSBuild.exe",
    ];

    for path in msbuild_paths {
        if std::path::Path::new(path).exists() {
            info!("[依赖检查] ✓ 在 {} 找到 MSBuild", path);
            return true;
        }
    }

    // 方法3: 尝试运行 cl.exe（C++ 编译器）
    if shell::run_cmd_output("cl.exe").is_ok() {
        info!("[依赖检查] ✓ cl.exe 可用");
        return true;
    }

    info!("[依赖检查] ✗ 未检测到 Build Tools");
    false
}

/// 安装 Visual Studio Build Tools
async fn install_build_tools() -> Result<InstallResult, String> {
    info!("[安装依赖] 开始安装 Visual Studio Build Tools...");

    // 使用 winget 安装 Build Tools
    let install_cmd = "winget install --id Microsoft.VisualStudio.2022.BuildTools --override \"--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended\"";

    info!("[安装依赖] 执行命令: {}", install_cmd);
    info!("[安装依赖] 注意：Build Tools 安装可能需要几分钟，请耐心等待...");

    match shell::run_cmd_output(install_cmd) {
        Ok(output) => {
            info!("[安装依赖] winget 输出: {}", output);

            // 等待安装完成
            std::thread::sleep(std::time::Duration::from_secs(3));

            // 验证安装
            if check_build_tools_installed() {
                Ok(InstallResult {
                    success: true,
                    message: "Visual Studio Build Tools 安装成功！".to_string(),
                    error: None,
                })
            } else {
                Ok(InstallResult {
                    success: false,
                    message: "Build Tools 安装完成，但可能需要重启系统".to_string(),
                    error: Some("请重启系统后重试".to_string()),
                })
            }
        }
        Err(e) => {
            warn!("[安装依赖] Build Tools 安装失败: {}", e);
            Ok(InstallResult {
                success: false,
                message: "Build Tools 安装失败".to_string(),
                error: Some(format!("建议手动安装: {}", e)),
            })
        }
    }
}

/// Windows 安装 OpenClaw
async fn install_openclaw_windows() -> Result<InstallResult, String> {
    // 使用 cmd.exe 而不是 PowerShell，避免执行策略问题
    // Windows 上 npm 同时提供 npm.cmd 和 npm.ps1，cmd 可以直接调用 npm.cmd
    info!("[安装OpenClaw] 使用 cmd.exe 执行 npm install...");

    // 先检查 Node.js
    let node_check = shell::run_cmd_output("node --version");
    if node_check.is_err() {
        warn!("[安装OpenClaw] Node.js 未安装或不在 PATH 中");
        return Ok(InstallResult {
            success: false,
            message: "请先安装 Node.js".to_string(),
            error: Some("Node.js 未找到".to_string()),
        });
    }

    info!("[安装OpenClaw] Node.js 已安装: {:?}", node_check);

    // 检查并安装 Build Tools（用于编译 native 模块）
    info!("[安装OpenClaw] 检查 Visual Studio Build Tools...");
    if !check_build_tools_installed() {
        warn!("[安装OpenClaw] 未检测到 Build Tools，开始安装...");

        let build_tools_result = install_build_tools().await?;
        if !build_tools_result.success {
            // Build Tools 安装失败，但可以尝试继续安装 OpenClaw（跳过可选依赖）
            warn!("[安装OpenClaw] Build Tools 安装失败，将跳过可选依赖");
            info!("[安装OpenClaw] 使用 --omit=optional 跳过 node-llama-cpp 等依赖");

            let install_cmd = "npm install -g openclaw@latest --omit=optional";
            info!("[安装OpenClaw] 执行命令: {}", install_cmd);

            match shell::run_cmd_output(install_cmd) {
                Ok(output) => {
                    info!("[安装OpenClaw] npm 输出: {}", output);
                    std::thread::sleep(std::time::Duration::from_millis(500));

                    if get_openclaw_version().is_some() {
                        return Ok(InstallResult {
                            success: true,
                            message: "OpenClaw 安装成功！（已跳过本地 LLM 支持）".to_string(),
                            error: Some("本地 LLM 功能不可用，但 API 模式正常工作".to_string()),
                        });
                    }
                }
                Err(e) => {
                    return Ok(InstallResult {
                        success: false,
                        message: "OpenClaw 安装失败".to_string(),
                        error: Some(e),
                    });
                }
            }
        } else {
            info!("[安装OpenClaw] ✓ Build Tools 已安装");
        }
    } else {
        info!("[安装OpenClaw] ✓ Build Tools 已存在");
    }

    // 使用 cmd.exe 执行 npm install（避免 PowerShell 执行策略问题）
    let install_cmd = "npm install -g openclaw@latest";
    info!("[安装OpenClaw] 执行命令: {}", install_cmd);

    match shell::run_cmd_output(install_cmd) {
        Ok(output) => {
            info!("[安装OpenClaw] npm 输出: {}", output);

            // 等待一下让文件系统同步
            std::thread::sleep(std::time::Duration::from_millis(500));

            // 验证安装
            if get_openclaw_version().is_some() {
                Ok(InstallResult {
                    success: true,
                    message: "OpenClaw 安装成功！".to_string(),
                    error: None,
                })
            } else {
                Ok(InstallResult {
                    success: false,
                    message: "安装后需要重启应用".to_string(),
                    error: Some(output),
                })
            }
        }
        Err(e) => {
            warn!("[安装OpenClaw] npm install 失败: {}", e);
            Ok(InstallResult {
                success: false,
                message: "OpenClaw 安装失败".to_string(),
                error: Some(e),
            })
        }
    }
}

/// Unix 系统安装 OpenClaw
async fn install_openclaw_unix() -> Result<InstallResult, String> {
    let script = r#"
# 检查 Node.js
if ! command -v node &> /dev/null; then
    echo "错误：请先安装 Node.js"
    exit 1
fi

echo "使用 npm 安装 OpenClaw..."
npm install -g openclaw@latest --unsafe-perm

# 验证安装
openclaw --version
"#;
    
    match shell::run_bash_output(script) {
        Ok(output) => Ok(InstallResult {
            success: true,
            message: format!("OpenClaw 安装成功！{}", output),
            error: None,
        }),
        Err(e) => Ok(InstallResult {
            success: false,
            message: "OpenClaw 安装失败".to_string(),
            error: Some(e),
        }),
    }
}

/// 初始化 OpenClaw 配置
#[command]
pub async fn init_openclaw_config() -> Result<InstallResult, String> {
    info!("[初始化配置] 开始初始化 OpenClaw 配置...");
    
    let config_dir = platform::get_config_dir();
    info!("[初始化配置] 配置目录: {}", config_dir);
    
    // 创建配置目录
    info!("[初始化配置] 创建配置目录...");
    if let Err(e) = std::fs::create_dir_all(&config_dir) {
        error!("[初始化配置] ✗ 创建配置目录失败: {}", e);
        return Ok(InstallResult {
            success: false,
            message: "创建配置目录失败".to_string(),
            error: Some(e.to_string()),
        });
    }
    
    // 创建子目录
    let subdirs = ["agents/main/sessions", "agents/main/agent", "credentials"];
    for subdir in subdirs {
        let path = format!("{}/{}", config_dir, subdir);
        info!("[初始化配置] 创建子目录: {}", subdir);
        if let Err(e) = std::fs::create_dir_all(&path) {
            error!("[初始化配置] ✗ 创建目录失败: {} - {}", subdir, e);
            return Ok(InstallResult {
                success: false,
                message: format!("创建目录失败: {}", subdir),
                error: Some(e.to_string()),
            });
        }
    }
    
    // 设置配置目录权限为 700（与 shell 脚本 chmod 700 一致）
    // 仅在 Unix 系统上执行
    #[cfg(unix)]
    {
        info!("[初始化配置] 设置目录权限为 700...");
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = std::fs::metadata(&config_dir) {
            let mut perms = metadata.permissions();
            perms.set_mode(0o700);
            if let Err(e) = std::fs::set_permissions(&config_dir, perms) {
                warn!("[初始化配置] 设置权限失败: {}", e);
            } else {
                info!("[初始化配置] ✓ 权限设置成功");
            }
        }
    }
    
    // 设置 gateway mode 为 local
    info!("[初始化配置] 执行: openclaw config set gateway.mode local");
    let result = shell::run_openclaw(&["config", "set", "gateway.mode", "local"]);
    
    match result {
        Ok(output) => {
            info!("[初始化配置] ✓ 配置初始化成功");
            debug!("[初始化配置] 命令输出: {}", output);
            Ok(InstallResult {
                success: true,
                message: "配置初始化成功！".to_string(),
                error: None,
            })
        },
        Err(e) => {
            error!("[初始化配置] ✗ 配置初始化失败: {}", e);
            Ok(InstallResult {
                success: false,
                message: "配置初始化失败".to_string(),
                error: Some(e),
            })
        },
    }
}

/// 打开终端执行安装脚本（用于需要管理员权限的场景）
#[command]
pub async fn open_install_terminal(install_type: String) -> Result<String, String> {
    match install_type.as_str() {
        "nodejs" => open_nodejs_install_terminal().await,
        "openclaw" => open_openclaw_install_terminal().await,
        _ => Err(format!("未知的安装类型: {}", install_type)),
    }
}

/// 打开终端安装 Node.js
async fn open_nodejs_install_terminal() -> Result<String, String> {
    if platform::is_windows() {
        // Windows: 打开 PowerShell 执行安装
        let script = r#"
Start-Process powershell -ArgumentList '-NoExit', '-Command', '
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "    Node.js 安装向导" -ForegroundColor White
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# 检查 winget
$hasWinget = Get-Command winget -ErrorAction SilentlyContinue
if ($hasWinget) {
    Write-Host "正在使用 winget 安装 Node.js 22..." -ForegroundColor Yellow
    winget install --id OpenJS.NodeJS.LTS --accept-source-agreements --accept-package-agreements
} else {
    Write-Host "请从以下地址下载安装 Node.js:" -ForegroundColor Yellow
    Write-Host "https://nodejs.org/en/download" -ForegroundColor Green
    Write-Host ""
    Start-Process "https://nodejs.org/en/download"
}

Write-Host ""
Write-Host "安装完成后请重启 OpenClaw Manager" -ForegroundColor Green
Write-Host ""
Read-Host "按回车键关闭此窗口"
' -Verb RunAs
"#;
        shell::run_powershell_output(script)?;
        Ok("已打开安装终端".to_string())
    } else if platform::is_macos() {
        // macOS: 打开 Terminal.app
        let script_content = r#"#!/bin/bash
clear
echo "========================================"
echo "    Node.js 安装向导"
echo "========================================"
echo ""

# 检查 Homebrew
if ! command -v brew &> /dev/null; then
    echo "正在安装 Homebrew..."
    /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
    
    if [[ -f /opt/homebrew/bin/brew ]]; then
        eval "$(/opt/homebrew/bin/brew shellenv)"
    elif [[ -f /usr/local/bin/brew ]]; then
        eval "$(/usr/local/bin/brew shellenv)"
    fi
fi

echo "正在安装 Node.js 22..."
brew install node@22
brew link --overwrite node@22

echo ""
echo "安装完成！"
node --version
echo ""
read -p "按回车键关闭此窗口..."
"#;
        
        let script_path = "/tmp/openclaw_install_nodejs.command";
        std::fs::write(script_path, script_content)
            .map_err(|e| format!("创建脚本失败: {}", e))?;
        
        std::process::Command::new("chmod")
            .args(["+x", script_path])
            .output()
            .map_err(|e| format!("设置权限失败: {}", e))?;
        
        std::process::Command::new("open")
            .arg(script_path)
            .spawn()
            .map_err(|e| format!("启动终端失败: {}", e))?;
        
        Ok("已打开安装终端".to_string())
    } else {
        Err("请手动安装 Node.js: https://nodejs.org/".to_string())
    }
}

/// 打开终端安装 OpenClaw
async fn open_openclaw_install_terminal() -> Result<String, String> {
    if platform::is_windows() {
        let script = r#"
Start-Process powershell -ArgumentList '-NoExit', '-Command', '
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "    OpenClaw 安装向导" -ForegroundColor White
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

Write-Host "正在安装 OpenClaw..." -ForegroundColor Yellow
npm install -g openclaw@latest

Write-Host ""
Write-Host "初始化配置..."
openclaw config set gateway.mode local

Write-Host ""
Write-Host "安装完成！" -ForegroundColor Green
openclaw --version
Write-Host ""
Read-Host "按回车键关闭此窗口"
'
"#;
        shell::run_powershell_output(script)?;
        Ok("已打开安装终端".to_string())
    } else if platform::is_macos() {
        let script_content = r#"#!/bin/bash
clear
echo "========================================"
echo "    OpenClaw 安装向导"
echo "========================================"
echo ""

echo "正在安装 OpenClaw..."
npm install -g openclaw@latest

echo ""
echo "初始化配置..."
openclaw config set gateway.mode local 2>/dev/null || true

mkdir -p ~/.openclaw/agents/main/sessions
mkdir -p ~/.openclaw/agents/main/agent
mkdir -p ~/.openclaw/credentials

echo ""
echo "安装完成！"
openclaw --version
echo ""
read -p "按回车键关闭此窗口..."
"#;
        
        let script_path = "/tmp/openclaw_install_openclaw.command";
        std::fs::write(script_path, script_content)
            .map_err(|e| format!("创建脚本失败: {}", e))?;
        
        std::process::Command::new("chmod")
            .args(["+x", script_path])
            .output()
            .map_err(|e| format!("设置权限失败: {}", e))?;
        
        std::process::Command::new("open")
            .arg(script_path)
            .spawn()
            .map_err(|e| format!("启动终端失败: {}", e))?;
        
        Ok("已打开安装终端".to_string())
    } else {
        // Linux
        let script_content = r#"#!/bin/bash
clear
echo "========================================"
echo "    OpenClaw 安装向导"
echo "========================================"
echo ""

echo "正在安装 OpenClaw..."
npm install -g openclaw@latest

echo ""
echo "初始化配置..."
openclaw config set gateway.mode local 2>/dev/null || true

mkdir -p ~/.openclaw/agents/main/sessions
mkdir -p ~/.openclaw/agents/main/agent
mkdir -p ~/.openclaw/credentials

echo ""
echo "安装完成！"
openclaw --version
echo ""
read -p "按回车键关闭..."
"#;
        
        let script_path = "/tmp/openclaw_install_openclaw.sh";
        std::fs::write(script_path, script_content)
            .map_err(|e| format!("创建脚本失败: {}", e))?;
        
        std::process::Command::new("chmod")
            .args(["+x", script_path])
            .output()
            .map_err(|e| format!("设置权限失败: {}", e))?;
        
        // 尝试不同的终端
        let terminals = ["gnome-terminal", "xfce4-terminal", "konsole", "xterm"];
        for term in terminals {
            if std::process::Command::new(term)
                .args(["--", script_path])
                .spawn()
                .is_ok()
            {
                return Ok("已打开安装终端".to_string());
            }
        }
        
        Err("无法启动终端，请手动运行: npm install -g openclaw".to_string())
    }
}

/// 卸载 OpenClaw
#[command]
pub async fn uninstall_openclaw() -> Result<InstallResult, String> {
    info!("[卸载OpenClaw] 开始卸载 OpenClaw...");
    let os = platform::get_os();
    info!("[卸载OpenClaw] 检测到操作系统: {}", os);
    
    // 先停止服务
    info!("[卸载OpenClaw] 尝试停止服务...");
    let _ = shell::run_openclaw(&["gateway", "stop"]);
    std::thread::sleep(std::time::Duration::from_millis(500));
    
    let result = match os.as_str() {
        "windows" => {
            info!("[卸载OpenClaw] 使用 Windows 卸载方式...");
            uninstall_openclaw_windows().await
        },
        _ => {
            info!("[卸载OpenClaw] 使用 Unix 卸载方式 (npm)...");
            uninstall_openclaw_unix().await
        },
    };
    
    match &result {
        Ok(r) if r.success => info!("[卸载OpenClaw] ✓ 卸载成功"),
        Ok(r) => warn!("[卸载OpenClaw] ✗ 卸载失败: {}", r.message),
        Err(e) => error!("[卸载OpenClaw] ✗ 卸载错误: {}", e),
    }
    
    result
}

/// Windows 卸载 OpenClaw
async fn uninstall_openclaw_windows() -> Result<InstallResult, String> {
    // 使用 cmd.exe 执行 npm uninstall，避免 PowerShell 执行策略问题
    info!("[卸载OpenClaw] 执行 npm uninstall -g openclaw...");
    
    match shell::run_cmd_output("npm uninstall -g openclaw") {
        Ok(output) => {
            info!("[卸载OpenClaw] npm 输出: {}", output);
            
            // 验证卸载是否成功
            std::thread::sleep(std::time::Duration::from_millis(500));
            if get_openclaw_version().is_none() {
                Ok(InstallResult {
                    success: true,
                    message: "OpenClaw 已成功卸载！".to_string(),
                    error: None,
                })
            } else {
                Ok(InstallResult {
                    success: false,
                    message: "卸载命令已执行，但 OpenClaw 仍然存在，请尝试手动卸载".to_string(),
                    error: Some(output),
                })
            }
        }
        Err(e) => {
            warn!("[卸载OpenClaw] npm uninstall 失败: {}", e);
            Ok(InstallResult {
                success: false,
                message: "OpenClaw 卸载失败".to_string(),
                error: Some(e),
            })
        }
    }
}

/// Unix 系统卸载 OpenClaw
async fn uninstall_openclaw_unix() -> Result<InstallResult, String> {
    let script = r#"
echo "卸载 OpenClaw..."
npm uninstall -g openclaw

# 验证卸载
if command -v openclaw &> /dev/null; then
    echo "警告：openclaw 命令仍然存在"
    exit 1
else
    echo "OpenClaw 已成功卸载"
    exit 0
fi
"#;
    
    match shell::run_bash_output(script) {
        Ok(output) => Ok(InstallResult {
            success: true,
            message: format!("OpenClaw 已成功卸载！{}", output),
            error: None,
        }),
        Err(e) => Ok(InstallResult {
            success: false,
            message: "OpenClaw 卸载失败".to_string(),
            error: Some(e),
        }),
    }
}

/// 版本更新信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    /// 是否有更新可用
    pub update_available: bool,
    /// 当前版本
    pub current_version: Option<String>,
    /// 最新版本
    pub latest_version: Option<String>,
    /// 错误信息
    pub error: Option<String>,
}

/// 检查 OpenClaw 更新
#[command]
pub async fn check_openclaw_update() -> Result<UpdateInfo, String> {
    info!("[版本检查] 开始检查 OpenClaw 更新...");
    
    // 获取当前版本
    let current_version = get_openclaw_version();
    info!("[版本检查] 当前版本: {:?}", current_version);
    
    if current_version.is_none() {
        info!("[版本检查] OpenClaw 未安装");
        return Ok(UpdateInfo {
            update_available: false,
            current_version: None,
            latest_version: None,
            error: Some("OpenClaw 未安装".to_string()),
        });
    }
    
    // 获取最新版本
    let latest_version = get_latest_openclaw_version();
    info!("[版本检查] 最新版本: {:?}", latest_version);
    
    if latest_version.is_none() {
        return Ok(UpdateInfo {
            update_available: false,
            current_version,
            latest_version: None,
            error: Some("无法获取最新版本信息".to_string()),
        });
    }
    
    // 比较版本
    let current = current_version.clone().unwrap();
    let latest = latest_version.clone().unwrap();
    let update_available = compare_versions(&current, &latest);
    
    info!("[版本检查] 是否有更新: {}", update_available);
    
    Ok(UpdateInfo {
        update_available,
        current_version,
        latest_version,
        error: None,
    })
}

/// 获取 npm registry 上的最新版本
fn get_latest_openclaw_version() -> Option<String> {
    // 使用 npm view 获取最新版本
    let result = if platform::is_windows() {
        shell::run_cmd_output("npm view openclaw version")
    } else {
        shell::run_bash_output("npm view openclaw version 2>/dev/null")
    };
    
    match result {
        Ok(version) => {
            let v = version.trim().to_string();
            if v.is_empty() {
                None
            } else {
                Some(v)
            }
        }
        Err(e) => {
            warn!("[版本检查] 获取最新版本失败: {}", e);
            None
        }
    }
}

/// 比较版本号，返回是否有更新可用
/// current: 当前版本 (如 "1.0.0" 或 "v1.0.0")
/// latest: 最新版本 (如 "1.0.1")
fn compare_versions(current: &str, latest: &str) -> bool {
    // 移除可能的 'v' 前缀和空白
    let current = current.trim().trim_start_matches('v');
    let latest = latest.trim().trim_start_matches('v');
    
    // 分割版本号
    let current_parts: Vec<u32> = current
        .split('.')
        .filter_map(|s| s.parse().ok())
        .collect();
    let latest_parts: Vec<u32> = latest
        .split('.')
        .filter_map(|s| s.parse().ok())
        .collect();
    
    // 比较每个部分
    for i in 0..3 {
        let c = current_parts.get(i).unwrap_or(&0);
        let l = latest_parts.get(i).unwrap_or(&0);
        if l > c {
            return true;
        } else if l < c {
            return false;
        }
    }
    
    false
}

/// 更新 OpenClaw
#[command]
pub async fn update_openclaw() -> Result<InstallResult, String> {
    info!("[更新OpenClaw] 开始更新 OpenClaw...");
    let os = platform::get_os();
    
    // 先停止服务
    info!("[更新OpenClaw] 尝试停止服务...");
    let _ = shell::run_openclaw(&["gateway", "stop"]);
    std::thread::sleep(std::time::Duration::from_millis(500));
    
    let result = match os.as_str() {
        "windows" => {
            info!("[更新OpenClaw] 使用 Windows 更新方式...");
            update_openclaw_windows().await
        },
        _ => {
            info!("[更新OpenClaw] 使用 Unix 更新方式 (npm)...");
            update_openclaw_unix().await
        },
    };
    
    match &result {
        Ok(r) if r.success => info!("[更新OpenClaw] ✓ 更新成功"),
        Ok(r) => warn!("[更新OpenClaw] ✗ 更新失败: {}", r.message),
        Err(e) => error!("[更新OpenClaw] ✗ 更新错误: {}", e),
    }
    
    result
}

/// Windows 更新 OpenClaw
async fn update_openclaw_windows() -> Result<InstallResult, String> {
    info!("[更新OpenClaw] 执行 npm install -g openclaw@latest...");
    
    match shell::run_cmd_output("npm install -g openclaw@latest") {
        Ok(output) => {
            info!("[更新OpenClaw] npm 输出: {}", output);
            
            // 获取新版本
            let new_version = get_openclaw_version();
            
            Ok(InstallResult {
                success: true,
                message: format!("OpenClaw 已更新到 {}", new_version.unwrap_or("最新版本".to_string())),
                error: None,
            })
        }
        Err(e) => {
            warn!("[更新OpenClaw] npm install 失败: {}", e);
            Ok(InstallResult {
                success: false,
                message: "OpenClaw 更新失败".to_string(),
                error: Some(e),
            })
        }
    }
}

/// Unix 系统更新 OpenClaw
async fn update_openclaw_unix() -> Result<InstallResult, String> {
    let script = r#"
echo "更新 OpenClaw..."
npm install -g openclaw@latest

# 验证更新
openclaw --version
"#;
    
    match shell::run_bash_output(script) {
        Ok(output) => Ok(InstallResult {
            success: true,
            message: format!("OpenClaw 已更新！{}", output),
            error: None,
        }),
        Err(e) => Ok(InstallResult {
            success: false,
            message: "OpenClaw 更新失败".to_string(),
            error: Some(e),
        }),
    }
}
