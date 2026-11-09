use super::create_runtime;
use crate::js::websocket::{open_sockets, WsNet, WsResolver};
use dcl_types::{interface::crdt_context::CrdtContext, SceneId, SceneResourceCounters};
use deno_core::{anyhow::Error as AnyError, futures::future::LocalBoxFuture, JsRuntime};
use std::{
    cell::Cell,
    io::{ErrorKind, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    rc::Rc,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

const MIB: usize = 1024 * 1024;

// ---- recording loopback server ----

trait Conn: Read + Write + Send {}
impl<T: Read + Write + Send> Conn for T {}

#[derive(Default)]
struct ServerLog {
    connections: usize,
    /// Raw upgrade requests, one per handshake the server answered.
    handshakes: Vec<String>,
}

/// A loopback WebSocket server whose behaviour is chosen by the request path. It records
/// every TCP connection and every upgrade request it answers.
struct WsServer {
    addr: SocketAddr,
    log: Arc<Mutex<ServerLog>>,
    stop: Arc<AtomicBool>,
}

impl WsServer {
    fn start() -> Self {
        Self::start_with(None)
    }

    fn start_with(tls: Option<native_tls::TlsAcceptor>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let addr = listener.local_addr().unwrap();
        let log = Arc::new(Mutex::new(ServerLog::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let (thread_log, thread_stop) = (log.clone(), stop.clone());
        std::thread::spawn(move || {
            while !thread_stop.load(Ordering::SeqCst) {
                let Ok((stream, _)) = listener.accept() else {
                    std::thread::sleep(Duration::from_millis(2));
                    continue;
                };
                thread_log.lock().unwrap().connections += 1;
                let (log, stop, tls) = (thread_log.clone(), thread_stop.clone(), tls.clone());
                std::thread::spawn(move || serve(stream, tls, log, stop));
            }
        });
        Self { addr, log, stop }
    }

    fn url(&self, path: &str) -> String {
        format!("ws://{}{path}", self.addr)
    }

    fn connections(&self) -> usize {
        self.log.lock().unwrap().connections
    }

    fn handshakes(&self) -> Vec<String> {
        self.log.lock().unwrap().handshakes.clone()
    }
}

impl Drop for WsServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

fn serve(
    stream: TcpStream,
    tls: Option<native_tls::TlsAcceptor>,
    log: Arc<Mutex<ServerLog>>,
    stop: Arc<AtomicBool>,
) {
    stream.set_nonblocking(false).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_millis(50)))
        .unwrap();
    let mut conn: Box<dyn Conn> = match tls {
        Some(acceptor) => match acceptor.accept(stream) {
            Ok(tls) => Box::new(tls),
            Err(_) => return,
        },
        None => Box::new(stream),
    };
    let Some(request) = read_request(&mut *conn, &stop) else {
        return;
    };
    let path = request
        .split_whitespace()
        .nth(1)
        .unwrap_or("/")
        .to_owned();
    if path == "/stall" {
        hold(&stop);
        return;
    }
    let header = |name: &str| {
        request.lines().find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.eq_ignore_ascii_case(name).then(|| value.trim().to_owned())
        })
    };
    let accept = tokio_tungstenite::tungstenite::handshake::derive_accept_key(
        header("sec-websocket-key").unwrap_or_default().as_bytes(),
    );
    let protocol = header("sec-websocket-protocol")
        .and_then(|p| p.split(',').next().map(|p| p.trim().to_owned()))
        .map(|p| format!("Sec-WebSocket-Protocol: {p}\r\n"))
        .unwrap_or_default();
    log.lock().unwrap().handshakes.push(request.clone());
    let response = format!(
        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n{protocol}\r\n"
    );
    if conn.write_all(response.as_bytes()).is_err() {
        return;
    }

    let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    match parts.as_slice() {
        ["echo"] => echo(&mut *conn, &stop),
        ["frame", n] => {
            let _ = write_frame(&mut *conn, true, 2, &vec![7; n.parse().unwrap()]);
            echo(&mut *conn, &stop);
        }
        ["fragments", a, b] => {
            let _ = write_frame(&mut *conn, false, 2, &vec![1; a.parse().unwrap()]);
            let _ = write_frame(&mut *conn, true, 0, &vec![2; b.parse().unwrap()]);
            echo(&mut *conn, &stop);
        }
        ["close", code] => {
            let payload = match code.parse::<u16>() {
                Ok(code) => [&code.to_be_bytes()[..], b"bye"].concat(),
                Err(_) => Vec::new(),
            };
            let _ = write_frame(&mut *conn, true, 8, &payload);
            echo(&mut *conn, &stop);
        }
        ["drop"] => {}
        // reads everything and never answers, a close frame included
        ["silent"] => {
            let mut buf = [0u8; 4096];
            while !stop.load(Ordering::SeqCst) {
                match conn.read(&mut buf) {
                    Ok(0) => return,
                    Ok(_) => {}
                    Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
                    Err(_) => return,
                }
            }
        }
        // never reads
        ["sink"] => hold(&stop),
        _ => {}
    }
}

