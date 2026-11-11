# Volt 2 Gen 2 控制协议调研（UASrv / UAMixerHelper）

> 调研日期：2026-11-02 · 设备：Volt 2 Gen 2（USB VID_2B5A PID_002D，序列号 12345678901234，已替换为示例值）
> UAD Console / UAMixerHelper 版本：2026-10-19 构建，plugin_sdk 5.4.9

## 1. 软件架构

```
UAD Console.exe (GUI, LUNA/Rack.dll)   本项目 Volt Control
            \                          /
             TCP 127.0.0.1:4720  (UASrv JSON 协议)
                        |
             UAMixerHelper.exe   <- 单实例；UAD Console / Volt Control 按需拉起，之后常驻
                                    （UA Connect 只在装完驱动后经 UAHelperService 的
                                     launch_mixer_server 拉起一次，平时不管它）
                        |
          hid_write / Thesycon TUSBAUDIO 驱动 (UniversalAudioUsbAudio*.sys)
                        |
                 Volt 2 Gen 2（HID 接口 MI_04，厂商自定义）
```

- UAMixerHelper 监听 `127.0.0.1:4720`（参数服务）和 `127.0.0.1:4748`（另一服务，`get /devices` 返回 invalid_object）。
- 设备 UID = 序列号 ASCII 的十六进制，右补 `00` 至 32 字符：`12345678901234` → `31323334353637383930313233340000`。
- `/devices` 下还有一个虚拟设备 `46bfabe9...`（工作区/会话用），按 `device_type` 以 `volt` 开头来识别真机。

## 2. 帧格式

- 纯 TCP，**每条消息以 `\0` 结尾**，UTF-8。
- 请求是文本：`<verb> <path>[ <参数>]`；应答是 JSON：`{"path": ..., "data": ...}` 或 `{"path": ..., "error": "uasrv.xxx"}`。
- **客户端约 75 s 不发任何数据会被服务端强制断开**（RST，`WinError 10054`；即使订阅推送一直在流动）。早先 40 s 的实验没覆盖到这一点。每 20 s 发一次 `get /ping`（→ `{"path":"/ping","data":true}`）可保持 150 s 以上仍在线（`research/keepalive2.py`）。Volt Control 用 20 s 心跳，5 s 无应答即重连。
- 非 UTF-8 字节：`string_value` 里的 `-∞` 以非 UTF-8 编码发送（显示为 `-��`），前端应按数值自行格式化。

## 3. 动词

| 动词 | 语法 | 说明 |
|------|------|------|
| `get` | `get /devices/<uid>` | 返回 `{properties, children, commands}`；`children` 只有相对路径名，需递归 |
| `set` | `set <对象路径>/<属性> <值>` 或 `set <对象路径>/<属性>/value <值>` | 值为 JSON 字面量（数字/布尔/字符串）。⚠ `set <p> {"value":22}` 会被解析成 0，**不要传对象** |
| `post` | `post <对象路径>/<命令名> <JSON参数>` | 调用 `commands` 中列出的命令，返回 `data` |
| `subscribe` / `unsubscribe` | `subscribe <路径>` | 首包 `data:null` 表示订阅成功；之后推送变化 |

已知错误码：`uasrv.invalid_object` / `invalid_property` / `invalid_value` / `invalid_type` / `read_only` / `too_many_requests` / `not_implemented`；`ntwk.parse_error`（无法解析的帧）。

## 4. `multi` 推送包（批量变化 / 电平表）

```json
{"path":"multi","data":{
  "tag": "meters",                       // 仅电平表包有
  "data":   [100.0, 1.0, "+100.0"],      // 第 j 个叶子的值
  "string": ["devices","3236...","buses","a7fe...","outputs","main","controls","gain","real_value","value","normalized_value","string_value"],
  "name":   [0,1,2,3,4,5,6,7,8,9,10,9,11,9],   // 节点 i 的名字 = string[name[i]]
  "parent": [0,0,1,2,3,4,5,6,7,8,7,10,7,12],   // 节点 i 的父节点；parent[i]==i 表示顶层
  "path":   [9,11,13]                          // data[j] 对应的叶子节点下标
}}
```

