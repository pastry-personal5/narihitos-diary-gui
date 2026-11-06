#![allow(clippy::zombie_processes)] // the scenarios deliberately orphan children

//! Test double for supervised CLIs. Scenario is argv[1]; codex-shaped invocations (`--version`,
//! `login status`, `exec ...`) read their scenario from `$CODEX_HOME/scenario`; claude-shaped ones
//! (`--version`, `auth status`, `-p ...`, recognised by `$CLAUDE_CONFIG_DIR`) from `$CLAUDE_CONFIG_DIR/scenario`;
//! qwen-shaped ones (`--version`, then flags with the prompt on stdin, recognised by `$QWEN_HOME`) from
//! `$QWEN_HOME/scenario`. Never shipped.
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::Duration;

fn out(s: &str) {
    let mut o = std::io::stdout().lock();
    let _ = writeln!(o, "{s}");
    let _ = o.flush();
}
fn sleep(s: u64) {
    std::thread::sleep(Duration::from_secs(s));
}
fn me() -> std::path::PathBuf {
    std::env::current_exe().unwrap()
}
fn ignore_term() {
    // SAFETY: SIG_IGN is a valid disposition.
    unsafe { libc::signal(libc::SIGTERM, libc::SIG_IGN) };
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let first = args.first().map(String::as_str).unwrap_or("");
    match first {
        "env" => {
            for (k, v) in std::env::vars() {
                out(&format!("env {k}={v}"));
            }
            // SAFETY: plain syscalls.
            out(&format!("pgid {}", unsafe { libc::getpgrp() }));
            out(&format!("pid {}", std::process::id()));
            out(&format!(
                "cwd {}",
                std::env::current_dir().unwrap().display()
            ));
            let mut s = String::new();
            std::io::stdin().read_to_string(&mut s).unwrap();
            out(&format!("stdin {}", s.len()));
        }
        "echo-stdin" => {
            let mut s = String::new();
            std::io::stdin().read_to_string(&mut s).unwrap();
            for l in s.lines() {
                out(l);
            }
        }
        "lines" => {
            for l in &args[1..] {
                out(l);
            }
        }
        "flood-stderr" => {
            let chunk = vec![b'e'; 64 * 1024];
            let mut e = std::io::stderr().lock();
            for _ in 0..48 {
                let _ = e.write_all(&chunk);
            }
            out("done");
        }
        "big-record" => {
            let _ = std::io::stdout().write_all(&vec![b'x'; 4096]);
            sleep(30);
        }
        "flood-stdout" => loop {
            if writeln!(std::io::stdout(), "{}", "y".repeat(100)).is_err() {
                break;
            }
        },
        "silent" => sleep(60),
        "drip" => loop {
            out("tick");
            std::thread::sleep(Duration::from_millis(200));
        },
        "ignore-term" => {
            ignore_term();
            out(&format!("pid {}", std::process::id()));
            out("ready");
            sleep(60);
        }
        "grandchild" => {
            let c = Command::new(me())
                .arg("silent")
                .stdout(Stdio::null())
                .spawn()
                .unwrap();
            out(&format!("grandchild {}", c.id()));
            if args.get(1).map(String::as_str) == Some("exit") {
                return;
            }
            sleep(60);
        }
        "escape" => {
            let c = Command::new(me())
                .arg("setsid-sleep")
                .stdout(Stdio::null())
                .spawn()
                .unwrap();
            out(&format!("escapee {}", c.id()));
            sleep(1); // stay alive so the supervisor scans while the escapee is still our child
        }
        "setsid-sleep" => {
            // SAFETY: plain syscall.
            unsafe { libc::setsid() };
            sleep(60);
        }
        "late-output" => {
            // On TERM, emit one more record and a result before dying.
            extern "C" fn on_term(_: libc::c_int) {
                let msg = b"{\"late\":true}\n";
                // SAFETY: write(2) is async-signal-safe.
                unsafe { libc::write(1, msg.as_ptr().cast(), msg.len()) };
                unsafe { libc::_exit(0) };
            }
            // SAFETY: handler is async-signal-safe.
            unsafe { libc::signal(libc::SIGTERM, on_term as *const () as usize) };
            out("started");
            sleep(60);
        }
        _ if std::env::var_os("CLAUDE_CONFIG_DIR").is_some() => claude(&args),
        _ if std::env::var_os("QWEN_HOME").is_some() => qwen(&args),
        _ => codex(&args),
    }
}

const GOOD: &str = r#"{"draft_markdown":"Tea was nice.","claim_support":[{"claim":"Had tea","snippet_ids":["snippet-1"]}],"warnings":[]}"#;