fn hold(stop: &AtomicBool) {
    while !stop.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn read_exact_or_stop(conn: &mut dyn Conn, buf: &mut [u8], stop: &AtomicBool) -> bool {
    let mut filled = 0;
    while filled < buf.len() {
        if stop.load(Ordering::SeqCst) {
            return false;
        }
        match conn.read(&mut buf[filled..]) {
            Ok(0) => return false,
            Ok(n) => filled += n,
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(_) => return false,
        }
    }
    true
}

fn read_request(conn: &mut dyn Conn, stop: &AtomicBool) -> Option<String> {
    let mut request = Vec::new();
    let mut byte = [0u8; 1];
    while !request.ends_with(b"\r\n\r\n") {
        if !read_exact_or_stop(conn, &mut byte, stop) || request.len() > 16 * 1024 {
            return None;
        }
        request.push(byte[0]);
    }
    String::from_utf8(request).ok()
}

fn read_frame(conn: &mut dyn Conn, stop: &AtomicBool) -> Option<(u8, Vec<u8>)> {
    let mut head = [0u8; 2];
    if !read_exact_or_stop(conn, &mut head, stop) {
        return None;
    }
    let opcode = head[0] & 0x0f;
    let len = match head[1] & 0x7f {
        126 => {
            let mut n = [0u8; 2];
            read_exact_or_stop(conn, &mut n, stop).then(|| u16::from_be_bytes(n) as usize)?
        }
        127 => {
            let mut n = [0u8; 8];
            read_exact_or_stop(conn, &mut n, stop).then(|| u64::from_be_bytes(n) as usize)?
        }
        n => n as usize,
    };
    let mut mask = [0u8; 4];
    if head[1] & 0x80 != 0 && !read_exact_or_stop(conn, &mut mask, stop) {
        return None;
    }
    let mut payload = vec![0u8; len];
    if !read_exact_or_stop(conn, &mut payload, stop) {
        return None;
    }
    for (i, b) in payload.iter_mut().enumerate() {
        *b ^= mask[i % 4];
    }
    Some((opcode, payload))
}

fn write_frame(conn: &mut dyn Conn, fin: bool, opcode: u8, payload: &[u8]) -> std::io::Result<()> {
    let mut frame = vec![if fin { 0x80 } else { 0 } | opcode];
    match payload.len() {
        n if n < 126 => frame.push(n as u8),
        n if n <= u16::MAX as usize => {
            frame.push(126);
            frame.extend_from_slice(&(n as u16).to_be_bytes());
        }
        n => {
            frame.push(127);
            frame.extend_from_slice(&(n as u64).to_be_bytes());
        }
    }
    frame.extend_from_slice(payload);
    conn.write_all(&frame)
}

/// Echoes data frames, answers pings and returns after answering a close.
fn echo(conn: &mut dyn Conn, stop: &AtomicBool) {
    while let Some((opcode, payload)) = read_frame(conn, stop) {
        let reply = match opcode {
            9 => 10,
            8 => {
                let _ = write_frame(conn, true, 8, &payload);
                return;
            }
            other => other,
        };
        if write_frame(conn, true, reply, &payload).is_err() {
            return;
        }
    }
}

// ---- resolver seam ----

/// Answers every lookup with the next of `answers` (the last one repeats) and counts calls.
struct ScriptedResolver {
    answers: Vec<Vec<SocketAddr>>,
    calls: Rc<Cell<usize>>,
}

impl WsResolver for ScriptedResolver {
    fn resolve(
        &self,
        _host: &str,
        _port: u16,
        _allow_private: bool,
    ) -> LocalBoxFuture<'static, Result<Vec<SocketAddr>, AnyError>> {
        let call = self.calls.get();
        self.calls.set(call + 1);
        let answer = self.answers[call.min(self.answers.len() - 1)].clone();
        Box::pin(async move { Ok(answer) })
    }
}

// ---- scene harness ----

#[derive(Clone, Copy)]
enum Scene {
    /// A deployed parcel scene on the desktop client.
    Ordinary,
    /// The bundled HUD or an operator system scene on the desktop client.
    DesktopSystem,
    /// An ordinary scene in preview mode.
    Preview,
    /// A super-user scene on the authoritative server.
    HostedSystem,
}

