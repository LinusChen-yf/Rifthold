一、项目目标（整体描述）
做一个 跨平台的桌面窗口切换器应用：
支持 macOS（优先实现），未来扩展到 Windows / Linux, 由于是跨平台应用，所以尽可能的使用平台通用 api。
通过 全局快捷键 呼出一个 全屏窗口概览 UI，风格类似 Safari 的标签页 Overview：
显示当前系统所有窗口的「缩略图网格」。
顶部有搜索框，可以过滤窗口。
只用键盘就可以完成：呼出 → 搜索 → 选择 → 切换窗口。

二、技术栈与项目结构
桌面框架：Tauri
前端：React + TypeScript
UI 组件：shadcn/ui + Tailwind CSS
后端：Rust（Tauri 后端 + 调系统 API）

三、开发步骤规划

第 2 步：实现前端的「全屏 Overview UI」骨架
目标：先把 UI 长什么样子搭出来，先不管真实窗口数据，前端用假数据。
主窗口设为全屏显示（Tauri 窗口配置）：
无标题栏 / 或半透明背景。
居中 / 全屏、置顶。
前端页面布局：
顶部：一个搜索输入框（shadcn 的 Input）。
中间：一个 网格（grid）视图，展示一堆 窗口卡片：
使用 div + grid 或 shadcn 的 Card 来做「窗口缩略图」卡片。
每个卡片包含：
上方：暂时用纯色块 / 占位图代替缩略图。
下方：窗口标题、应用名文本。
使用一些基本样式：
grid grid-cols-... gap-...
hover / 选中态加 ring/shadow 等。
用假数据（例如 8~12 个窗口）喂给 UI，前端自己管理：

第 3 步：实现前端的键盘交互（只操作 UI）
先完全在前端里处理：
状态管理：
windows: 显示的窗口数组（初始为 mock 数据）。
query: 搜索框内容。
selectedIndex: 当前高亮卡片下标。
键盘逻辑：
输入框聚焦时：
输入文字 → 更改 query → 前端根据 title / appName 做过滤 → 更新 filteredWindows, 隐藏其他窗口。
方向键：
简化版：先实现 ↑ / ↓ 在列表里移动（线性移动）。
Enter：
输出一条 console.log：例如 activate window id=xx（之后接到后端）。
Esc/呼出快捷键：
调用 Tauri API 关闭 / 隐藏窗口（也可以先只是 console.log）。
打开应用时：
自动聚焦搜索框。
默认 selectedIndex = 0（高亮第一个）。

第 4 步：Tauri 后端命令（先用 mock）
先让前后端「能对话」，后端先不调用系统 API，用 mock 数据：
在 src-tauri/src/main.rs（或相应文件）里加两个命令：
#[derive(serde::Serialize)]
struct WindowInfo {
    id: String,
    title: String,
    app_name: String,
    // thumbnail 以后再加，可以先不管
}

#[tauri::command]
fn list_windows() -> Vec<WindowInfo> {
    vec![
        WindowInfo {
            id: "1".into(),
            title: "Mock Window 1".into(),
            app_name: "MockApp".into(),
        },
        // ...
    ]
}

#[tauri::command]
fn activate_window(id: String) -> Result<(), String> {
    println!("activate_window called with id={}", id);
    Ok(())
}
在 tauri::Builder 里注册：
tauri::Builder::default()
    .invoke_handler(tauri::generate_handler![list_windows, activate_window])
    ...
前端用 @tauri-apps/api 调用这些命令：
启动时调用 list_windows() 替换掉 mock。
按 Enter 时调用 activate_window(id)。

第 5 步：实现全局快捷键 + 显示/隐藏主窗口（macOS 优先）
目标：按一个全局快捷键（比如 Option+Space）呼出 / 隐藏应用。
在 Rust / Tauri 端注册全局快捷键（可用 Tauri 插件或自己写）：
例子（伪代码）：
// 监听全局快捷键，触发时：
// - 如果窗口隐藏 → 显示并聚焦
// - 如果窗口显示 → 隐藏
控制 Tauri 窗口：
使用 tauri::Window 的 show() / hide() / set_focus() 等 API。
打开时通知前端刷新一次 list_windows() 并聚焦搜索框。
前端按 Esc 时：
调用 Tauri 的 window.hide()（可以通过 @tauri-apps/api/window）。

第 6 步：接入真实系统 API（先从 macOS 开始）
这一部分主要是 Rust + 各平台 API 的事，可以晚一点做，先保证 UI 和流程都通了。
macOS list_windows()：
使用 CGWindow API / AppKit 获取当前所有可见窗口。
提取：
标题
所属 app 名称
窗口句柄 / ID
转换成 WindowInfo 传给前端。
macOS activate_window(id)：
根据 id 找到对应窗口 / app。
调用 AppKit / AppleScript / AX API 激活窗口。
缩略图（以后再做）：
可以用 CGWindowListCreateImage 拿窗口截图，转成 base64 或文件路径传给前端。
Windows / Linux 可以先不做，等 macOS 跑通之后，再各自写对应的实现。
