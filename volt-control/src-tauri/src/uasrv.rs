//! Minimal async client for Universal Audio's UASrv protocol, as served by
//! `UAMixerHelper.exe` on 127.0.0.1:4720. See `docs/protocol.md`.
//!
//! Framing: every message is UTF-8 text terminated by `\0`.
//! Requests are `<verb> <path>[ <arg>]`; replies are `{"path":..,"data":..}`
//! or `{"path":..,"error":..}`. Unsolicited pushes reuse the same shape, plus
//! the compressed `{"path":"multi",...}` batch format decoded by [`decode_multi`].

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Result, anyhow};
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot};

pub const DEFAULT_ADDR: &str = "127.0.0.1:4720";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

/// Something the server sent that is not the answer to a pending request.
#[derive(Debug)]
pub enum Push {
    /// A single property update; `path` ends with the property name
    /// (the trailing `/value` of the wire path has been stripped).
    Value { path: String, value: Value },
    /// A meter batch: `(meter property path, per-channel values)`.
    Meters(Vec<(String, Value)>),
}

type Reply = oneshot::Sender<Result<Value>>;
type Pending = Arc<Mutex<HashMap<String, VecDeque<Reply>>>>;

#[derive(Clone)]
pub struct Client {
    tx: mpsc::UnboundedSender<Vec<u8>>,
    pending: Pending,
}

impl Client {
    /// Connects and spawns the reader/writer tasks. The returned receiver
    /// yields pushes and closes when the connection is lost.
    pub async fn connect(addr: &str) -> Result<(Client, mpsc::UnboundedReceiver<Push>)> {
        let stream = TcpStream::connect(addr).await?;
        stream.set_nodelay(true)?;
        let (mut rd, mut wr) = stream.into_split();
        let (tx, mut out_rx) = mpsc::unbounded_channel::<Vec<u8>>();
        let (push_tx, push_rx) = mpsc::unbounded_channel();
        let pending: Pending = Arc::default();

        tokio::spawn(async move {
            while let Some(frame) = out_rx.recv().await {
                if wr.write_all(&frame).await.is_err() {
                    break;
                }
            }
        });

        let pend = pending.clone();
        tokio::spawn(async move {
            let mut buf: Vec<u8> = Vec::with_capacity(1 << 16);
            let mut chunk = vec![0u8; 1 << 16];
            loop {
                let n = match rd.read(&mut chunk).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => n,
                };
                buf.extend_from_slice(&chunk[..n]);
                while let Some(end) = buf.iter().position(|&b| b == 0) {
                    let frame: Vec<u8> = buf.drain(..=end).collect();
                    // Some strings (e.g. "-∞") are not valid UTF-8 on the wire.
                    let text = String::from_utf8_lossy(&frame[..frame.len() - 1]);
                    match serde_json::from_str::<Value>(&text) {
                        Ok(msg) => dispatch(msg, &pend, &push_tx),
                        Err(e) => log::warn!("uasrv: bad frame ({e}): {}", truncate(&text, 200)),
                    }
                }
            }
            // Connection gone: fail everything still waiting.
            for (_, waiters) in pend.lock().unwrap().drain() {
                for w in waiters {
                    let _ = w.send(Err(anyhow!("connection closed")));
                }
            }
        });

        Ok((Client { tx, pending }, push_rx))
    }

    /// Sends a request and waits for the reply addressed to `path`.
    pub async fn request(&self, verb: &str, path: &str, arg: Option<&str>) -> Result<Value> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.pending
            .lock()
            .unwrap()
            .entry(path.to_string())
            .or_default()
            .push_back(reply_tx);
        self.send_line(verb, path, arg)?;
        match tokio::time::timeout(REQUEST_TIMEOUT, reply_rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(anyhow!("connection closed")),
            Err(_) => Err(anyhow!("timeout waiting for {verb} {path}")),
        }
    }

    /// Fire-and-forget. The server's echo arrives as a [`Push::Value`].
    pub fn send_line(&self, verb: &str, path: &str, arg: Option<&str>) -> Result<()> {
        let mut line = format!("{verb} {path}");
        if let Some(a) = arg {
            line.push(' ');
            line.push_str(a);
        }
        let mut frame = line.into_bytes();
        frame.push(0);
        self.tx.send(frame).map_err(|_| anyhow!("connection closed"))
    }

    pub async fn get(&self, path: &str) -> Result<Value> {
        self.request("get", path, None).await
    }

    pub async fn post(&self, path: &str, args: &Value) -> Result<Value> {
        self.request("post", path, Some(&args.to_string())).await
    }

    pub async fn subscribe(&self, path: &str) -> Result<()> {
        self.request("subscribe", path, None).await.map(|_| ())
    }
}