解码：从叶子沿 `parent` 回溯到自指节点，拼出 `/devices/<uid>/.../gain/real_value/value`。末段 `value` 是"属性的值"，去掉即得属性路径。电平表包 `data[j]` 是数组（每声道一个值，线性幅度）。

## 5. Volt 2 Gen 2 参数树（关键部分）

设备节点 `/devices/<uid>`：`name, device_type="volt_2_gen2", sample_rate(44.1k–192k), display_brightness, no_clip, dim_on, talkback_on, preset_name, firmware_version, clock_locked …`

`/devices/<uid>/buses/<bus_uid>`，按 `bus_type` 识别：

| bus_type | name | 说明 |
|----------|------|------|
| `input` | ANALOG 1 / ANALOG 2 / ANALOG 1-2 | 1-2 为立体声链接后的总线（`enabled`=链接状态） |
| `loopback` | LOOPBACK / LOOPBACK L / R | |
| `talkback` | TALKBACK | |
| `master` | MONITOR | 监听输出总线，`mix_in_source="mon"` |
| `headphone` | HEADPHONE | 耳机总线 |
| `cue_0` / `cue_1` | CUE 1 / CUE 2 | |

控件节点（`.../controls/<name>`）属性：`value, real_value, normalized_value, string_value, discrete, num_steps, string_values, real_values`。写 `value`（真实值）或 `normalized_value`（0–1）均可。

