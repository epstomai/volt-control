//! UI language for backend-made text: status line, notices, tray, parameter names.
//!
//! Each row of [`TEXTS`] holds one string in every language, so a missing
//! translation cannot compile. `{0}`, `{1}` are positional placeholders.
//! The frontend has its own dictionary (`src/lib/i18n.svelte.ts`).

use std::sync::atomic::{AtomicU8, Ordering};

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Lang {
    #[serde(rename = "zh-CN")]
    ZhCn,
    #[serde(rename = "zh-TW")]
    ZhTw,
    #[serde(rename = "en")]
    En,
    #[serde(rename = "ja")]
    Ja,
}

const ALL: [Lang; 4] = [Lang::ZhCn, Lang::ZhTw, Lang::En, Lang::Ja];

static CURRENT: AtomicU8 = AtomicU8::new(0);

pub fn current() -> Lang {
    ALL[CURRENT.load(Ordering::Relaxed) as usize]
}

pub fn set(lang: Lang) {
    CURRENT.store(lang as u8, Ordering::Relaxed);
}

/// The configured language, or the Windows display language when unset.
pub fn resolve(setting: Option<Lang>) -> Lang {
    setting.unwrap_or_else(system)
}

/// Windows display language; anything not translated falls back to English.
pub fn system() -> Lang {
    let id = unsafe { windows::Win32::Globalization::GetUserDefaultUILanguage() };
    from_langid(id)
}

fn from_langid(id: u16) -> Lang {
    const LANG_CHINESE: u16 = 0x04;
    const LANG_JAPANESE: u16 = 0x11;
    match id & 0x3ff {
        // zh-TW, zh-HK, zh-MO and neutral zh-Hant use traditional characters.
        LANG_CHINESE if matches!(id, 0x0404 | 0x0c04 | 0x1404 | 0x7c04) => Lang::ZhTw,
        LANG_CHINESE => Lang::ZhCn,
        LANG_JAPANESE => Lang::Ja,
        _ => Lang::En,
    }
}

/// Text for `key` in the current language; an unknown key is returned as-is.
pub fn t(key: &str) -> &str {
    match TEXTS.iter().find(|(k, _)| *k == key) {
        Some((_, row)) => row[current() as usize],
        None => {
            log::error!("i18n: missing key {key}");
            key
        }
    }
}

/// [`t`] with `{0}`, `{1}`, … replaced by `args`.
pub fn tf(key: &str, args: &[&dyn std::fmt::Display]) -> String {
    let mut s = t(key).to_string();
    for (i, a) in args.iter().enumerate() {
        s = s.replace(&format!("{{{i}}}"), &a.to_string());
    }
    s
}

/// A message that can be rendered again after the language changes (the status line).
#[derive(Clone, Debug)]
pub struct Msg {
    key: &'static str,
    args: Vec<String>,
}

impl Msg {
    pub fn new(key: &'static str) -> Msg {
        Msg { key, args: Vec::new() }
    }

    pub fn with(key: &'static str, args: &[&dyn std::fmt::Display]) -> Msg {
        Msg { key, args: args.iter().map(|a| a.to_string()).collect() }
    }

    /// Text that is already final (e.g. an error message).
    pub fn raw(text: impl Into<String>) -> Msg {
        Msg { key: "", args: vec![text.into()] }
    }

    pub fn render(&self) -> String {
        if self.key.is_empty() {
            return self.args.concat();
        }
        let args: Vec<&dyn std::fmt::Display> = self.args.iter().map(|a| a as &dyn std::fmt::Display).collect();
        tf(self.key, &args)
    }
}