fn dispatch(msg: Value, pending: &Pending, push_tx: &mpsc::UnboundedSender<Push>) {
    let Some(path) = msg.get("path").and_then(Value::as_str) else {
        return;
    };
    if path == "multi" {
        let data = &msg["data"];
        let leaves = decode_multi(data);
        if data.get("tag").and_then(Value::as_str) == Some("meters") {
            let _ = push_tx.send(Push::Meters(leaves.into_iter().map(|(p, v)| (strip_value(p), v)).collect()));
        } else {
            for (p, v) in leaves {
                let _ = push_tx.send(Push::Value { path: strip_value(p), value: v });
            }
        }
        return;
    }
    let waiter = pending.lock().unwrap().get_mut(path).and_then(VecDeque::pop_front);
    if let Some(w) = waiter {
        let result = match msg.get("error") {
            Some(err) => Err(anyhow!("{path}: {}", err.as_str().unwrap_or("error"))),
            None => Ok(msg.get("data").cloned().unwrap_or(Value::Null)),
        };
        let _ = w.send(result);
        return;
    }
    if let Some(err) = msg.get("error") {
        log::warn!("uasrv: {path}: {err}");
        return;
    }
    match msg.get("data") {
        None | Some(Value::Null) => {}
        Some(v) => {
            // Subscription pushes and fire-and-forget echoes are both "<obj>/<prop>/value".
            let _ = push_tx.send(Push::Value { path: strip_value(path.to_string()), value: v.clone() });
        }
    }
}

/// Every update path carries one trailing `/value` (the property's value):
/// `".../gain/normalized_value/value"` -> `".../gain/normalized_value"`.
/// Writers therefore always `set <obj>/<prop>/value`, so echoes follow the same rule.
fn strip_value(mut p: String) -> String {
    if p.ends_with("/value") {
        p.truncate(p.len() - "/value".len());
    }
    p
}

/// Expands a `multi` batch into `(absolute path, value)` pairs.
///
/// Node `i` is named `string[name[i]]` and hangs under `parent[i]`
/// (a node that is its own parent is top-level). `data[j]` belongs to node `path[j]`.
pub fn decode_multi(d: &Value) -> Vec<(String, Value)> {
    let arr = |k: &str| d.get(k).and_then(Value::as_array).cloned().unwrap_or_default();
    let (strings, names, parents, leaves, data) = (arr("string"), arr("name"), arr("parent"), arr("path"), arr("data"));
    let idx = |v: &Value| v.as_u64().map(|x| x as usize);
    let mut out = Vec::with_capacity(leaves.len());
    'leaf: for (j, leaf) in leaves.iter().enumerate() {
        let Some(mut i) = idx(leaf) else { continue };
        let mut segs = Vec::new();
        for _ in 0..=names.len() {
            let Some(name) = names.get(i).and_then(idx).and_then(|s| strings.get(s)).and_then(Value::as_str) else {
                continue 'leaf;
            };
            segs.push(name);
            let Some(p) = parents.get(i).and_then(idx) else { continue 'leaf };
            if p == i {
                segs.reverse();
                out.push((format!("/{}", segs.join("/")), data.get(j).cloned().unwrap_or(Value::Null)));
                continue 'leaf;
            }
            i = p;
        }
        log::warn!("uasrv: cyclic multi packet ignored");
    }
    out
}

fn truncate(s: &str, n: usize) -> &str {
    match s.char_indices().nth(n) {
        Some((i, _)) => &s[..i],
        None => s,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn decodes_multi_sample() {
        // Captured from UAMixerHelper after `set .../gain/value 100`.
        let d = json!({"data":[100.0,1.0,"+100.0"],
            "string":["devices","3236","buses","a7fe","outputs","main","controls","gain","real_value","value","normalized_value","string_value"],
            "name":[0,1,2,3,4,5,6,7,8,9,10,9,11,9],
            "parent":[0,0,1,2,3,4,5,6,7,8,7,10,7,12],
            "path":[9,11,13]});
        let got = decode_multi(&d);
        assert_eq!(got[0].0, "/devices/3236/buses/a7fe/outputs/main/controls/gain/real_value/value");
        assert_eq!(got[1].0, "/devices/3236/buses/a7fe/outputs/main/controls/gain/normalized_value/value");
        assert_eq!(got[2].1, json!("+100.0"));
        assert_eq!(strip_value(got[1].0.clone()), "/devices/3236/buses/a7fe/outputs/main/controls/gain/normalized_value");
    }

    #[test]
    fn strip_value_removes_exactly_one_suffix() {
        assert_eq!(strip_value("/a/controls/gain/value/value".into()), "/a/controls/gain/value");
        assert_eq!(strip_value("/dev/display_brightness/value".into()), "/dev/display_brightness");
        assert_eq!(strip_value("/dev/display_brightness".into()), "/dev/display_brightness");
    }
}