| 控件 | 路径 | 取值 |
|------|------|------|
| 监听音量 | `buses/<MONITOR>/outputs/main/controls/gain` | 0–100 线性，与机身旋钮（监听焦点）同一参数（2026-11-02 实测为 0） |
| 监听静音 / 单声道 | 同上 `mute` / `mix_to_mono` | 0/1 |
| 耳机音量 | `buses/<HEADPHONE>/outputs/main/controls/gain` | 0–100，与机身旋钮（耳机焦点）同一参数，实听验证有效。**物理回环实测（2026-11-03）**：衰减 = (100 − 值) dB，严格 1 dB/格（100→−0.7 dB 相对源，50→−50 dB，2→−98 dB），0 = 静音；即 100 dB 量程的对数刻度，0–60 落在 −100…−40 dB，故前 60% 几乎听不见。改值后约 0.5–1 s 平滑过渡（`research/hp_loop.py`，数据 `hp_taper_loop.csv`）。监听音量预计同为固件同一刻度，未经物理回环验证。Volt Control 默认把监听/耳机推子分两段线性映射：0–10% → 设备值 0–40（近似无声区，4 格/%），10–100% → 40–100（每 1% ≈ 0.67 格 = 0.67 dB），全程连续无跳变，界面显示与机身相同的设备值和对应 dB；写 `value` 而非 `normalized_value`；设置里可关（`model::volume`、`config.volume_curve`）。曾用三次幅度曲线（50% ≈ −18 dB），因低位推子动 1% 设备值跳 2–5 格、与机身显示对不上而弃用；也试过 1% 起直接从 40 起步，但 1%→0% 由 40 跳到静音像故障，改为两段 |
| 前级增益 | `buses/<input>/controls/preamp_gain` | dB，0–60（物理旋钮为数字编码器，变化实时推送） |
| 48V / 反相 | `preamp_48v` / `preamp_invert` | 0/1（物理按钮实时推送） |
| Tone | `preamp_tone` | OFF/WARMTH/FOCUS/SHINE。实测有效（−20 dBFS 正弦，相对 OFF）：WARMTH 100 Hz 0 / 1 kHz −1.4 / 8 kHz −3.0 dB；FOCUS −2.9 / −0.5 / +2.1；SHINE −2.0 / −0.6 / +3.4（`research/tone_test.py`） |
| 压缩 | `preamp_compressor` | OFF/FAST/GUITAR/VOCAL。**在 Volt 2 Gen 2 上无效**（2026-11-03 物理回环 MON L → 输入 2 实测：输入 −54…−0.4 dBFS 稳态、−30→−6→−30 dBFS 阶跃，四种模式输出逐点相同，读回值确认已切换）；该机无板载压缩（板载压缩只在 Volt Max / 76 型号上），参数是 UASrv 通用模型带出来的（`research/comp_test.py`） |
| 通道推子 | `buses/<src>/outputs/main/controls/gain` | dB，-144(=-∞)…+12，0 dB≈0.782 归一化 |
| 声像 | `pan`（单声道） / `pan_left`,`pan_right`（立体声） | |
| CUE 发送 | `buses/<src>/outputs/cue_0|cue_1/controls/{gain,pan,mute,pre_fader,enabled}` | |
| 独奏 | `buses/<src>/controls/solo` | 0/1 |
| DIRECT（机身键） | `buses/<input>/outputs/main/controls/mute` **取反** | mute=0 ⇔ DIRECT 亮（输入进入监听混音）；实机按键对比确认 |
| 电平表 | `buses/<b>/outputs/main/meters/level`（name=Level，description="Input Metering"，`units: "percent"`） | 订阅后 ~50 Hz `multi`(tag=meters)，**所有总线（含输入）都用这个**；输入通道上它在 DIRECT/推子之前取值。`buses/<b>/meters/level`（ConsoleInput，units=level）在 Volt 上恒为 0、不推送。HEADPHONE 总线的输出表也从不推送；耳机物理输出 `HP 1` 的 `mix_in_source`=`cue1`，`CUE 1` 输出的 `mix_in_source`=`mon`，即耳机播放监听混音，Volt Control 按这条链在发现时选源（默认用 MONITOR 表，音量前取值） |
| 电平刻度（percent） | — | 不是线性幅度，是 UA 表头位置（正弦峰值实测）：0 ~ −21 dBFS 严格 `1 + dB/33`（−6→0.818，−18→0.455，−21→0.364）；往下压缩：−24→0.318，−30→0.243，−40→0.146，−50→0.065，−60→0.015，−70→0.0037，−80→0.0009。Volt Control 在后端按此表换回线性幅度（`model::meter_amplitude`），`research/meter_cal.py` |
| 电平表实测 | — | 叶子为 `level/{clip,value,peak}/value`（种类在**父节点**名上）。只有 MONITOR(master) 与 LOOPBACK 的输出表推送；HEADPHONE 输出表订阅成功但从不推送。监听输出表在**音量控制之前**取值（音量 0 与 100 读数相同，开 `post_fader_meters` 也一样），Volt 的「MON L/R」录音端点（OBS 经 obs-websocket 读取）同样在音量与静音之前取值（耳机 100/50、监听 0/100 读数恒为 −23 dBFS）。「LOOPBACK L/R」录音端点也一样：−20 dBFS 正弦恒读 −23.01 dBFS RMS（即原样回送），与耳机、监听音量以及 LOOPBACK 自身的 gain（0/50/100）都无关。因此监听/耳机音量曲线无法用软件测量，需物理回环：耳机输出 → 输入 2（48V 关，前级增益临时降到 0，输入 2 的 DIRECT/cue/send 均关以免回授）；用 TRS 线时输入 2 只收到左声道（tip），测试音放左声道即可（`hp_loop.py`；早期尝试：`research/sweep_mon.py`、`sweep_hp.py`、`obs_meter.py`、`obs_mon_check.py`、`loopback_probe.py`）） |

物理输出 `/devices/<uid>/outputs/<uid>`：`MON`(Monitor)、`HP 1`(Headphone, `mix_in_source="cue1"`)、`CUE 1`、`CUE 2`。

## 6. 命令

- 自动增益：`post /auto_gain/trigger_auto_gain {"device_uid":"<uid>","channels":["<bus_uid>"],"duration":<秒>}`；状态见 `/auto_gain` 的 `status/count_down`。
- 立体声链接：`post /link_channels {"device_uid":"<uid>","channel_uid":"<ANALOG 1 bus uid>","stem_format":"stereo"|"mono"}`。
- 清除独奏：`post /devices/<uid>/clear_solos {}`。

## 7. 硬件事实（实机验证）

