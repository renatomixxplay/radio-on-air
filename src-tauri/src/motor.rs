//! Conversa com o Liquidsoap, que roda escondido e só toca o áudio.
//!
//! O programa é quem manda: ele guarda a fila e entrega ao Liquidsoap uma faixa
//! por vez, poucos segundos antes da atual acabar. O Liquidsoap avisa pelo stdout
//! (`@@FAIXA <arquivo>`) quando uma faixa começa a tocar.
//!
//! Músicas entram com `autocue:`: o Liquidsoap corta o silêncio do começo e do
//! fim, iguala o volume e mistura 1 s com a próxima música. O resto (comercial,
//! vinheta, hora certa, cartucheira) entra com `volume:`: só iguala o volume e
//! toca inteiro, sem mistura.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub enum EventoMotor {
    /// Uma faixa começou. Caminho vazio = a fila do motor ficou sem nada (silêncio).
    /// O número é a duração que vai tocar de fato (música com o silêncio cortado).
    Faixa(String, Option<f64>),
    /// A pré-escuta começou um arquivo (vazio = terminou).
    Previa(String),
    /// Um comercial por cima (ducking) começou.
    Fala(String),
    /// O processo do Liquidsoap terminou.
    Parou(String),
}

pub struct Motor {
    processo: Child,
    porta: u16,
}

/// Últimas mensagens do Liquidsoap (sem as linhas de registro com data).
#[derive(Clone, Default)]
pub struct Mensagens(Arc<Mutex<std::collections::VecDeque<String>>>);

impl Mensagens {
    pub fn anotar(&self, linha: &str) {
        let l = linha.trim();
        let registro = l.starts_with(">>>") || l.chars().take(4).all(|c| c.is_ascii_digit()) && l.len() > 4;
        if l.is_empty() || registro {
            return;
        }
        let mut v = self.0.lock().unwrap_or_else(|p| p.into_inner());
        if v.len() == 4 {
            v.pop_front();
        }
        v.push_back(l.to_string());
    }

    pub fn texto(&self) -> String {
        self.0.lock().unwrap_or_else(|p| p.into_inner()).iter().cloned().collect::<Vec<_>>().join(" ")
    }
}

/// Lê `<arquivo>\t<cue_in>\t<cue_out>`: devolve o arquivo e quanto vai tocar.
fn ler_faixa(resto: &str) -> (String, Option<f64>) {
    let mut partes = resto.trim().split('\t');
    let arq = partes.next().unwrap_or("").trim().to_string();
    let num = |p: Option<&str>| p.and_then(|v| v.trim().parse::<f64>().ok());
    let (ini, fim) = (num(partes.next()), num(partes.next()));
    let toca = fim.map(|f| f - ini.unwrap_or(0.0)).filter(|d| *d > 0.0);
    (arq, toca)
}

/// Liquidsoap que veio junto no instalador (Windows), se houver.
static EMBUTIDO: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// Registra o Liquidsoap que veio dentro do programa (pasta de recursos).
pub fn usar_embutido(pasta_recursos: &Path) {
    let exe = pasta_recursos.join("liquidsoap").join(if cfg!(windows) { "liquidsoap.exe" } else { "liquidsoap" });
    if exe.is_file() {
        // Instalado numa pasta com acento: o Liquidsoap roda de uma cópia sem acento.
        let _ = EMBUTIDO.set(crate::sistema::liquidsoap_sem_acento(&exe));
    }
}

/// Procura o executável do Liquidsoap.
pub fn localizar() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("RADIO_LIQUIDSOAP") {
        return Some(PathBuf::from(p));
    }
    if let Some(p) = EMBUTIDO.get() {
        return Some(p.clone());
    }
    let nome = if cfg!(windows) { "liquidsoap.exe" } else { "liquidsoap" };
    let mut candidatos: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).map(|d| d.join(nome)).collect())
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

/// Quanto o som sai depois do que o motor está tocando (a folga da saída no Mac).
pub const ATRASO_SAIDA: f64 = if cfg!(any(target_os = "macos", windows)) { 1.0 } else { 0.0 };

