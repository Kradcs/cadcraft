//! Codex app-server JSON-lines transport. All process I/O stays off the UI thread.
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::Value;

// App-server notifications can echo the prepared visual references (up to 24 MiB).
const MAX_MESSAGE_BYTES: usize = 32 * 1024 * 1024;

pub struct Client {
    tx: SyncSender<Value>,
    pub rx: Receiver<Result<Value, String>>,
    child: Arc<Mutex<Option<Child>>>,
}

struct Process(Arc<Mutex<Option<Child>>>);
impl Drop for Process {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.0.lock()
            && let Some(mut child) = guard.take()
        {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.child.lock()
            && let Some(child) = guard.as_mut()
        {
            let _ = child.kill();
        }
    }
}

impl Client {
    pub fn start(executable: &str, ctx: egui::Context) -> Result<Self, String> {
        let (tx, commands) = mpsc::sync_channel(32);
        let (events, rx) = mpsc::sync_channel(256);
        let executable = executable.to_owned();
        let child = Arc::new(Mutex::new(None));
        let process = child.clone();
        std::thread::Builder::new()
            .name("cadcraft-codex".into())
            .spawn(move || {
                if let Err(e) = run(&executable, commands, &events, &ctx, process) {
                    let _ = events.send(Err(e));
                    ctx.request_repaint();
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Self { tx, rx, child })
    }

    pub fn send(&self, value: Value) -> Result<(), String> {
        self.tx.try_send(value).map_err(|e| format!("Codex connection unavailable: {e}"))
    }
}

fn run(
    executable: &str,
    commands: Receiver<Value>,
    events: &SyncSender<Result<Value, String>>,
    ctx: &egui::Context,
    process: Arc<Mutex<Option<Child>>>,
) -> Result<(), String> {
    let process = Process(process);
    let mut command = Command::new(executable);
    command
        .args(["-c", "features.shell_tool=false", "-c", "features.apply_patch_freeform=false", "app-server"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: the desktop conversation must not open a console window.
        command.creation_flags(0x0800_0000);
    }
    let mut child = command.spawn().map_err(|e| format!("Cannot start Codex: {e}. Install Codex CLI and check its executable path."))?;
    let input = child.stdin.take();
    let output = child.stdout.take();
    *process.0.lock().map_err(|_| "Codex process state unavailable")? = Some(child);
    let mut input = input.ok_or("Codex stdin unavailable")?;
    let output = output.ok_or("Codex stdout unavailable")?;
    let (lines_tx, lines) = mpsc::sync_channel(256);
    std::thread::Builder::new()
        .name("cadcraft-codex-output".into())
        .spawn(move || {
            let mut reader = BufReader::new(output);
            loop {
                let mut line = Vec::new();
                let result = reader.by_ref().take(MAX_MESSAGE_BYTES as u64 + 1).read_until(b'\n', &mut line);
                let value = match result {
                    Ok(0) => Err("Codex disconnected".into()),
                    Ok(_) if line.len() > MAX_MESSAGE_BYTES => Err("Codex message exceeds 32 MiB".into()),
                    Ok(_) => serde_json::from_slice(&line).map_err(|e| format!("Invalid Codex message: {e}")),
                    Err(e) => Err(e.to_string()),
                };
                let failed = value.is_err();
                if lines_tx.send(value).is_err() || failed {
                    break;
                }
            }
        })
        .map_err(|e| e.to_string())?;
    loop {
        loop {
            match commands.try_recv() {
                Ok(value) => {
                    serde_json::to_writer(&mut input, &value).map_err(|e| e.to_string())?;
                    input.write_all(b"\n").and_then(|_| input.flush()).map_err(|e| e.to_string())?;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => return Ok(()),
            }
        }
        match lines.recv_timeout(Duration::from_millis(20)) {
            Ok(value) => {
                let failed = value.is_err();
                if events.send(value).is_err() {
                    return Ok(());
                }
                ctx.request_repaint();
                if failed {
                    return Ok(());
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return Err("Codex disconnected".into()),
        }
    }
}

#[cfg(test)]
pub(super) fn test_client() -> (Client, Receiver<Value>) {
    let (tx, rx) = mpsc::sync_channel(32);
    let (_events, inbox) = mpsc::sync_channel(256);
    (Client { tx, rx: inbox, child: Arc::new(Mutex::new(None)) }, rx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_executable_returns_an_actionable_error() {
        let client = Client::start("/cadcraft-nonexistent-executable/codex", egui::Context::default()).unwrap();
        let error = client.rx.recv_timeout(Duration::from_secs(5)).unwrap().unwrap_err();
        assert!(error.contains("Cannot start Codex"), "{error}");
    }

    #[cfg(unix)]
    #[test]
    fn process_transport_roundtrips_json_and_stops_its_child() {
        use std::os::unix::fs::PermissionsExt;
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("cadcraft-agent-{}-{stamp}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        let executable = dir.join("fake-codex");
        std::fs::write(&executable, "#!/bin/sh\nwhile IFS= read -r line; do printf '%s\\n' \"$line\"; done\n").unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
        let client = Client::start(executable.to_str().unwrap(), egui::Context::default()).unwrap();
        let packet = serde_json::json!({"id":1,"method":"initialize","params":{"text":"ação\nquoted \"text\""}});
        client.send(packet.clone()).unwrap();
        assert_eq!(client.rx.recv_timeout(Duration::from_secs(5)).unwrap().unwrap(), packet);
        // A normal prepared image can exceed the old 2 MiB notification limit.
        let visual = serde_json::json!({"type":"image", "url":format!("data:image/png;base64,{}", "A".repeat(3 * 1024 * 1024))});
        client.send(visual.clone()).unwrap();
        assert_eq!(client.rx.recv_timeout(Duration::from_secs(10)).unwrap().unwrap(), visual);
        let process = client.child.clone();
        drop(client);
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while process.lock().unwrap().is_some() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(process.lock().unwrap().is_none(), "the child must be reaped after disconnect");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