const PRELUDE: &str = r#"
function connect(url, protocols) {
    return new Promise((resolve) => {
        const ws = new WebSocket(url, protocols);
        let error = null;
        ws.addEventListener("open", () => resolve({ ws, ok: true }));
        ws.addEventListener("error", (e) => { error = e.message; });
        ws.addEventListener("close", (e) => resolve({ ws, ok: false, error, code: e.code }));
    });
}
function closed(ws) { return new Promise((r) => ws.addEventListener("close", (e) => r(e))); }
function message(ws) { return new Promise((r) => ws.addEventListener("message", (e) => r(e.data), { once: true })); }
async function close(ws, code, reason) { const c = closed(ws); ws.close(code, reason); return await c; }
function track(ws) {
    const log = [];
    for (const type of ["open", "error", "close"]) {
        ws.addEventListener(type, (e) => log.push(type === "close" ? `close:${e.code}:${e.wasClean}` : type));
    }
    return log;
}
"#;

struct Harness {
    runtime: JsRuntime,
    rt: tokio::runtime::Runtime,
    grant: bool,
    /// The message of every permission prompt the scene raised.
    prompts: Vec<Option<String>>,
    _system: Option<tokio::sync::mpsc::UnboundedReceiver<system_bridge::SystemApi>>,
}

impl Harness {
    fn new(scene: Scene) -> Self {
        let super_user = matches!(scene, Scene::DesktopSystem | Scene::HostedSystem);
        let (mut runtime, _) =
            create_runtime(false, super_user, "websocket-test", Default::default());
        let state = runtime.op_state();
        let mut system = None;
        {
            let mut state = state.borrow_mut();
            state.put(CrdtContext::new(
                SceneId(bevy::prelude::Entity::from_raw(0)),
                "hash".to_owned(),
                "title".to_owned(),
                false,
                matches!(scene, Scene::Preview),
                matches!(scene, Scene::HostedSystem),
            ));
            state.put(SceneResourceCounters::default());
            state.put(dcl_types::RpcCalls::default());
            state.put(Vec::<dcl_types::SceneLogMessage>::default());
            state.put(dcl_types::SceneElapsedTime(0.0));
            if super_user {
                let (sx, rx) = tokio::sync::mpsc::unbounded_channel();
                state.put(dcl_js::SuperUserScene(sx));
                system = Some(rx);
            }
        }
        runtime.execute_script("<prelude>", PRELUDE).unwrap();
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        Self {
            runtime,
            rt,
            grant: true,
            prompts: Vec::new(),
            _system: system,
        }
    }

    fn with_net(mut self, net: WsNet) -> Self {
        self.runtime.op_state().borrow_mut().put(net);
        self
    }

    fn start(&mut self, body: &str) {
        let script = format!(
            "globalThis.__result = 'pending'; (async () => {{ {body} }})().then((x) => {{ __result = String(x); }}, (e) => {{ __result = 'threw ' + String(e); }});"
        );
        // async ops spawn onto the current runtime when called
        let _runtime = self.rt.enter();
        self.runtime.execute_script("<test>", script).unwrap();
    }

    /// Runs the event loop, answering permission prompts, until it has nothing left to do or
    /// `limit` passes. Returns whether it finished.
    fn drive(&mut self, limit: Duration) -> bool {
        let state = self.runtime.op_state();
        let grant = self.grant;
        let runtime = &mut self.runtime;
        let prompts = &mut self.prompts;
        self.rt.block_on(async {
            let answer = async {
                loop {
                    tokio::time::sleep(Duration::from_millis(2)).await;
                    let calls = std::mem::take(
                        &mut **state.borrow_mut().borrow_mut::<dcl_types::RpcCalls>(),
                    );
                    for call in calls {
                        if let common::rpc::RpcCall::RequestGenericPermission {
                            response,
                            message,
                            ..
                        } = call
                        {
                            prompts.push(message);
                            std::thread::spawn(move || response.send(grant));
                        }
                    }
                }
            };
            tokio::select! {
                r = tokio::time::timeout(limit, runtime.run_event_loop(Default::default())) => match r {
                    Ok(r) => { r.unwrap(); true }
                    Err(_) => false,
                },
                _ = answer => unreachable!(),
            }
        })
    }

    fn run(&mut self, body: &str) -> String {
        self.start(body);
        if !self.drive(Duration::from_secs(20)) {
            let result = self.eval("String(globalThis.__result)");
            panic!("the scene did not settle; result: {result}");
        }
        self.eval("String(globalThis.__result)")
    }

    fn eval(&mut self, script: &str) -> String {
        let _runtime = self.rt.enter();
        let value = self
            .runtime
            .execute_script("<eval>", script.to_owned())
            .unwrap();
        let scope = &mut self.runtime.handle_scope();
        deno_core::v8::Local::new(scope, value).to_rust_string_lossy(scope)
    }

    fn counters(&mut self) -> SceneResourceCounters {
        self.runtime
            .op_state()
            .borrow()
            .borrow::<SceneResourceCounters>()
            .clone()
    }

    fn sockets(&mut self) -> u64 {
        open_sockets(&self.runtime.op_state().borrow())
    }
}