- Volt 2 Gen 2 只有**一个数字编码器旋钮**，靠面板按键切换它控制的对象（输入 1/2 增益、监听、耳机…），屏幕显示当前对象。
- 旋钮在任何焦点下转动都会实时推送对应参数：输入增益 → `preamp_gain`；耳机 → HEADPHONE 总线 `gain`（实测 100 → 95.3）。软件写同一参数，双向同步。
- **当前焦点（屏幕显示哪一项）不在参数树里**：按键切换前后全树无差异；软件改耳机音量时屏幕也不会切换。因此无法经 UAMixerHelper 让屏幕切页。
- 48V 等按钮实时推送。
- 亮度只有 `display_brightness`（0–255，越界钳位）：只影响屏幕，按键灯亮度不变（22 / 255 实测均无变化）；UA 软件栈里不存在按键亮度参数。
- 焦点切到耳机时，Tone / 48V / AUTO / DIRECT 四个输入专用按键灯会变暗（表示对当前焦点不可用），INPUT 与耳机选择键不变；切回输入后恢复；在耳机焦点下按这四个键中的任意一个，会直接把旋钮焦点切回 Input 1（用户实机观察）。这些都是固件行为，与电脑端写入无关。

## 8. 厂商 HID 协议（UAMixerHelper ↔ Volt，静态 + 动态逆向）

接口：VID 2B5A / PID 002D / MI_04，usage page 0xFF01。hidapi 路径可与 UAMixerHelper 共享打开。

- 主机→设备：39 字节输出报告 `f0 <cmd> <data…> f7`，补零（`WriteFile`，即 hidapi `hid_write`）。
- 设备→主机：74 字节输入报告。电平表 `f1 0c 23 …` 50 Hz；其余为 `f0 <cmd> … f7`。
- 发送路径：分发函数 `0x190a7a0` 从 1024 项环形队列取 `{类别, 属性, arg0, arg1}`，组帧后调用 `0x190c680` → hidapi `hid_write`（`0x1911390`）；收包解析在 `0x190b2a0`（跳转表 `0x190c4e0`，cmd 1..0x23）。

| cmd | 方向 | 含义 / 帧 |
|---|---|---|
| 0x01 | → | 混音器增益 `f0 01 <a> <b> <u32> f7`（9 字节，值钳位 0x7f64f07d） |
| 0x02 | → | 混音器开关 `f0 02 <a> <b> <v> f7` |
| 0x03–0x07 | ↔ | 前级属性（EPreampProp 0–4，48V/相位/AUTO 等） |
| 0x08 | → | 设备开关位 |
| 0x09 / 0x0a | ↔ | 查询类（4 字节） |
| 0x11 | → | 请求整机状态，设备回送状态包 |
| 0x12 | ↔ | `f0 12 <选中输入> <链接掩码> f7`：第 3 字节 bit0 = 输入 1+2 立体声链接（EPreampProp 6）；第 2 字节 = 设备当前选中的输入（0=输入1，1=输入2），软件只记录不回调 |
| 0x13 / 0x15 / 0x17 / 0x18 / 0x1d / 0x1f / 0x23 | ↔ | 其他设备属性 / 位掩码 |
| 0x16 | ↔ | 前级增益 `f0 16 <ch> <lo> <hi&3> f7`（10 bit） |
| 0x1c | ↔ | 耳机/监听音量，值×10 小端：`f0 1c af 03 00 f7` = 94.3 |
| 0x21 | → | 屏幕亮度 `f0 21 <0-255> f7` |
| 0x22 | ↔ | Tone `f0 22 <mask> 00 00 00 f7`（`10` = 输入 2 WARMTH） |
| 0x14 / 0x19 / 0x1a / 0x1b / 0x20 | ← | 仅设备上报（状态值 / 位掩码 / 76 系列压缩表） |

焦点相关结论：

- 按 INPUT 在输入 1/2 间切换时，设备上报 `f0 12 01 00 f7` / `f0 12 00 00 f7`（选中输入变化）；切到耳机/监听、HP 键在音箱/耳机间切换，**都不产生任何上报**。2026-11-03 复核：录 60 s 全部输入报告（含 50 Hz 电平帧 `f1 0c 23`，74 字节，第 3/4 字节 = 输入 1/2 电平，第 55–58 字节 = 监听/耳机电平，其余恒 0），期间依次切耳机、监听、输入 1、输入 2：只有切到输入 2 时出现 `f0 12 01 00 f7`，电平帧任何字节都不随焦点变化。结论：主机只能得知选中的输入（1/2），无法得知旋钮焦点是否在耳机/监听。
- 主机主动发 `f0 12 01 00 f7`（补零 39 字节，写入成功）时，屏幕与按键灯均无变化：固件不接受主机改选中输入。
- UA 软件用到的命令里**没有**切换旋钮焦点/屏幕页的命令；未使用的编号为 0x0b–0x10、0x1e 及 0x23 以上，作用未知，本地无固件镜像可供分析。
- UA Connect（`resources/app.asar` → `build/main/index.cjs`）对 Volt 只用 HID 读 ID 报告：`[0xE8, 0, 0, 0x74, 0xFF]`，回包里有固件 / FPGA / DSP 版本和 UUID。Volt Gen 2 归入 `riod` 家族（固件名 `RIO-2`），固件登录后从 UA 服务器按 `getUpdateUrl({appId, version})` 下载，由 `neo_dfu_utils.exe` 通过 DFU 刷写。本地与公网都没有固件文件。