/// Display name of a parameter, derived from its id so it can be re-translated in place.
pub fn param_label(id: &str) -> String {
    let parts: Vec<&str> = id.split('.').collect();
    let key = match parts.as_slice() {
        ["mon", "volume"] => "p.mon_volume",
        ["mon", "mute"] => "p.mon_mute",
        ["mon", "mono"] => "p.mon_mono",
        ["hp", "volume"] => "p.hp_volume",
        ["hp", "mute"] => "p.hp_mute",
        ["hp", "mono"] => "p.hp_mono",
        [cue @ ("cue1" | "cue2"), what] => {
            let name = if *cue == "cue1" { "CUE 1" } else { "CUE 2" };
            let key = match *what {
                "volume" => "p.cue_volume",
                "mute" => "p.cue_mute",
                "mono" => "p.cue_mono",
                _ => return id.to_string(),
            };
            return tf(key, &[&name]);
        }
        ["dev", "dim"] => return "Dim".into(),
        ["in", _, "gain"] => "p.gain",
        ["in", _, "48v"] => return "48V".into(),
        ["in", _, "tone"] => return "Tone".into(),
        ["in", _, "comp"] => "p.comp",
        ["in", _, "invert"] => "p.invert",
        ["in", _, "direct"] => return "DIRECT".into(),
        ["in", _, "jack_mic"] => return "MIC".into(),
        ["in", _, "jack_line"] => return "LINE".into(),
        ["in", _, "jack_hiz"] => return "HI-Z".into(),
        ["in", _, "autogain"] => "p.autogain",
        ["in", _, "link"] => "p.link",
        ["bus", _, "enabled"] => "p.enabled",
        ["mix", _, "solo"] => return "Solo".into(),
        ["mix", _, _, "gain"] => "p.fader",
        ["mix", _, _, "mute"] => "p.mute",
        ["mix", _, _, "pan"] => "p.pan",
        ["mix", _, _, "pan_l"] => "p.pan_l",
        ["mix", _, _, "pan_r"] => "p.pan_r",
        ["mix", _, _, "pre"] => "p.pre_fader",
        ["dev", "online"] => "p.online",
        ["dev", "sample_rate"] => "p.sample_rate",
        ["dev", "brightness"] => "p.brightness",
        ["dev", "no_clip"] => "p.no_clip",
        ["sys", "peak_hold"] => "p.peak_hold",
        ["sys", "clip_hold"] => "p.clip_hold",
        ["sys", "post_fader_meters"] => "p.post_fader_meters",
        ["act", "identify"] => "p.identify",
        ["act", "clear_solos"] => "p.clear_solos",
        ["win", "volume"] => "p.sys_volume",
        ["win", "mute"] => "p.sys_mute",
        _ => return id.to_string(),
    };
    t(key).to_string()
}

