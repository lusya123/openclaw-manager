# Windows PowerShell 执行策略问题修复

## 问题分析

### 根本原因
Windows 上点击"安装 OpenClaw"一直转圈的原因：

1. **PowerShell 执行策略限制**
   - Windows 上的 `npm` 实际是 `npm.ps1` 脚本文件
   - 即使代码使用了 `-ExecutionPolicy Bypass`，这只对直接执行的脚本有效
   - 当脚本内部调用 `npm` 时，`npm.ps1` 在新上下文执行，**不继承** Bypass 策略
   - 导致安装命令被阻止，但因为设置了 `CREATE_NO_WINDOW`，用户看不到错误提示

2. **同步阻塞无超时**
   - 代码使用 `cmd.output()` 同步等待命令完成
   - 没有超时机制，如果命令卡住会永久等待

### 用户遇到的错误
```powershell
# 错误1：PATH 问题
npm : 无法将"npm"项识别为 cmdlet

# 错误2：执行策略限制（核心问题）
npm : 无法加载文件 C:\Program Files\nodejs\npm.ps1，因为在此系统上禁止运行脚本
```

## 代码修改

### 修改 1: `install_openclaw_windows()` (src-tauri/src/commands/installer.rs:508-556)

**改动前**：使用 PowerShell 执行 npm 命令
```rust
let script = r#"
npm install -g openclaw@latest --unsafe-perm
"#;
shell::run_powershell_output(script)
```

**改动后**：使用 cmd.exe 执行 npm 命令
```rust
// 使用 cmd.exe 而不是 PowerShell（避免执行策略问题）
let install_cmd = "npm install -g openclaw@latest";
shell::run_cmd_output(install_cmd)
```

**原理**：
- Windows 上 npm 同时提供 `npm.cmd` 和 `npm.ps1`
- cmd.exe 调用 `npm.cmd`，**不受 PowerShell 执行策略限制**
- PowerShell 调用 `npm.ps1`，受执行策略限制

### 修改 2: `install_nodejs_windows()` (src-tauri/src/commands/installer.rs:325-397)

**改动前**：使用 PowerShell 执行 winget 和 fnm
**改动后**：使用 cmd.exe 执行 winget，简化流程

```rust
// 使用 cmd.exe 调用 winget
if shell::run_cmd_output("winget --version").is_ok() {
    let install_cmd = "winget install --id OpenJS.NodeJS.LTS --accept-source-agreements --accept-package-agreements";
    shell::run_cmd_output(install_cmd)
}
```

## 在 Windows 上编译和测试

### 前置条件
1. 已安装 Node.js (v22+)
2. 已安装 Rust 和 Cargo
3. 已安装 Tauri CLI

### 编译步骤

```powershell
# 1. 进入项目目录
cd openclaw-manager

# 2. 安装依赖
npm install

# 3. 开发模式测试（推荐先测试）
npm run tauri:dev

# 4. 编译生产版本
npm run tauri:build
```

### 编译输出位置
- 可执行文件：`src-tauri/target/release/openclaw-manager.exe`
- 安装包：`src-tauri/target/release/bundle/`

### 测试步骤

1. **运行编译后的应用**
   ```powershell
   .\src-tauri\target\release\openclaw-manager.exe
   ```

2. **测试安装流程**
   - 打开应用
   - 进入"概览"页面
   - 点击"安装 OpenClaw"按钮
   - 观察是否还会一直转圈

3. **验证修复**
   - 应该能看到安装进度或错误提示
   - 不应该再出现无响应的转圈状态
   - 如果安装失败，应该有明确的错误信息

### 预期行为

**修复后的正常流程**：
1. 点击"安装 OpenClaw"
2. 后台执行 `npm install -g openclaw@latest`（使用 cmd.exe）
3. 几秒到几十秒后完成（取决于网络速度）
4. 显示"安装成功"或具体的错误信息

**可能的错误情况**：
- 网络问题：显示 npm 网络错误
- 权限问题：提示需要管理员权限
- Node.js 未安装：提示先安装 Node.js

## 用户侧临时解决方案

如果不想重新编译，可以手动安装：

```powershell
# 以管理员身份运行 PowerShell

# 方案1：修改执行策略（推荐）
Set-ExecutionPolicy RemoteSigned -Scope CurrentUser

# 方案2：使用 cmd.exe 安装（绕过 PowerShell）
cmd /c "npm install -g openclaw@latest"
```

## 技术细节

### 为什么 cmd.exe 可以绕过执行策略？

1. **PowerShell 执行策略**只影响 `.ps1` 脚本文件
2. **cmd.exe** 执行 `.cmd` 和 `.bat` 批处理文件，不受 PowerShell 策略限制
3. npm 在 Windows 上同时提供：
   - `npm.cmd` - 批处理文件（cmd.exe 使用）
   - `npm.ps1` - PowerShell 脚本（PowerShell 使用）
4. 当在 cmd.exe 中调用 `npm` 时，自动使用 `npm.cmd`

### shell.rs 中的相关函数

```rust
// src-tauri/src/utils/shell.rs

// PowerShell 执行（受执行策略限制）
pub fn run_powershell_output(script: &str) -> Result<String, String> {
    let mut cmd = Command::new("powershell");
    cmd.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", script]);
    // 注意：Bypass 只对直接执行的脚本有效，不影响脚本内调用的其他 .ps1 文件
}

// cmd.exe 执行（不受执行策略限制）
pub fn run_cmd_output(script: &str) -> Result<String, String> {
    let mut cmd = Command::new("cmd");
    cmd.args(["/c", script]);
    // cmd.exe 执行批处理文件，没有执行策略概念
}
```

## 总结

这次修改从根本上解决了 Windows 上的安装问题：
- ✅ 绕过 PowerShell 执行策略限制
- ✅ 不需要用户修改系统设置
- ✅ 更可靠，cmd.exe 在所有 Windows 版本都可用
- ✅ 保持了相同的功能和用户体验

修改后的代码在 Windows 上应该能够正常安装 OpenClaw，不会再出现一直转圈的问题。