/// Opens, round-trips one text message and closes cleanly; `refused: <error>` otherwise.
fn echo_outcome(scene: Scene, url: &str) -> String {
    Harness::new(scene).run(&format!(
        r#"
        const {{ ws, ok, error }} = await connect({url:?});
        if (!ok) return "refused: " + error;
        ws.send("ping");
        const reply = await message(ws);
        const event = await close(ws, 1000, "done");
        return `${{reply}} ${{event.code}} ${{event.wasClean}}`;
        "#
    ))
}

// ---- access policy ----

#[test]
fn private_network_access_follows_the_fetch_rule() {
    let server = WsServer::start();
    let url = server.url("/echo");
    assert_eq!(echo_outcome(Scene::DesktopSystem, &url), "ping 1000 true");
    assert_eq!(echo_outcome(Scene::Preview, &url), "ping 1000 true");
    for scene in [Scene::Ordinary, Scene::HostedSystem] {
        let outcome = echo_outcome(scene, &url);
        assert!(outcome.contains("must be `wss`"), "{outcome}");
        // wss passes the scheme rule and is refused at the resolved address instead
        let mut harness = Harness::new(scene);
        let outcome = harness.run(&format!(
            "const r = await connect('wss://localhost:{}/echo'); return r.ok ? 'open' : r.error;",
            server.addr.port()
        ));
        assert!(outcome.contains("non-public"), "{outcome}");
        let counters = harness.counters();
        assert_eq!((counters.ws_refused, counters.ws_failed), (1, 0));
    }
    // only the two permitted scenes ever reached the server
    assert_eq!(server.connections(), 2);
    assert_eq!(server.handshakes().len(), 2);
}

#[test]
fn link_local_and_metadata_stay_blocked_for_privileged_scenes() {
    let server = WsServer::start();
    let metadata: SocketAddr = format!("169.254.169.254:{}", server.addr.port())
        .parse()
        .unwrap();
    for url in [
        "ws://169.254.169.254/latest/meta-data/",
        "ws://[fe80::1]:80/",
        "ws://[fd00:ec2::254]/",
    ] {
        let outcome = echo_outcome(Scene::DesktopSystem, url);
        assert!(outcome.contains("non-public"), "{url}: {outcome}");
    }
    // a name that resolves to metadata is refused before it is dialled
    let calls = Rc::new(Cell::new(0));
    let mut harness = Harness::new(Scene::DesktopSystem).with_net(WsNet {
        resolver: Rc::new(ScriptedResolver {
            answers: vec![vec![metadata]],
            calls: calls.clone(),
        }),
        ..WsNet::default()
    });
    let outcome = harness.run(&format!(
        "const r = await connect('ws://metadata.test:{}/echo'); return r.ok ? 'open' : r.error;",
        server.addr.port()
    ));
    assert!(outcome.contains("non-public"), "{outcome}");
    assert_eq!(calls.get(), 1);
    assert_eq!(harness.counters().ws_refused, 1);
    // the same scene reaches the loopback server
    assert_eq!(
        echo_outcome(Scene::DesktopSystem, &server.url("/echo")),
        "ping 1000 true"
    );
    assert_eq!(server.connections(), 1);
}

#[test]
fn one_resolution_per_socket_and_only_that_answer_is_dialled() {
    let first = WsServer::start();
    let second = WsServer::start();
    let resolver = |calls: &Rc<Cell<usize>>| {
        Rc::new(ScriptedResolver {
            answers: vec![vec![first.addr], vec![second.addr]],
            calls: calls.clone(),
        })
    };
    let calls = Rc::new(Cell::new(0));
    let mut harness = Harness::new(Scene::DesktopSystem).with_net(WsNet {
        resolver: resolver(&calls),
        ..WsNet::default()
    });
    let outcome = harness.run(
        r#"
        const { ws, ok, error } = await connect("ws://rebind.test/echo");
        if (!ok) return error;
        ws.send("x");
        await message(ws);
        return (await close(ws)).code;
        "#,
    );
    assert_eq!(outcome, "1005");
    assert_eq!(calls.get(), 1);
    assert_eq!(first.handshakes().len(), 1);
    assert!(first.handshakes()[0].contains("\r\nHost: rebind.test\r\n"));
    assert_eq!(second.connections(), 0);

    // a scene without private access is refused on the first answer and dials nothing
    let calls = Rc::new(Cell::new(0));
    let mut harness = Harness::new(Scene::Ordinary).with_net(WsNet {
        resolver: resolver(&calls),
        ..WsNet::default()
    });
    let outcome =
        harness.run("const r = await connect('wss://rebind.test/echo'); return r.error;");
    assert!(outcome.contains("non-public"), "{outcome}");
    assert_eq!(calls.get(), 1);
    assert_eq!(first.connections(), 1);
    assert_eq!(second.connections(), 0);
}