/// key -> [简体中文, 繁體中文, English, 日本語] (order of [`Lang`]).
#[rustfmt::skip]
const TEXTS: &[(&str, [&str; 4])] = &[
    // --- connection status
    ("status.starting", ["正在启动…", "正在啟動…", "Starting…", "起動中…"]),
    ("status.connecting", ["正在连接 UA Mixer 服务…", "正在連線 UA Mixer 服務…", "Connecting to the UA Mixer service…", "UA Mixer サービスに接続中…"]),
    ("status.helper_starting", ["正在启动 UAMixerHelper…", "正在啟動 UAMixerHelper…", "Starting UAMixerHelper…", "UAMixerHelper を起動中…"]),
    ("status.helper_unresponsive", [
        "UAMixerHelper 已在运行但未响应 4720 端口，等待其就绪…",
        "UAMixerHelper 已在執行但未回應 4720 連接埠，等待其就緒…",
        "UAMixerHelper is running but not answering on port 4720; waiting…",
        "UAMixerHelper は起動していますがポート 4720 に応答しません。待機中…",
    ]),
    ("status.helper_exited", [
        "UAMixerHelper 启动后退出，稍后重试…",
        "UAMixerHelper 啟動後即結束，稍後重試…",
        "UAMixerHelper exited right after launch; retrying shortly…",
        "UAMixerHelper が起動直後に終了しました。まもなく再試行します…",
    ]),
    ("status.helper_missing", ["未找到 {0}，请安装 UAD Console", "找不到 {0}，請安裝 UAD Console", "{0} not found. Please install UAD Console.", "{0} が見つかりません。UAD Console をインストールしてください"]),
    ("status.helper_failed", ["启动 UAMixerHelper 失败：{0}", "啟動 UAMixerHelper 失敗：{0}", "Could not start UAMixerHelper: {0}", "UAMixerHelper を起動できません: {0}"]),
    ("status.connected", ["已连接 {0}", "已連線 {0}", "Connected to {0}", "{0} に接続済み"]),
    ("status.connected_partial", ["已连接 {0}，{1} 个订阅失败", "已連線 {0}，{1} 個訂閱失敗", "Connected to {0}; {1} subscriptions failed", "{0} に接続済み（{1} 件の購読に失敗）"]),
    ("status.lost", ["与 UA Mixer 服务的连接已断开，正在重连…", "與 UA Mixer 服務的連線已中斷，正在重新連線…", "Lost the connection to the UA Mixer service; reconnecting…", "UA Mixer サービスとの接続が切れました。再接続中…"]),
    ("status.device_offline", ["Volt 已断开", "Volt 已中斷連線", "Volt disconnected", "Volt が切断されました"]),
    ("status.no_response", ["UA Mixer 服务无响应，正在重连…", "UA Mixer 服務無回應，正在重新連線…", "The UA Mixer service stopped responding; reconnecting…", "UA Mixer サービスが応答しません。再接続中…"]),
    // --- errors
    ("err.no_volt", ["未找到在线的 Volt 设备", "找不到連線中的 Volt 裝置", "No online Volt device found", "オンラインの Volt デバイスが見つかりません"]),
    ("err.no_ua_device", ["未找到 UA 设备", "找不到 UA 裝置", "No UA device found", "UA デバイスが見つかりません"]),
    ("err.system_volume", ["系统音量不可用", "系統音量無法使用", "System volume is unavailable", "システム音量を利用できません"]),
    ("err.not_connected", ["设备未连接", "裝置未連線", "Device not connected", "デバイスが接続されていません"]),
    ("err.engine_stopped", ["引擎已停止", "引擎已停止", "Engine stopped", "エンジンが停止しました"]),
    ("err.unknown_param", ["未知参数 {0}", "未知參數 {0}", "Unknown parameter {0}", "不明なパラメーター {0}"]),
    ("err.no_mapping", ["映射不存在", "對應不存在", "Mapping not found", "マッピングが存在しません"]),
    ("err.no_port", ["端口不存在", "連接埠不存在", "Port not found", "ポートが存在しません"]),
    // --- notices
    ("notice.midi_gone", ["MIDI 设备已断开，重新插入后会自动连接", "MIDI 裝置已中斷，重新插入後會自動連線", "MIDI device disconnected; it reconnects when plugged back in", "MIDI デバイスが切断されました。再接続すると自動的に接続します"]),
    ("notice.midi_back", ["MIDI 设备已连接", "MIDI 裝置已連線", "MIDI device connected", "MIDI デバイスが接続されました"]),
    ("notice.save_failed", ["保存配置失败：{0}", "儲存設定失敗：{0}", "Could not save settings: {0}", "設定を保存できません: {0}"]),
    ("notice.post_failed", ["{0} 失败：{1}", "{0} 失敗：{1}", "{0} failed: {1}", "{0} が失敗しました: {1}"]),
    ("notice.learn_failed", ["MIDI 学习失败：{0}", "MIDI 學習失敗：{0}", "MIDI learn failed: {0}", "MIDI ラーンに失敗しました: {0}"]),
    ("midi.input_failed", [
        "MIDI 输入 “{0}” 打开失败：{1}（可能被其他程序独占）",
        "MIDI 輸入「{0}」開啟失敗：{1}（可能被其他程式獨占）",
        "Could not open MIDI input \"{0}\": {1} (another program may be using it)",
        "MIDI 入力「{0}」を開けません: {1}（他のアプリが使用中の可能性があります）",
    ]),
    ("midi.output_failed", ["MIDI 输出 “{0}” 打开失败：{1}", "MIDI 輸出「{0}」開啟失敗：{1}", "Could not open MIDI output \"{0}\": {1}", "MIDI 出力「{0}」を開けません: {1}"]),
    // --- tray
    ("tray.show", ["显示 Volt Control", "顯示 Volt Control", "Show Volt Control", "Volt Control を表示"]),
    ("tray.mon_mute", ["监听静音 开/关", "監聽靜音 開/關", "Monitor mute on/off", "モニターミュート オン/オフ"]),
    ("tray.hp_mute", ["耳机静音 开/关", "耳機靜音 開/關", "Headphone mute on/off", "ヘッドホンミュート オン/オフ"]),
    ("tray.quit", ["退出", "結束", "Quit", "終了"]),
    ("tray.tooltip", ["Volt Control\n监听 {0}  ·  耳机 {1}", "Volt Control\n監聽 {0}  ·  耳機 {1}", "Volt Control\nMonitor {0}  ·  Phones {1}", "Volt Control\nモニター {0}  ·  ヘッドホン {1}"]),
    // --- parameter names
    ("p.mon_volume", ["监听音量", "監聽音量", "Monitor volume", "モニター音量"]),
    ("p.mon_mute", ["监听静音", "監聽靜音", "Monitor mute", "モニターミュート"]),
    ("p.mon_mono", ["监听单声道", "監聽單聲道", "Monitor mono", "モニターモノラル"]),
    ("p.hp_volume", ["耳机音量", "耳機音量", "Headphone volume", "ヘッドホン音量"]),
    ("p.hp_mute", ["耳机静音", "耳機靜音", "Headphone mute", "ヘッドホンミュート"]),
    ("p.hp_mono", ["耳机单声道", "耳機單聲道", "Headphone mono", "ヘッドホンモノラル"]),
    ("p.cue_volume", ["{0} 总音量", "{0} 總音量", "{0} master volume", "{0} マスター音量"]),
    ("p.cue_mute", ["{0} 静音", "{0} 靜音", "{0} mute", "{0} ミュート"]),
    ("p.cue_mono", ["{0} 单声道", "{0} 單聲道", "{0} mono", "{0} モノラル"]),
    ("p.gain", ["增益", "增益", "Gain", "ゲイン"]),
    ("p.comp", ["压缩", "壓縮", "Compressor", "コンプレッサー"]),
    ("p.invert", ["反相", "反相", "Polarity invert", "位相反転"]),
    ("p.autogain", ["自动增益", "自動增益", "Auto gain", "オートゲイン"]),
    ("p.link", ["立体声链接", "立體聲連結", "Stereo link", "ステレオリンク"]),
    ("p.enabled", ["启用", "啟用", "Enabled", "有効"]),
    ("p.fader", ["推子", "推桿", "Fader", "フェーダー"]),
    ("p.mute", ["静音", "靜音", "Mute", "ミュート"]),
    ("p.pan", ["声像", "聲像", "Pan", "パン"]),
    ("p.pan_l", ["声像 L", "聲像 L", "Pan L", "パン L"]),
    ("p.pan_r", ["声像 R", "聲像 R", "Pan R", "パン R"]),
    ("p.pre_fader", ["推子前", "推桿前", "Pre-fader", "プリフェーダー"]),
    ("p.online", ["在线", "連線中", "Online", "オンライン"]),
    ("p.sample_rate", ["采样率", "取樣率", "Sample rate", "サンプルレート"]),
    ("p.brightness", ["屏幕亮度", "螢幕亮度", "Screen brightness", "画面の明るさ"]),
    ("p.no_clip", ["防削波", "防削波", "No-Clip", "No-Clip"]),
    ("p.peak_hold", ["峰值保持", "峰值保持", "Peak hold", "ピークホールド"]),
    ("p.clip_hold", ["削波保持", "削波保持", "Clip hold", "クリップホールド"]),
    ("p.post_fader_meters", ["推子后电平表", "推桿後電平表", "Post-fader meters", "ポストフェーダーメーター"]),
    ("p.identify", ["定位设备", "定位裝置", "Identify device", "デバイスを識別"]),
    ("p.clear_solos", ["清除 Solo", "清除 Solo", "Clear solos", "ソロを解除"]),
    ("p.sys_volume", ["系统音量", "系統音量", "System volume", "システム音量"]),
    ("p.sys_mute", ["系统静音", "系統靜音", "System mute", "システムミュート"]),
];

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests that switch the global language must not interleave.
    static LANG_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn placeholders(s: &str) -> Vec<usize> {
        (0..4).filter(|i| s.contains(&format!("{{{i}}}"))).collect()
    }

    #[test]
    fn rows_are_complete_and_unique() {
        let mut seen = std::collections::HashSet::new();
        for (key, row) in TEXTS {
            assert!(seen.insert(key), "duplicate key {key}");
            let first = placeholders(row[0]);
            for s in row {
                assert!(!s.is_empty(), "{key}: empty translation");
                assert_eq!(placeholders(s), first, "{key}: placeholders differ in {s:?}");
            }
        }
    }

    /// Every `t("…")`, `tf("…")`, `Msg::new("…")` / `Msg::with("…")` in the sources has a row.
    #[test]
    fn used_keys_exist() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|e| e != "rs") {
                continue;
            }
            let src = std::fs::read_to_string(&path).unwrap();
            for call in ["t(\"", "tf(\"", "Msg::new(\"", "Msg::with(\""] {
                for (i, _) in src.match_indices(call) {
                    // `insert("…` is not `t("…`
                    if src[..i].chars().next_back().is_some_and(|c| c.is_alphanumeric() || c == '_') {
                        continue;
                    }
                    let rest = &src[i + call.len()..];
                    let key = &rest[..rest.find('"').unwrap()];
                    let key_like = key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.');
                    if key_like && key.contains('.') {
                        assert!(TEXTS.iter().any(|(k, _)| *k == key), "{}: unknown key {key}", path.display());
                    }
                }
            }
        }
    }

    #[test]
    fn param_labels_follow_language() {
        let _g = LANG_LOCK.lock().unwrap();
        set(Lang::En);
        assert_eq!(param_label("mix.a1.cue1.gain"), "Fader");
        assert_eq!(param_label("cue2.volume"), "CUE 2 master volume");
        assert_eq!(param_label("in.a12.link"), "Stereo link");
        set(Lang::Ja);
        assert_eq!(param_label("mon.volume"), "モニター音量");
        set(Lang::ZhCn);
        assert_eq!(param_label("win.mute"), "系统静音");
        assert_eq!(param_label("in.a1.48v"), "48V");
        assert_eq!(param_label("something.new"), "something.new");
    }

    #[test]
    fn langid_mapping() {
        assert_eq!(from_langid(0x0804), Lang::ZhCn);
        assert_eq!(from_langid(0x0404), Lang::ZhTw);
        assert_eq!(from_langid(0x0c04), Lang::ZhTw);
        assert_eq!(from_langid(0x0411), Lang::Ja);
        assert_eq!(from_langid(0x0409), Lang::En);
        assert_eq!(from_langid(0x0407), Lang::En);
    }

    #[test]
    fn msg_renders_in_current_language() {
        let _g = LANG_LOCK.lock().unwrap();
        let m = Msg::with("status.connected", &[&"Volt 2"]);
        set(Lang::En);
        assert_eq!(m.render(), "Connected to Volt 2");
        set(Lang::ZhTw);
        assert_eq!(m.render(), "已連線 Volt 2");
        assert_eq!(Msg::raw("x").render(), "x");
        set(Lang::ZhCn);
    }
}