/// Linha de saída de som de cada sistema. `placa` = nome do dispositivo
/// (`None` = o padrão do sistema).
fn saida(id: &str, fonte: &str, placa: Option<&str>) -> String {
    if cfg!(target_os = "macos") {
        let indice = placa
            .and_then(|p| crate::placas::mac_indice(&crate::placas::mac_saidas(), p))
            .map(|i| format!("audio_device_index={i}, "))
            .unwrap_or_default();
        // A saída do macOS tem pouca folga: qualquer tranco do sistema (abrir um app,
        // disco, memória) virava uma travadinha. O buffer guarda ~1 s de áudio pronto.
        // (O buffer "adaptativo" foi descartado: ele muda a velocidade do som para
        // acompanhar a placa, e isso se ouvia como acelerar/desacelerar.)
        format!(
            r#"output.url(id="{id}", fallible=true, self_sync=true, url="default", %ffmpeg(format="audiotoolbox", {indice}%audio(codec="pcm_f32le")), buffer(fallible=true, buffer=1., max=10., {fonte}))"#
        )
    } else if cfg!(windows) {
        // Windows: PortAudio, placa escolhida pelo nome (só se ela estiver ligada,
        // senão o motor nem abriria). Mesma folga de 1 s do Mac contra travadinhas.
        let device = placa
            .filter(|p| crate::placas::listar().saidas.iter().any(|s| s.id == *p))
            .map(|p| format!("device_name={p:?}, "))
            .unwrap_or_default();
        format!(
            r#"output.portaudio(id="{id}", fallible=true, buflen=2048, {device}buffer(fallible=true, buffer=1., max=10., {fonte}))"#
        )
    } else {
        let device = placa.map(|p| format!("device={p:?}, ")).unwrap_or_default();
        format!(r#"output.pulseaudio(id="{id}", fallible=true, {device}{fonte})"#)
    }
}

/// Tamanho, em segundos, do pedaço de áudio que o Liquidsoap processa por vez.
fn quadro() -> f64 {
    // No Windows vale a mesma folga do Mac (a placa também é atendida por pedaços).
    if cfg!(any(target_os = "macos", windows)) {
        // A saída de som do macOS (ffmpeg/audiotoolbox) guarda só DOIS pedaços na
        // placa. Com o padrão (0,04 s) isso dá ~80 ms de folga: abrir o navegador
        // ou outro programa já fazia o som "raspar". Com 0,2 s a folga vai a ~0,4 s.
        0.2
    } else {
        0.04
    }
}

fn script(porta: u16, log: &str, audio: &crate::config::Audio) -> String {
    format!(
        r#"
settings.frame.duration.set({quadro:.2})
settings.log.stdout.set(false)
settings.log.level.set(2)
settings.log.file.set(true)
settings.log.file.append.set(false)
settings.log.file.path.set({log:?})
settings.server.telnet.set(true)
settings.server.telnet.bind_addr.set("127.0.0.1")
settings.server.telnet.port.set({porta})

# Ar: fila de músicas (com volume controlável para o ducking) + cartucheira por cima.
# Músicas (autocue): corta o silêncio do começo/fim, iguala o volume e mistura
# no máximo 1 s com a próxima música.
settings.autocue.internal.max_overlap.set(1.)
# Protocolo "volume:": mede o arquivo e só iguala o volume (sem cortar nada).
def protocolo_volume(~rlog:_, ~maxtime:_, arq) =
  medidas = file.autocue.metadata(request_metadata=[], arq)
  amp = medidas["liq_amplify"]
  if amp == "" then arq else "annotate:liq_amplify=#{{string.quote(amp)}}:#{{arq}}" end
end
protocol.add("volume", protocolo_volume, doc="Só iguala o volume", syntax="volume:arquivo")
fila_q = request.queue(id="fila")
cart = amplify(override="liq_amplify", 1., request.queue(id="cart"))
# Só mistura quando as duas faixas são músicas analisadas; o resto toca em sequência.
def transicao(a, b) =
  if a.metadata["liq_autocue"] != "" and b.metadata["liq_autocue"] != "" then
    cross.simple(fade_in=0., fade_out=1., initial_fade_in_metadata=b.metadata, initial_fade_out_metadata=a.metadata, a.source, b.source)
  else
    sequence([a.source, b.source])
  end
end
# O volume de cada faixa é aplicado ANTES da emenda: depois dela, o último
# pedaço da faixa perde o ajuste e tocava no volume original (mais alto).
fila = cross(duration=1., transicao, amplify(override="liq_amplify", 1., fila_q))
musica = fallback(id="musica", track_sensitive=false, [fila, blank(id="silencio")])
# A troca de faixa sem mistura gera uma "faixa vazia" de passagem com a fila
# pronta: só avisa silêncio quando a fila acabou de verdade.
musica.on_track(synchronous=true, fun (m) ->
  if m["filename"] != "" then
    print("@@FAIXA #{{m['filename']}}\t#{{m['liq_cue_in']}}\t#{{m['liq_cue_out']}}")
  elsif not fila.is_ready() then
    print("@@FAIXA ")
  end)
# Comercial com ducking: toca por cima. A música abaixa para 20% enquanto há voz
# (e até 1,5 s depois, para não subir entre um arquivo e outro) e volta suave.
fala = request.queue(id="fala")
fala = amplify(override="liq_amplify", 1., fala)
fala = rms(duration=0.2, fala)
fala_ar = fallback(track_sensitive=false, [fala, blank()])
fala_ar.on_track(synchronous=true, fun (m) -> print("@@FALA #{{m['filename']}}"))
ultima_voz = ref(-100.)
ganho_fala = ref(1.)
# Suavização por pedaço de áudio: mesma velocidade qualquer que seja o tamanho
# do pedaço (0,15 por pedaço de 0,04 s).
suave = {suave:.4}
def ganho() =
  if fala.is_ready() and fala.rms() > 0.02 then
    ultima_voz := time()
  end
  alvo = if time() - ultima_voz() < 1.5 then 0.2 else 1. end
  ganho_fala := ganho_fala() + (alvo - ganho_fala()) * suave
  ganho_fala()
end
vol = interactive.float("vol", 1.0)
musica = amplify(override=null, fun () -> ganho() * vol(), musica)

# Link ao vivo (MP3/AAC/HLS). Fica desligado até o programa mandar ligar.
# Se cair, tenta de novo sozinho; enquanto isso, silêncio.
aovivo = input.http(id="aovivo_in", start=false, self_sync=false, poll_delay=3., "")
server.register(namespace="aovivo", description="Liga o link", "ligar", fun (u) -> begin
  aovivo.set_url(u)
  aovivo.start()
  "OK"
end)
server.register(namespace="aovivo", description="Desliga o link", "desligar", fun (_) -> begin
  aovivo.stop()
  "OK"
end)
vol_link = interactive.float("vol_link", 1.0)
link_ar = fallback(track_sensitive=false, [amplify(override=null, vol_link, aovivo), blank()])

radio = add(normalize=false, [musica, link_ar, fala_ar, cart])
{saida_ar}

# Pré-escuta: saída separada, não mexe no que está no ar.
pre = request.queue(id="pre")
previa = fallback(id="previa", track_sensitive=false, [pre, blank()])
previa.on_track(synchronous=true, fun (m) -> print("@@PRE #{{m['filename']}}"))
{saida_pre}
"#,
        quadro = quadro(),
        suave = 1.0 - 0.85_f64.powf(quadro() / 0.04),
        saida_ar = saida("saida", "radio", audio.saida.as_deref()),
        saida_pre = saida("preescuta", "previa", audio.pre_escuta.as_deref()),
    )
}

impl Motor {
    pub fn iniciar(
        exe: &Path,
        dir_dados: &Path,
        audio: &crate::config::Audio,
        ao_evento: impl Fn(EventoMotor) + Send + 'static,
    ) -> Result<Self, String> {
        let porta = porta_livre();
        let dir_dados = &crate::sistema::pasta_motor(dir_dados);
        std::fs::create_dir_all(dir_dados).map_err(|e| e.to_string())?;
        let arquivo = dir_dados.join("motor.liq");
        let log = dir_dados.join("motor.log").to_string_lossy().into_owned();
        std::fs::write(&arquivo, script(porta, &log, audio)).map_err(|e| e.to_string())?;

        // Se o programa fechou de mau jeito antes, o motor antigo pode ter ficado vivo.
        let arquivo_pid = dir_dados.join("motor.pid");
        crate::sistema::fechar_antigo(&arquivo_pid, "motor.liq");

        let mut processo = crate::sistema::liquidsoap(exe)
            .arg(&arquivo)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("não consegui abrir o Liquidsoap: {e}"))?;

        let _ = std::fs::write(&arquivo_pid, processo.id().to_string());
        let stdout = processo.stdout.take().expect("stdout");
        let stderr = processo.stderr.take().expect("stderr");
        // Lê stderr e stdout o tempo todo (para o cano não encher) e guarda as
        // últimas mensagens: é o que aparece no aviso se o motor cair.
        let mensagens = Mensagens::default();
        let guarda = mensagens.clone();
        std::thread::spawn(move || {
            for linha in BufReader::new(stderr).lines().map_while(Result::ok) {
                guarda.anotar(&linha);
            }
        });
        let ultimas = mensagens.clone();
        std::thread::spawn(move || {
            for linha in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Some(resto) = linha.strip_prefix("@@FAIXA") {
                    let (arq, toca) = ler_faixa(resto);
                    ao_evento(EventoMotor::Faixa(crate::sistema::do_motor(&arq), toca));
                } else if let Some(arq) = linha.strip_prefix("@@FALA") {
                    ao_evento(EventoMotor::Fala(crate::sistema::do_motor(arq.trim())));
                } else if let Some(arq) = linha.strip_prefix("@@PRE") {
                    ao_evento(EventoMotor::Previa(crate::sistema::do_motor(arq.trim())));
                } else {
                    ultimas.anotar(&linha);
                }
            }
            ao_evento(EventoMotor::Parou(ultimas.texto()));
        });

        let mut motor = Self { processo, porta };
        // Espera o servidor de comandos subir. Em máquina lenta ou ocupada o
        // Liquidsoap pode levar vários segundos para carregar.
        for _ in 0..600 {
            if TcpStream::connect(("127.0.0.1", porta)).is_ok() {
                return Ok(motor);
            }
            if let Ok(Some(_)) = motor.processo.try_wait() {
                // Dá tempo de ler o que ele escreveu antes de fechar.
                std::thread::sleep(Duration::from_millis(300));
                let msg = mensagens.texto();
                return Err(if msg.is_empty() {
                    "O motor de áudio fechou ao iniciar.".into()
                } else {
                    format!("O motor de áudio fechou ao iniciar: {msg}")
                });
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Err("O motor de áudio não respondeu em 60 segundos.".into())
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

    /// Entrega uma faixa ao ar. Música vai com `autocue:` (corte de silêncio,
    /// volume igual e mistura); o resto com `volume:` (só volume igual).
    pub fn enfileirar(&self, caminho: &str, musica: bool) -> Result<(), String> {
        let prefixo = if musica { "autocue:" } else { "volume:" };
        self.comando(&format!("fila.push {prefixo}{}", crate::sistema::para_motor(caminho))).map(|_| ())
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

    /// Liga o link ao vivo (tenta reconectar sozinho se cair).
    pub fn link_ligar(&self, url: &str) -> Result<(), String> {
        self.comando(&format!("aovivo.ligar {}", url.trim())).map(|_| ())
    }

    pub fn link_desligar(&self) -> Result<(), String> {
        self.comando("aovivo.desligar x").map(|_| ())
    }

    /// Volume do link (1.0 = normal), para o fade de saída.
    pub fn link_volume(&self, v: f64) -> Result<(), String> {
        self.comando(&format!("var.set vol_link = {v:.3}")).map(|_| ())
    }

    pub fn previa_tocar(&self, caminho: &str) -> Result<(), String> {
        self.comando("pre.flush_and_skip")?;
        self.comando(&format!("pre.push {}", crate::sistema::para_motor(caminho))).map(|_| ())
    }

    pub fn previa_parar(&self) -> Result<(), String> {
        self.comando("pre.flush_and_skip").map(|_| ())
    }

    /// Recomeça a pré-escuta a partir de `segundos`. Pular pela saída não
    /// funciona (no Mac ela tem uma folga de 1 s e só anda dentro dela) e o pulo
    /// do Liquidsoap não volta para trás: recomeçar no ponto vale para os dois lados.
    pub fn previa_ir(&self, caminho: &str, segundos: f64) -> Result<(), String> {
        self.comando("pre.flush_and_skip")?;
        let caminho = crate::sistema::para_motor(caminho);
        self.comando(&format!("pre.push annotate:liq_cue_in=\"{segundos:.2}\":{caminho}")).map(|_| ())
    }

    /// Toca arquivos por cima da música, em sequência (comercial com ducking).
    pub fn falar(&self, caminhos: &[String]) -> Result<(), String> {
        for c in caminhos {
            self.comando(&format!("fala.push volume:{}", crate::sistema::para_motor(c)))?;
        }
        Ok(())
    }

    pub fn cartucho(&self, caminho: &str) -> Result<(), String> {
        self.comando(&format!("cart.push volume:{}", crate::sistema::para_motor(caminho))).map(|_| ())
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

#[cfg(test)]
mod testes {
    #[test]
    fn le_faixa_com_e_sem_corte() {
        assert_eq!(super::ler_faixa(" /m/a.mp3\t1.6\t150.5"), ("/m/a.mp3".into(), Some(148.9)));
        assert_eq!(super::ler_faixa(" /m/comercial.mp3\t\t"), ("/m/comercial.mp3".into(), None));
        assert_eq!(super::ler_faixa(" "), ("".into(), None));
    }

    /// O script do motor precisa ser aceito pelo Liquidsoap instalado.
    #[test]
    fn script_do_motor_e_valido() {
        let Some(exe) = super::localizar() else { return };
        let dir = std::env::temp_dir().join(format!("radio-motor-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let arq = dir.join("teste.liq");
        std::fs::write(&arq, super::script(1, &dir.join("log").to_string_lossy(), &Default::default())).unwrap();
        let saida = std::process::Command::new(exe).arg("--check").arg(&arq).output().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(saida.status.success(), "{}{}", String::from_utf8_lossy(&saida.stdout), String::from_utf8_lossy(&saida.stderr));
    }
}
