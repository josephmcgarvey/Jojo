//! Records the raw output of a program running in a PTY, for snapshot tests.
//!
//! Usage:
//!   cargo run -p jojod --example record -- --out tests/recordings/NAME.COLSxROWS.bin
//!       --cols 80 --rows 24 --duration-ms 6000
//!       [--step DELAY_MS:TEXT]... -- PROGRAM [ARGS]...
//!
//! TEXT accepts the escapes \r \n \t \e (ESC) \x7f (DEL) and \\.
//! The recorder answers terminal queries (such as the cursor position
//! request ConPTY sends at startup) the way the daemon does, so shells do not
//! stall waiting for a reply.

use jojo_grid::TermGrid;
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn unescape(s: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            let mut buf = [0u8; 4];
            out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
            continue;
        }
        match chars.next() {
            Some('r') => out.push(b'\r'),
            Some('n') => out.push(b'\n'),
            Some('t') => out.push(b'\t'),
            Some('e') => out.push(0x1b),
            Some('\\') => out.push(b'\\'),
            Some('x') => {
                let hex: String = chars.by_ref().take(2).collect();
                out.push(u8::from_str_radix(&hex, 16).expect("two hex digits after \\x"));
            }
            other => panic!("unknown escape \\{other:?}"),
        }
    }
    out
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let split = args.iter().position(|a| a == "--").expect("missing -- before the program");
    let (opts, program) = (&args[..split], &args[split + 1..]);
    let mut out = None;
    let (mut cols, mut rows, mut duration) = (80u16, 24u16, 5000u64);
    let mut steps: Vec<(u64, Vec<u8>)> = Vec::new();
    let mut i = 0;
    while i < opts.len() {
        let value = opts.get(i + 1).expect("option needs a value").clone();
        match opts[i].as_str() {
            "--out" => out = Some(value),
            "--cols" => cols = value.parse()?,
            "--rows" => rows = value.parse()?,
            "--duration-ms" => duration = value.parse()?,
            "--step" => {
                let (delay, text) = value.split_once(':').expect("--step DELAY_MS:TEXT");
                steps.push((delay.parse()?, unescape(text)));
            }
            other => anyhow::bail!("unknown option {other}"),
        }
        i += 2;
    }
    let out = out.expect("--out is required");

    let pair = native_pty_system().openpty(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })?;
    let mut cmd = CommandBuilder::new(&program[0]);
    cmd.args(&program[1..]);
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    // Recordings must not depend on the launching tool's color preference:
    // Claude Code's PowerShell tool (and some CI runners) injects NO_COLOR=1
    // into every command it runs, and pwsh honors that by selecting
    // PlainText output rendering instead of the Host rendering (with VT
    // support) it would otherwise pick under ConPTY. A real Jojo session is
    // never launched that way, so a recording made under NO_COLOR would not
    // reflect what the daemon actually sees.
    cmd.env_remove("NO_COLOR");
    let mut child = pair.slave.spawn_command(cmd)?;
    drop(pair.slave);
    let mut reader = pair.master.try_clone_reader()?;
    let writer = Arc::new(Mutex::new(pair.master.take_writer()?));

    let captured = Arc::new(Mutex::new(Vec::new()));
    let reader_thread = {
        let captured = captured.clone();
        let writer = writer.clone();
        std::thread::spawn(move || {
            let mut grid = TermGrid::new(cols, rows, 10_000);
            let mut buf = vec![0u8; 64 * 1024];
            while let Ok(n) = reader.read(&mut buf) {
                if n == 0 {
                    break;
                }
                captured.lock().unwrap().extend_from_slice(&buf[..n]);
                grid.feed(&buf[..n]);
                let replies = grid.take_replies();
                if !replies.is_empty() {
                    let _ = writer.lock().unwrap().write_all(&replies);
                }
            }
        })
    };

    let start = Instant::now();
    for (delay, text) in steps {
        std::thread::sleep(Duration::from_millis(delay));
        writer.lock().unwrap().write_all(&text)?;
    }
    let remaining = Duration::from_millis(duration).saturating_sub(start.elapsed());
    std::thread::sleep(remaining);

    // Write what has been captured so far to disk before joining the reader
    // thread: if the child never closes its end of the pty (so `reader`
    // never sees EOF) the join below can block indefinitely, and the
    // capture must already be safely on disk by then rather than lost in
    // memory while the process hangs.
    let bytes = captured.lock().unwrap().clone();
    std::fs::write(&out, &bytes)?;
    println!("wrote {} bytes to {out}", bytes.len());
    let _ = child.kill();
    drop(pair.master);
    let _ = reader_thread.join();
    Ok(())
}