## 9. 复现脚本（`research/`）

- `probe.py <port> <cmd...>`：发命令看应答
- `dump_tree.py <root> <out.json>`：递归导出参数树（Git Bash 下需 `MSYS_NO_PATHCONV=1`）
- `watch2.py <log> <秒>`：订阅全部控件并记录推送
- `metertest.py` / `multi_raw.py`：电平表与 multi 格式样本
- `hid_sniff.py <秒> <log> [跳过前缀]`：只读嗅探厂商 HID 输入报告
- `hook_run.py <秒> <log> <hook.js>`：Frida 挂 UAMixerHelper（`hook_tusb.js` 记录 HID 写帧，`hook_bt.js` 附调用栈）
- `disfn.py <rva>` / `list_cmds.py` / `jt.py` / `recv_jt.py`：capstone 静态反汇编，列出全部发送点与收包分支
- `hid_try_focus.py [帧…]`：向设备发指定帧并监听回报（仅用于已知帧）

## 10. 其他型号适配（无实机）

- **界面按设备数据生成**：主控页的输入条来自 `/devices/<uid>/buses` 中 `bus_type == "input"` 的总线；立体声对按名称配对（`"ANALOG 1-2"` ↔ `"ANALOG 1"` + `"ANALOG 2"`），每对一个链接参数 `in.<立体声 slug>.link`（Volt 2 为 `in.a12.link`，旧配置里的 `in.link` 映射/上限加载时自动迁移）。设备上不存在的控件对象不生成参数，前端控件缺参数时不渲染。
- **压缩器**：`preamp_compressor` 在所有 Volt 上都有，但 Gen 2 非 76 型号实测无效，按 `device_type` 隐藏（`volt_1/2/4_gen2`）。
- **导出设备信息**（设置 → 导出设备信息）：新开一条连接，只发 `get`，保存 `/`、`/devices/**`、`/auto_gain/**` 到桌面 `VoltControl-<device_type>.json`（格式 `volt-control-tree/1`，`nodes: {path: data}`；含序列号）。
- **离线测试**：把导出文件放进 `volt-control/src-tauri/tests/fixtures/`，`cargo test --lib fixture` 用假 UASrv 回放该文件跑完整 `discover`，检查每个输入都有增益/电平表、链接参数齐全。
- **UAMixerHelper 内置模拟设备**：设置项 `num_emulated_volts_debug` / `emulated_volt_type_debug`（类型 Rio1/Rio2/Rio4/Rio2D/Rio4D/Rio4Dp/OneOfEach）已编译进程序但 flags=0，配置文件、环境变量、命令行、UASrv 都无法打开；唯一途径是二进制补丁（破坏签名，需替换正在运行的 helper），未实施。

## 11. 闲置熄屏（Volt Control 功能）

Volt 没有自带休眠，UA 软件栈里也没有超时/休眠设置；可控的灯光只有屏幕（`display_brightness`），按键灯不可控。Volt Control 在闲置 `screen_sleep_min` 分钟（默认 15，0=从不）后把亮度写 0，原值存入配置 `screen_dimmed_from`；以下任一即唤醒并恢复：软件内操作、MIDI 输入（经抖动滤波）、设备推送的参数变化（亮度本身除外）、Windows 音量键。程序退出时同步恢复；崩溃/强杀后下次连接时恢复（2026-11-03 实测：1 分钟熄屏、强杀重启恢复、UI 操作唤醒均正常）。按编码器切换焦点不产生任何主机可见事件，因此不能唤醒。
