//! Conversa com o Liquidsoap, que roda escondido e só toca o áudio.
//!
//! O programa é quem manda: ele guarda a fila e entrega ao Liquidsoap uma faixa
//! por vez, poucos segundos antes da atual acabar. O Liquidsoap avisa pelo stdout
//! (`@@FAIXA <arquivo>`) quando uma faixa começa a tocar.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub enum EventoMotor {
    /// Uma faixa começou. Caminho vazio = a fila do motor ficou sem nada (silêncio).
    Faixa(String),
    /// A pré-escuta começou um arquivo (vazio = terminou).
    Previa(String),
    /// O processo do Liquidsoap terminou.
    Parou(String),
}

pub struct Motor {
    processo: Child,
    porta: u16,
}

/// Procura o executável do Liquidsoap.
pub fn localizar() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("RADIO_LIQUIDSOAP") {
        return Some(PathBuf::from(p));
    }
    let mut candidatos: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).map(|d| d.join("liquidsoap")).collect())
        .unwrap_or_default();
    if let Some(home) = std::env::var_os("HOME") {
        // Instalação isolada via opam (usada para desenvolver no Mac).
        candidatos.push(Path::new(&home).join(".opam/radio/bin/liquidsoap"));
    }
    candidatos.push("/usr/bin/liquidsoap".into());
    candidatos.push("/usr/local/bin/liquidsoap".into());
    candidatos.into_iter().find(|p| p.is_file())
}

fn porta_livre() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .map(|a| a.port())
        .unwrap_or(14_321)
}