#[test]
fn a_denied_permission_never_reaches_the_server() {
    let server = WsServer::start();
    let mut harness = Harness::new(Scene::DesktopSystem);
    harness.grant = false;
    let outcome = harness.run(&format!(
        "const r = await connect({:?}); return r.ok ? 'open' : r.error + ' ' + r.code;",
        server.url("/echo")
    ));
    assert!(outcome.contains("User denied"), "{outcome}");
    assert!(outcome.ends_with(" 1006"), "{outcome}");
    assert_eq!(server.connections(), 0);
    assert_eq!(harness.counters().ws_refused, 1);
    assert_eq!(harness.sockets(), 0);
    harness.grant = true;
    let outcome = harness.run(&format!(
        "const r = await connect({:?}); return r.ok ? (await close(r.ws)).code : r.error;",
        server.url("/echo")
    ));
    assert_eq!(outcome, "1005");
    assert_eq!(server.handshakes().len(), 1);
}

// ---- limits and lifecycle ----

#[test]
fn inbound_messages_of_one_mebibyte_close_the_socket() {
    let server = WsServer::start();
    let receive = |path: &str| {
        Harness::new(Scene::DesktopSystem).run(&format!(
            r#"
            const {{ ws, ok, error }} = await connect({:?});
            if (!ok) return "refused " + error;
            ws.binaryType = "arraybuffer";
            const outcome = await Promise.race([
                message(ws).then((data) => "message " + data.byteLength),
                closed(ws).then((e) => "closed " + e.code),
            ]);
            if (ws.readyState !== WebSocket.CLOSED) await close(ws);
            return outcome;
            "#,
            server.url(path)
        ))
    };
    assert_eq!(receive(&format!("/frame/{}", MIB - 1)), format!("message {}", MIB - 1));
    assert_eq!(receive(&format!("/frame/{MIB}")), "closed 1006");
    let part = 400 * 1024;
    assert_eq!(
        receive(&format!("/fragments/{part}/{part}")),
        format!("message {}", 2 * part)
    );
    let part = 600 * 1024;
    assert_eq!(receive(&format!("/fragments/{part}/{part}")), "closed 1006");
}

#[test]
fn the_33rd_socket_is_refused() {
    let server = WsServer::start();
    let mut harness = Harness::new(Scene::DesktopSystem);
    let outcome = harness.run(&format!(
        r#"
        const url = {:?};
        const open = [];
        for (let i = 0; i < 32; i++) {{
            const r = await connect(url);
            if (!r.ok) return `socket ${{i}}: ${{r.error}}`;
            open.push(r.ws);
        }}
        const refused = await connect(url);
        await close(open.pop());
        const again = await connect(url);
        open.push(again.ws);
        for (const ws of open) await close(ws);
        return `${{refused.ok}} ${{refused.error}} | ${{again.ok}}`;
        "#,
        server.url("/echo")
    ));
    assert!(outcome.starts_with("false "), "{outcome}");
    assert!(outcome.contains("32 sockets"), "{outcome}");
    assert!(outcome.ends_with("| true"), "{outcome}");
    assert_eq!(server.handshakes().len(), 33);
    assert_eq!(harness.counters().ws_refused, 1);
    assert_eq!(harness.counters().ws_opened, 33);
    assert_eq!(harness.sockets(), 0);
}

#[test]
fn a_stalled_handshake_times_out_and_frees_its_slot() {
    let server = WsServer::start();
    let mut harness = Harness::new(Scene::DesktopSystem).with_net(WsNet {
        handshake_timeout: Duration::from_millis(300),
        ..WsNet::default()
    });
    let outcome = harness.run(&format!(
        "const r = await connect({:?}); return r.ok ? 'open' : r.error + ' ' + r.code;",
        server.url("/stall")
    ));
    assert!(outcome.contains("timed out"), "{outcome}");
    assert!(outcome.ends_with(" 1006"), "{outcome}");
    assert_eq!(harness.counters().ws_failed, 1);
    assert_eq!(harness.sockets(), 0);
    assert_eq!(server.handshakes().len(), 0);
}

#[test]
fn closing_while_connecting_cancels_the_handshake() {
    let server = WsServer::start();
    let mut harness = Harness::new(Scene::DesktopSystem);
    let started = Instant::now();
    let outcome = harness.run(&format!(
        r#"
        const ws = new WebSocket({:?});
        const events = [];
        ws.onopen = () => events.push("open");
        ws.onerror = () => events.push("error");
        const event = await close(ws);
        return events.join(",") + " " + event.code;
        "#,
        server.url("/stall")
    ));
    assert_eq!(outcome, "error 1006");
    assert!(started.elapsed() < Duration::from_secs(5));
    assert_eq!(harness.sockets(), 0);
}

