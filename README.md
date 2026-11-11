# UAD-RE · Volt Control

为 **Universal Audio Volt 2 Gen 2** 做的第三方控制软件：在软件里直接调监听音箱/耳机音量，并支持 MIDI 控制器。

## 目录

| 路径 | 内容 |
|------|------|
| `docs/protocol.md` | 控制协议调研（UAMixerHelper 的 UASrv 协议、参数树、实机验证结论） |
| `research/` | 调研脚本（协议探测、参数树导出、推送监听、截图/点击辅助） |
| `volt-control/` | 应用源码：Tauri 2 + Rust 后端，Svelte 5 + TS 前端 |

## 工作原理

`UAMixerHelper.exe`（随 UAD Console 安装，单实例，关掉 UAD Console 也常驻）在 `127.0.0.1:4720` 上提供设备参数树。开机时没有任何 UA 组件会可靠地拉起它（UA Connect 只在装完驱动后请求一次），所以 Volt Control 连不上时会自己启动它，不依赖 UA Connect。Volt Control 作为它的另一个客户端：读写参数、订阅变化，所以和 UAD Console、机身按键始终同步，互不冲突。

> 机身只有一个旋钮，按键切换它控制的对象。软件里的监听/耳机音量和机身旋钮是**同一个参数**，双向实时同步。
> 限制：机身屏幕当前显示哪一项（旋钮焦点）不对电脑开放，软件无法让屏幕切页；按键灯亮度也没有可调参数（只有屏幕亮度）。

## 功能

- **主控**：监听音量 / 静音 / 单声道 / Dim；耳机音量 / 静音；输入 1、2 的增益、+48V（二次确认）、Tone、压缩、反相、DIRECT（直接监听）、自动增益、插孔类型显示、立体声链接
- **混音**：监听混音与 CUE 1 / CUE 2 的推子、声像、静音、Solo、推子前发送，带实时电平表
- **MIDI**：右键任意控件 → 转动控制器即完成绑定；支持 7-bit / 14-bit CC（学习时自动识别）、绝对旋钮/推子、三种相对编码器、按键切换/按住/触发；音量类默认启用软接管（防跳变）；值变化时回发给控制器（LED 环 / 电动推子）
- **设置**：采样率、屏幕亮度、No-Clip、定位设备、电平表保持时间；**音量安全上限**（同时限制界面和 MIDI）
- 关窗缩到托盘继续响应 MIDI；登录 Windows 时自动在托盘启动；托盘菜单可一键静音监听/耳机；断线自动重连

## 构建与运行

```bash
cd volt-control
npm install
npx tauri dev      # 开发
npx tauri build    # 安装包：src-tauri/target/release/bundle/nsis/
```

测试：

```bash
cd volt-control/src-tauri
cargo test                                   # 单元测试
cargo test live_ -- --ignored --nocapture    # 连真机：只读发现 + 无害参数写入回滚
```

配置文件：`%APPDATA%\com.knia.voltcontrol\config.json`（MIDI 端口与映射、音量上限）。

## 许可证

[MIT](LICENSE)。本项目与 Universal Audio 无关联；Universal Audio、UA、Volt、UAD 是其商标。