fn arg_after(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

/// Emulates the parts of `codex` the adapter uses. Scenario and recordings live in `$CODEX_HOME`.
fn codex(args: &[String]) {
    let home = std::path::PathBuf::from(std::env::var("CODEX_HOME").unwrap_or_default());
    let file = |n: &str| {
        std::fs::read_to_string(home.join(n))
            .unwrap_or_default()
            .trim()
            .to_owned()
    };
    let a0 = args.first().map(String::as_str).unwrap_or("");
    if a0 == "--version" {
        let v = file("version");
        out(if v.is_empty() {
            "codex-cli 0.167.0"
        } else {
            &v
        });
        return;
    }
    if a0 == "login" {
        match file("login").as_str() {
            "none" => {
                out("Not logged in");
                std::process::exit(1);
            }
            "boom" => std::process::exit(2),
            _ => out("Logged in using ChatGPT"),
        }
        return;
    }
    if a0 != "exec" {
        std::process::exit(64);
    }
    let mut prompt = String::new();
    std::io::stdin().read_to_string(&mut prompt).unwrap();
    let _ = std::fs::write(home.join("argv"), args.join("\n"));
    let _ = std::fs::write(home.join("stdin"), &prompt);
    let last = arg_after(args, "--output-last-message").unwrap();
    let chat = arg_after(args, "--output-schema").is_none();
    let body = |good: &str| {
        if chat {
            "A reply.".to_owned()
        } else {
            good.to_owned()
        }
    };
    let started = || {
        out(r#"{"type":"thread.started","thread_id":"t"}"#);
        out(r#"{"type":"turn.started"}"#);
    };
    let message = |text: &str| {
        let v = serde_json::json!({"type":"item.completed","item":{"id":"item_0","type":"agent_message","text":text}});
        out(&v.to_string());
    };
    let done = || out(r#"{"type":"turn.completed","usage":{"input_tokens":1,"output_tokens":1}}"#);
    let fail = |m: &str| {
        let v = serde_json::json!({"type":"turn.failed","error":{"message":m}});
        out(&v.to_string());
        std::process::exit(1);
    };
    match file("scenario").as_str() {
        "malformed" => {
            let _ = std::fs::write(&last, "not json");
            started();
            message("not json");
            done();
        }
        "fenced" => {
            let t = format!("```json\n{GOOD}\n```");
            let _ = std::fs::write(&last, &t);
            started();
            message(&t);
            done();
        }
        "split" => {
            let _ = std::fs::write(&last, body(GOOD));
            let line = serde_json::json!({"type":"item.completed","item":{"id":"item_0","type":"agent_message","text":body(GOOD)}}).to_string();
            let (a, b) = line.split_at(line.len() / 2);
            started();
            let mut o = std::io::stdout().lock();
            let _ = o.write_all(a.as_bytes());
            let _ = o.flush();
            std::thread::sleep(Duration::from_millis(150));
            let _ = writeln!(o, "{b}");
            drop(o);
            done();
        }
        "unauthorized" => {
            started();
            fail(
                "unexpected status 401 Unauthorized: Missing bearer or basic authentication in header",
            );
        }
        "ratelimit" => {
            started();
            fail("You have hit your usage limit");
        }
        "context" => {
            started();
            fail("Your input exceeds the context window of this model");
        }
        "otherfail" => {
            started();
            fail("something odd");
        }
        "silent-exec" => {
            started();
            sleep(60);
        }
        "late" => {
            started();
            extern "C" fn on_term(_: libc::c_int) {
                let msg = b"{\"type\":\"turn.completed\"}\n";
                // SAFETY: write(2) and _exit are async-signal-safe.
                unsafe { libc::write(1, msg.as_ptr().cast(), msg.len()) };
                unsafe { libc::_exit(0) };
            }
            let _ = std::fs::write(&last, GOOD);
            // SAFETY: handler is async-signal-safe.
            unsafe { libc::signal(libc::SIGTERM, on_term as *const () as usize) };
            sleep(60);
        }
        _ => {
            let _ = std::fs::write(&last, body(GOOD));
            started();
            message(&body(GOOD));
            done();
        }
    }
}

/// Emulates the parts of `claude` the adapter uses. Scenario and recordings live in `$CLAUDE_CONFIG_DIR`.
fn claude(args: &[String]) {
    let home = std::path::PathBuf::from(std::env::var("CLAUDE_CONFIG_DIR").unwrap_or_default());
    let file = |n: &str| {
        std::fs::read_to_string(home.join(n))
            .unwrap_or_default()
            .trim()
            .to_owned()
    };
    let a0 = args.first().map(String::as_str).unwrap_or("");
    if a0 == "--version" {
        let v = file("version");
        out(if v.is_empty() {
            "2.1.319 (Claude Code)"
        } else {
            &v
        });
        return;
    }
    if a0 == "auth" {
        match file("login").as_str() {
            "none" => {
                out("{\n  \"loggedIn\": false\n}");
                std::process::exit(1);
            }
            "boom" => std::process::exit(2),
            "garbage" => out("not json"),
            _ => out("{\n  \"loggedIn\": true,\n  \"authMethod\": \"claude.ai\"\n}"),
        }
        return;
    }
    if a0 != "-p" {
        std::process::exit(64);
    }
    let mut prompt = String::new();
    std::io::stdin().read_to_string(&mut prompt).unwrap();
    let _ = std::fs::write(home.join("argv"), args.join("\n"));
    let _ = std::fs::write(home.join("stdin"), &prompt);
    let _ = std::fs::write(
        home.join("env"),
        std::env::vars()
            .map(|(k, _)| k)
            .collect::<Vec<_>>()
            .join("\n"),
    );
    let _ = std::fs::write(
        home.join("cwd"),
        std::env::current_dir().unwrap().display().to_string(),
    );
    let chat = !args.iter().any(|a| a == "--json-schema");
    let text = if chat { "A reply." } else { GOOD };
    let started = || out(r#"{"type":"system","subtype":"init","claude_code_version":"2.1.319"}"#);
    let assistant = |t: &str| {
        out(&serde_json::json!({"type":"assistant","message":{"content":[{"type":"text","text":t}]}}).to_string())
    };
    let result = |is_error: bool, t: &str, structured: bool, status: Option<i64>| {
        let mut v = serde_json::json!({"type":"result","subtype":"success","is_error":is_error,
            "terminal_reason": if is_error {"api_error"} else {"completed"},"result":t,"api_error_status":status});
        if structured && let Ok(o) = serde_json::from_str::<serde_json::Value>(t) {
            v["structured_output"] = o;
        }
        out(&v.to_string());
    };
    let fail = |m: &str, status: Option<i64>| {
        started();
        assistant(m);
        result(true, m, false, status);
        std::process::exit(1);
    };
    match file("scenario").as_str() {
        "malformed" => {
            started();
            result(false, "not json", false, None);
        }
        "unstructured" => {
            started();
            result(false, GOOD, false, None);
        }
        "fenced" => {
            started();
            let t = format!("```json\n{GOOD}\n```");
            result(false, &t, true, None);
        }
        "invalid-structured" => {
            started();
            // structured_output is an object, but the text is not the closed schema-1 object.
            out(
                r#"{"type":"result","subtype":"success","is_error":false,"terminal_reason":"completed","result":"{\"draft_markdown\":\"\"}","structured_output":{"draft_markdown":""}}"#,
            );
        }
        "no-result" => {
            started();
            assistant(text);
        }
        "split" => {
            started();
            let line = serde_json::json!({"type":"result","subtype":"success","is_error":false,"terminal_reason":"completed","result":text,"structured_output": if chat {serde_json::Value::Null} else {serde_json::from_str(GOOD).unwrap()}}).to_string();
            let (a, b) = line.split_at(line.len() / 2);
            let mut o = std::io::stdout().lock();
            let _ = o.write_all(a.as_bytes());
            let _ = o.flush();
            std::thread::sleep(Duration::from_millis(150));
            let _ = writeln!(o, "{b}");
        }
        "unknown-events" => {
            started();
            out(r#"{"type":"mystery"}"#);
            assistant(text);
            result(false, text, !chat, None);
        }
        "unauthorized" => fail("Not logged in \u{b7} Please run /login", None),
        "ratelimit" => fail("You've hit your usage limit", Some(429)),
        "context" => fail("Prompt is too long: exceeds the context window", None),
        "otherfail" => fail("something odd", None),
        "silent-exec" => {
            started();
            sleep(60);
        }
        "late" => {
            started();
            extern "C" fn on_term(_: libc::c_int) {
                let msg = b"{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"terminal_reason\":\"completed\",\"result\":\"late\"}\n";
                // SAFETY: write(2) and _exit are async-signal-safe.
                unsafe { libc::write(1, msg.as_ptr().cast(), msg.len()) };
                unsafe { libc::_exit(0) };
            }
            // SAFETY: handler is async-signal-safe.
            unsafe { libc::signal(libc::SIGTERM, on_term as *const () as usize) };
            sleep(60);
        }
        _ => {
            started();
            if chat {
                for piece in ["A ", "reply."] {
                    out(&serde_json::json!({"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":piece}}}).to_string());
                }
            }
            assistant(text);
            result(false, text, !chat, None);
        }
    }
}

/// Emulates the parts of `qwen` the adapter uses. Scenario and recordings live in `$QWEN_HOME`.
fn qwen(args: &[String]) {
    let home = std::path::PathBuf::from(std::env::var("QWEN_HOME").unwrap_or_default());
    let file = |n: &str| {
        std::fs::read_to_string(home.join(n))
            .unwrap_or_default()
            .trim()
            .to_owned()
    };
    if args.first().map(String::as_str) == Some("--version") {
        let v = file("version");
        out(if v.is_empty() { "0.26.4" } else { &v });
        return;
    }
    let mut prompt = String::new();
    std::io::stdin().read_to_string(&mut prompt).unwrap();
    let _ = std::fs::write(home.join("argv"), args.join("\n"));
    let _ = std::fs::write(home.join("stdin"), &prompt);
    let _ = std::fs::write(
        home.join("env"),
        std::env::vars()
            .map(|(k, _)| k)
            .collect::<Vec<_>>()
            .join("\n"),
    );
    let _ = std::fs::write(
        home.join("cwd"),
        std::env::current_dir().unwrap().display().to_string(),
    );
    let chat = !args.iter().any(|a| a == "--json-schema");
    let text = if chat { "A reply." } else { GOOD };
    let init = |model: &str, version: &str| {
        out(&serde_json::json!({"type":"system","subtype":"init","model":model,"qwen_code_version":version}).to_string())
    };
    let started = || init("qwen3.6:35b-a3b", "0.26.4");
    let assistant = |t: &str| {
        out(&serde_json::json!({"type":"assistant","message":{"content":[{"type":"text","text":t}]}}).to_string())
    };
    let result = |t: &str, structured: bool| {
        let mut v =
            serde_json::json!({"type":"result","subtype":"success","is_error":false,"result":t});
        if structured && let Ok(o) = serde_json::from_str::<serde_json::Value>(t) {
            v["structured_result"] = o;
        }
        out(&v.to_string());
    };
    // Error results carry `error.message` and no `result`, as the real CLI does.
    let fail = |m: &str, with_init: bool| {
        if with_init {
            started();
        }
        out(&serde_json::json!({"type":"result","subtype":"error_during_execution","is_error":true,"error":{"message":m}}).to_string());
        std::process::exit(1);
    };
    match file("scenario").as_str() {
        "malformed" => {
            started();
            result("not json", false);
        }
        "unstructured" => {
            started();
            result(GOOD, false);
        }
        "fenced" => {
            started();
            result(&format!("```json\n{GOOD}\n```"), true);
        }
        "invalid-structured" => {
            started();
            out(
                r#"{"type":"result","subtype":"success","is_error":false,"result":"{\"draft_markdown\":\"\"}","structured_result":{"draft_markdown":""}}"#,
            );
        }
        "no-result" => {
            started();
            assistant(text);
        }
        "exit-55" => {
            started();
            assistant("x");
            eprintln!(
                "Run aborted: tool-call budget of 0 exceeded (--max-tool-calls); observed 1."
            );
            std::process::exit(55);
        }
        "model-mismatch" => {
            init("some-other-model", "0.26.4");
            result(text, !chat);
        }
        "version-drift" => {
            init("qwen3.6:35b-a3b", "0.27.0");
            result(text, !chat);
        }
        "split" => {
            started();
            let line = serde_json::json!({"type":"result","subtype":"success","is_error":false,"result":text,"structured_result": if chat {serde_json::Value::Null} else {serde_json::from_str(GOOD).unwrap()}}).to_string();
            let (a, b) = line.split_at(line.len() / 2);
            let mut o = std::io::stdout().lock();
            let _ = o.write_all(a.as_bytes());
            let _ = o.flush();
            std::thread::sleep(Duration::from_millis(150));
            let _ = writeln!(o, "{b}");
        }
        "unknown-events" => {
            started();
            out(r#"{"type":"mystery"}"#);
            assistant(text);
            result(text, !chat);
        }
        "unauthorized" => fail(
            "No auth type is selected. Please configure an auth type",
            false,
        ),
        "endpoint-down" => fail("connect ECONNREFUSED 127.0.0.1:8186", true),
        "ratelimit" => fail("429 You've hit your usage limit", true),
        "context" => fail("Prompt exceeds the context window", true),
        "otherfail" => fail("something odd", true),
        "silent-exec" => {
            started();
            sleep(60);
        }
        "late" => {
            started();
            extern "C" fn on_term(_: libc::c_int) {
                let msg = b"{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"result\":\"late\"}\n";
                // SAFETY: write(2) and _exit are async-signal-safe.
                unsafe { libc::write(1, msg.as_ptr().cast(), msg.len()) };
                unsafe { libc::_exit(0) };
            }
            // SAFETY: handler is async-signal-safe.
            unsafe { libc::signal(libc::SIGTERM, on_term as *const () as usize) };
            sleep(60);
        }
        _ => {
            started();
            if chat {
                // Reasoning deltas must not reach the sink; only answer text does.
                out(
                    r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"thinking_delta","thinking":"hmm"}}}"#,
                );
                for piece in ["A ", "reply."] {
                    out(&serde_json::json!({"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":piece}}}).to_string());
                }
            }
            assistant(text);
            result(text, !chat);
        }
    }
}