#[test]
fn a_send_past_the_outbound_cap_fails_the_socket() {
    let server = WsServer::start();
    let mut harness = Harness::new(Scene::DesktopSystem);
    let outcome = harness.run(&format!(
        r#"
        const chunk = new Uint8Array({MIB});
        const stalled = (await connect({sink:?})).ws;
        const stalledLog = track(stalled);
        const stalledEnded = closed(stalled);
        let sent = 0, buffered = 0, threw = "no";
        try {{
            while (stalled.readyState === WebSocket.OPEN && sent < 64) {{
                buffered = stalled.bufferedAmount;
                stalled.send(chunk);
                sent++;
            }}
        }} catch (e) {{ threw = String(e); }}
        await stalledEnded;

        const echo = (await connect({echo:?})).ws;
        const echoLog = track(echo);
        const echoEnded = closed(echo);
        echo.send(new Uint8Array(8 * {MIB} + 1));
        await echoEnded;

        const fresh = (await connect({echo:?})).ws;
        fresh.send(new Uint8Array(1000));
        const echoed = (await message(fresh)).size;
        await close(fresh);
        return [sent, buffered, threw, stalledLog.join(","), echoLog.join(","), echoed].join(" | ");
        "#,
        sink = server.url("/sink"),
        echo = server.url("/echo")
    ));
    let parts: Vec<&str> = outcome.split(" | ").collect();
    assert_eq!(parts.len(), 6, "{outcome}");
    // the eighth MiB passes the cap and fails the socket; send() itself never throws
    assert_eq!(parts[0], "8", "{outcome}");
    assert_eq!(parts[1], (7 * MIB).to_string());
    assert_eq!(parts[2], "no");
    assert_eq!(parts[3], "error,close:1006:false");
    assert_eq!(parts[4], "error,close:1006:false");
    assert_eq!(parts[5], "1000");
    assert_eq!(harness.sockets(), 0);
}

#[test]
fn a_flood_of_empty_sends_is_capped() {
    let server = WsServer::start();
    let mut harness = Harness::new(Scene::DesktopSystem);
    let outcome = harness.run(&format!(
        r#"
        const ws = (await connect({:?})).ws;
        const log = track(ws);
        const ended = closed(ws);
        let sent = 0;
        while (ws.readyState === WebSocket.OPEN && sent < 1000000) {{
            ws.send("");
            sent++;
        }}
        await ended;
        return [sent, log.join(",")].join(" | ");
        "#,
        server.url("/sink")
    ));
    let (sent, log) = outcome.split_once(" | ").unwrap();
    let sent: usize = sent.parse().unwrap();
    // every queued message counts against the 8 MiB cap, payload or not
    let queued = harness.counters().ws_msgs_out as usize;
    assert!(queued > 1000 && queued < 8 * MIB / 32, "{queued}");
    assert_eq!(sent, queued + 1);
    assert_eq!(log, "error,close:1006:false");
    assert_eq!(harness.sockets(), 0);
}

#[test]
fn a_queued_blob_past_the_cap_fails_the_socket_and_drops_later_sends() {
    let server = WsServer::start();
    let mut harness = Harness::new(Scene::DesktopSystem);
    let outcome = harness.run(&format!(
        r#"
        const {{ ws }} = await connect({:?});
        ws.send(new Uint8Array([1]));
        const Blob = (await message(ws)).constructor;
        const log = track(ws);
        const received = [];
        ws.addEventListener("message", (e) => received.push(e.data));
        let threw = "no";
        try {{
            ws.send(new Blob([new Uint8Array(8 * {MIB} + 1)]));
            ws.send("after");
        }} catch (e) {{ threw = String(e); }}
        await closed(ws);
        return [threw, log.join(","), received.length].join(" | ");
        "#,
        server.url("/echo")
    ));
    assert_eq!(outcome, "no | error,close:1006:false | 0");
    assert_eq!(harness.sockets(), 0);
}

#[test]
fn buffered_amount_is_reported_while_closing() {
    let server = WsServer::start();
    let mut harness = Harness::new(Scene::DesktopSystem).with_net(WsNet {
        close_timeout: Duration::from_millis(300),
        ..WsNet::default()
    });
    let outcome = harness.run(&format!(
        r#"
        const {{ ws }} = await connect({:?});
        ws.send(new Uint8Array(1000));
        const c = closed(ws);
        ws.close();
        const closing = [ws.readyState, ws.bufferedAmount];
        const event = await c;
        return [...closing, ws.readyState, event.code].join(" ");
        "#,
        server.url("/silent")
    ));
    assert_eq!(outcome, "2 1000 3 1006");
}

#[test]
fn a_peer_dropping_the_connection_reports_error_then_close() {
    let server = WsServer::start();
    let outcome = Harness::new(Scene::DesktopSystem).run(&format!(
        r#"
        const ws = new WebSocket({:?});
        const log = track(ws);
        await closed(ws);
        return log.join(",");
        "#,
        server.url("/drop")
    ));
    assert_eq!(outcome, "open,error,close:1006:false");
}