/// Linha de saída de som de cada sistema.
fn saida(id: &str, fonte: &str) -> String {
    if cfg!(target_os = "macos") {
        // A saída do macOS tem pouca folga: qualquer tranco do sistema (abrir um app,
        // disco, memória) virava uma travadinha. O buffer guarda ~1 s de áudio pronto
        // e se ajusta sozinho ao relógio da placa de som.
        format!(
            r#"output.url(id="{id}", fallible=true, self_sync=true, url="default", %ffmpeg(format="audiotoolbox", %audio(codec="pcm_f32le")), buffer.adaptative(fallible=true, buffer=1., max=3., {fonte}))"#
        )
    } else {
        format!(r#"output.pulseaudio(id="{id}", fallible=true, {fonte})"#)
    }
}

fn script(porta: u16, log: &str) -> String {
    format!(
        r#"
settings.log.stdout.set(false)
settings.log.level.set(2)
settings.log.file.set(true)
settings.log.file.append.set(false)
settings.log.file.path.set({log:?})
settings.server.telnet.set(true)
settings.server.telnet.bind_addr.set("127.0.0.1")
settings.server.telnet.port.set({porta})

# Ar: fila de músicas (com volume controlável para o ducking) + cartucheira por cima.
fila = request.queue(id="fila")
cart = request.queue(id="cart")
musica = fallback(id="musica", track_sensitive=false, [fila, blank(id="silencio")])
musica.on_track(synchronous=true, fun (m) -> print("@@FAIXA #{{m['filename']}}"))
vol = interactive.float("vol", 1.0)
musica = amplify(vol, musica)
radio = add(normalize=false, [musica, cart])
{saida_ar}

# Pré-escuta: saída separada, não mexe no que está no ar.
pre = request.queue(id="pre")
previa = fallback(id="previa", track_sensitive=false, [pre, blank()])
previa.on_track(synchronous=true, fun (m) -> print("@@PRE #{{m['filename']}}"))
{saida_pre}
"#,
        saida_ar = saida("saida", "radio"),
        saida_pre = saida("preescuta", "previa"),
    )
}

impl Motor {
    pub fn iniciar(
        exe: &Path,
        dir_dados: &Path,
        ao_evento: impl Fn(EventoMotor) + Send + 'static,
    ) -> Result<Self, String> {
        let porta = porta_livre();
        std::fs::create_dir_all(dir_dados).map_err(|e| e.to_string())?;
        let arquivo = dir_dados.join("motor.liq");
        let log = dir_dados.join("motor.log").to_string_lossy().into_owned();
        std::fs::write(&arquivo, script(porta, &log)).map_err(|e| e.to_string())?;

        // Se o programa fechou de mau jeito antes, o motor antigo pode ter ficado vivo.
        let arquivo_pid = dir_dados.join("motor.pid");
        if let Some(pid) = std::fs::read_to_string(&arquivo_pid).ok().and_then(|p| p.trim().parse::<u32>().ok()) {
            let cmd = Command::new("ps").args(["-p", &pid.to_string(), "-o", "command="]).output();
            if cmd.is_ok_and(|o| String::from_utf8_lossy(&o.stdout).contains("motor.liq")) {
                let _ = Command::new("kill").arg(pid.to_string()).status();
            }
        }

        let mut processo = Command::new(exe)
            .arg(&arquivo)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("não consegui abrir o Liquidsoap: {e}"))?;

        let _ = std::fs::write(&arquivo_pid, processo.id().to_string());
        let stdout = processo.stdout.take().expect("stdout");
        let stderr = processo.stderr.take().expect("stderr");
        // Lê o stderr o tempo todo (para o cano não encher) e guarda o fim.
        let ultimo_erro = Arc::new(Mutex::new(String::new()));
        let guarda = ultimo_erro.clone();
        std::thread::spawn(move || {
            for linha in BufReader::new(stderr).lines().map_while(Result::ok) {
                if !linha.trim().is_empty() {
                    *guarda.lock().unwrap() = linha;
                }
            }
        });
        std::thread::spawn(move || {
            for linha in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Some(arq) = linha.strip_prefix("@@FAIXA ") {
                    ao_evento(EventoMotor::Faixa(arq.trim().to_string()));
                } else if let Some(arq) = linha.strip_prefix("@@PRE") {
                    ao_evento(EventoMotor::Previa(arq.trim().to_string()));
                }
            }
            let erro = ultimo_erro.lock().unwrap().clone();
            ao_evento(EventoMotor::Parou(erro));
        });

        let motor = Self { processo, porta };
        // Espera o servidor de comandos subir.
        for _ in 0..50 {
            if TcpStream::connect(("127.0.0.1", porta)).is_ok() {
                return Ok(motor);
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Err("o Liquidsoap não respondeu".into())
    }

    /// Envia um comando ao servidor telnet do Liquidsoap e devolve a resposta.
    fn comando(&self, cmd: &str) -> Result<String, String> {
        let mut s = TcpStream::connect(("127.0.0.1", self.porta)).map_err(|e| e.to_string())?;
        s.set_read_timeout(Some(Duration::from_secs(2))).ok();
        writeln!(s, "{cmd}").map_err(|e| e.to_string())?;
        let mut resp = String::new();
        for linha in BufReader::new(&s).lines() {
            let linha = linha.map_err(|e| e.to_string())?;
            if linha.trim() == "END" {
                break;
            }
            resp.push_str(&linha);
            resp.push('\n');
        }
        let _ = writeln!(s, "quit");
        Ok(resp)
    }

    pub fn enfileirar(&self, caminho: &str) -> Result<(), String> {
        self.comando(&format!("fila.push {caminho}")).map(|_| ())
    }

    pub fn pular(&self) -> Result<(), String> {
        self.comando("fila.skip").map(|_| ())
    }

    /// Esvazia a fila do motor e pula a faixa atual.
    pub fn esvaziar_e_pular(&self) -> Result<(), String> {
        self.comando("fila.flush_and_skip").map(|_| ())
    }

    /// Volume da música no ar (1.0 = normal). Não afeta a cartucheira.
    pub fn volume(&self, v: f64) -> Result<(), String> {
        self.comando(&format!("var.set vol = {v:.3}")).map(|_| ())
    }

    pub fn previa_tocar(&self, caminho: &str) -> Result<(), String> {
        self.comando("pre.flush_and_skip")?;
        self.comando(&format!("pre.push {caminho}")).map(|_| ())
    }

    pub fn previa_parar(&self) -> Result<(), String> {
        self.comando("pre.flush_and_skip").map(|_| ())
    }

    /// Anda `delta` segundos na pré-escuta (negativo volta).
    pub fn previa_andar(&self, delta: f64) -> Result<(), String> {
        self.comando(&format!("preescuta.seek {delta:.2}")).map(|_| ())
    }

    pub fn cartucho(&self, caminho: &str) -> Result<(), String> {
        self.comando(&format!("cart.push {caminho}")).map(|_| ())
    }

    pub fn pausar(&self) -> Result<(), String> {
        self.comando("saida.stop").map(|_| ())
    }

    pub fn retomar(&self) -> Result<(), String> {
        self.comando("saida.start").map(|_| ())
    }
}

impl Drop for Motor {
    fn drop(&mut self) {
        let _ = self.processo.kill();
        let _ = self.processo.wait();
    }
}