#[test]
fn closing_while_connecting_fails_even_once_the_connection_completed() {
    let server = WsServer::start();
    let mut harness = Harness::new(Scene::DesktopSystem);
    // b's open can run before a's connect result reaches the scene; closing a then must still
    // report a failed connection, not a clean close
    let outcome = harness.run(&format!(
        r#"
        const url = {:?};
        const settle = (ws) => new Promise((r) => {{
            if (ws.readyState === WebSocket.OPEN || ws.readyState === WebSocket.CLOSED) r();
            ws.addEventListener("open", () => r());
            ws.addEventListener("close", () => r());
        }});
        const bad = [];
        for (let round = 0; round < 3; round++) {{
        const pairs = [];
        for (let i = 0; i < 16; i++) {{
            const a = new WebSocket(url), b = new WebSocket(url);
            const pair = {{ a, b, log: track(a), early: false }};
            b.addEventListener("open", () => {{
                if (a.readyState === WebSocket.CONNECTING) {{ pair.early = true; a.close(1000); }}
            }});
            pairs.push(pair);
        }}
        for (const pair of pairs) {{
            await settle(pair.a);
            await settle(pair.b);
            if (pair.early) {{
                if (pair.a.readyState !== WebSocket.CLOSED) await closed(pair.a);
                if (pair.log.join(",") !== "error,close:1006:false") bad.push(pair.log.join(","));
            }}
            for (const ws of [pair.a, pair.b]) if (ws.readyState === WebSocket.OPEN) await close(ws);
        }}
        }}
        return bad.join(";");
        "#,
        server.url("/echo")
    ));
    assert_eq!(outcome, "");
    assert_eq!(harness.sockets(), 0);
}

#[test]
fn the_permission_prompt_names_the_parsed_origin() {
    let server = WsServer::start();
    let port = server.addr.port();
    let mut harness = Harness::new(Scene::DesktopSystem);
    let outcome = harness.run(&format!(
        "const r = await connect('ws://decentraland.org:x@127.0.0.1:{port}/echo?q=1'); if (r.ok) await close(r.ws); return r.ok;"
    ));
    assert_eq!(outcome, "true");
    let mut ordinary = Harness::new(Scene::Ordinary);
    ordinary.run("await connect('wss://[::1]/echo');");
    assert_eq!(
        [harness.prompts, ordinary.prompts].concat(),
        vec![
            Some(format!("ws://127.0.0.1:{port}")),
            Some("wss://[::1]:443".to_owned())
        ]
    );
}

#[test]
fn an_unanswered_close_holds_the_slot_until_the_close_timeout() {
    let server = WsServer::start();
    let mut harness = Harness::new(Scene::DesktopSystem).with_net(WsNet {
        close_timeout: Duration::from_millis(1500),
        ..WsNet::default()
    });
    harness.start(&format!(
        r#"
        const {{ ws }} = await connect({:?});
        globalThis.__ws = ws;
        const event = await close(ws, 3000, "bye");
        return event.code + " " + event.wasClean;
        "#,
        server.url("/silent")
    ));
    assert!(!harness.drive(Duration::from_millis(500)));
    assert_eq!(harness.eval("String(__ws.readyState)"), "2");
    assert_eq!(harness.sockets(), 1);
    assert!(harness.drive(Duration::from_secs(5)));
    assert_eq!(harness.eval("String(__result)"), "1006 false");
    assert_eq!(harness.sockets(), 0);

    // an answered close finishes at once
    let started = Instant::now();
    assert_eq!(
        echo_outcome(Scene::DesktopSystem, &server.url("/echo")),
        "ping 1000 true"
    );
    assert!(started.elapsed() < Duration::from_millis(1500));
}

// ---- protocol ----

#[test]
fn self_signed_wss_is_refused() {
    let dir = std::env::temp_dir().join(format!("dcl-ws-tls-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let status = std::process::Command::new("openssl")
        .args(["req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "1"])
        .args(["-subj", "/CN=localhost", "-keyout"])
        .arg(dir.join("key.pem"))
        .arg("-out")
        .arg(dir.join("cert.pem"))
        .stderr(std::process::Stdio::null())
        .status()
        .expect("the test needs the openssl command to make a self-signed certificate");
    assert!(status.success());
    let identity = native_tls::Identity::from_pkcs8(
        &std::fs::read(dir.join("cert.pem")).unwrap(),
        &std::fs::read(dir.join("key.pem")).unwrap(),
    )
    .unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let server =
        WsServer::start_with(Some(native_tls::TlsAcceptor::new(identity).unwrap()));
    let mut harness = Harness::new(Scene::DesktopSystem);
    let outcome = harness.run(&format!(
        "const r = await connect('wss://localhost:{}/echo'); return r.ok ? 'open' : r.code;",
        server.addr.port()
    ));
    assert_eq!(outcome, "1006");
    assert_eq!(server.connections(), 1);
    assert_eq!(server.handshakes().len(), 0);
    assert_eq!(harness.counters().ws_failed, 1);
}

#[test]
fn server_close_codes_reach_the_scene_mapped() {
    let server = WsServer::start();
    for (path, expected) in [
        ("/close/1000", "1000 bye"),
        ("/close/3000", "3000 bye"),
        ("/close/4999", "4999 bye"),
        ("/close/1001", "1001 bye"),
        ("/close/999", "1002 "),
        ("/close/1004", "1002 "),
        ("/close/1005", "1002 "),
        ("/close/1006", "1002 "),
        ("/close/1015", "1002 "),
        ("/close/5000", "1002 "),
        ("/close/none", "1005 "),
        ("/drop", "1006 "),
    ] {
        let outcome = Harness::new(Scene::DesktopSystem).run(&format!(
            r#"
            const {{ ws, ok, error }} = await connect({:?});
            if (!ok) return "refused " + error;
            const event = await closed(ws);
            return event.code + " " + (event.code === 1002 ? "" : event.reason);
            "#,
            server.url(path)
        ));
        assert_eq!(outcome, expected, "{path}");
    }
}

#[test]
fn binary_types_round_trip_and_traffic_is_counted() {
    let server = WsServer::start();
    let mut harness = Harness::new(Scene::DesktopSystem);
    let outcome = harness.run(&format!(
        r#"
        const {{ ws, ok, error }} = await connect({:?}, ["chat", "x"]);
        if (!ok) return "refused " + error;
        const out = [ws.protocol];
        ws.send("hi");
        out.push(await message(ws));
        ws.send(new Uint8Array([1, 2, 3]));
        const blob = await message(ws);
        // scenes have no Blob global; a received message is one
        const Blob = blob.constructor;
        out.push(Blob.name, [...new Uint8Array(await blob.arrayBuffer())].join(","));
        ws.binaryType = "arraybuffer";
        ws.send(new Uint8Array([4, 5]).buffer);
        const ab = await message(ws);
        out.push(ab instanceof ArrayBuffer, [...new Uint8Array(ab)].join(","));
        ws.send(new Blob([new Uint8Array([6])]));
        out.push([...new Uint8Array(await message(ws))].join(","));
        await close(ws, 1000);
        return out.join(" ");
        "#,
        server.url("/echo")
    ));
    assert_eq!(outcome, "chat hi Blob 1,2,3 true 4,5 6");
    let request = server.handshakes()[0].to_ascii_lowercase();
    let headers: Vec<&str> = request.lines().collect();
    assert_eq!(
        headers
            .iter()
            .filter(|h| h.starts_with("user-agent:"))
            .copied()
            .collect::<Vec<_>>(),
        vec!["user-agent: dclexplorer/0.1"]
    );
    assert!(headers.contains(&"sec-websocket-protocol: chat, x"));
    assert!(!request.contains("cookie:") && !request.contains("authorization:"));
    let counters = harness.counters();
    assert_eq!(counters.ws_opened, 1);
    assert_eq!(
        (counters.ws_msgs_out, counters.ws_bytes_out),
        (4, 2 + 3 + 2 + 1)
    );
    assert_eq!((counters.ws_msgs_in, counters.ws_bytes_in), (4, 2 + 3 + 2 + 1));
    assert_eq!((counters.ws_refused, counters.ws_failed), (0, 0));
}

#[test]
fn direct_op_calls_are_validated() {
    let server = WsServer::start();
    let mut harness = Harness::new(Scene::DesktopSystem);
    let outcome = harness.run(&format!(
        r#"
        const ops = Deno.core.ops;
        const rid = ops.op_ws_create({:?}, "");
        await ops.op_ws_connect(rid);
        const refused = [];
        for (const [code, reason] of [[999], [1001], [2999], [5000], [3000.5], [70000], [NaN], ["1000"], [1000, "x".repeat(124)]]) {{
            try {{ ops.op_ws_close(rid, code, reason); refused.push("accepted " + code); }}
            catch {{ refused.push("refused"); }}
        }}
        let reconnect = "accepted";
        try {{ await ops.op_ws_connect(rid); }} catch (e) {{ reconnect = String(e); }}
        ops.op_ws_close(rid, 4000, "ok");
        let sent = "accepted";
        try {{ ops.op_ws_send_text(rid, "late"); }} catch (e) {{ sent = String(e); }}
        const kind = await ops.op_ws_next_event(rid);
        const code = ops.op_ws_get_close_code(rid);
        Deno.core.close(rid);
        return [refused.join(","), reconnect.includes("not connecting"), sent.includes("closing"), kind, code].join(" | ");
        "#,
        server.url("/echo")
    ));
    assert_eq!(
        outcome,
        "refused,refused,refused,refused,refused,refused,refused,refused,refused | true | true | 2 | 4000"
    );
    assert_eq!(harness.sockets(), 0);
}

