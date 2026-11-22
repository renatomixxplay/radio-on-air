//! Conversa com o Liquidsoap, que roda escondido e só toca o áudio.
//!
//! O programa é quem manda: ele guarda a fila e entrega ao Liquidsoap uma faixa
//! por vez, poucos segundos antes da atual acabar. O Liquidsoap avisa pelo stdout
//! (`@@FAIXA <arquivo>`) quando uma faixa começa a tocar.
//!
//! Os arquivos já vêm preparados (silêncio cortado, volume igual): o motor não
//! analisa nada, por isso tudo começa na hora. Músicas entram marcadas como
//! `tipo="musica"` e se misturam com a próxima música (crossfade fixo de 2 s);
//! o resto (comercial, vinheta, hora certa, cartucheira) toca inteiro, em
//! sequência.

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
settings.log.level.set(3)
settings.log.file.set(true)
settings.log.file.append.set(false)
settings.log.file.path.set({log:?})
settings.server.telnet.set(true)
settings.server.telnet.bind_addr.set("127.0.0.1")
settings.server.telnet.port.set({porta})

# Ar: fila (músicas, comerciais, vinhetas; volume controlável para o ducking)
# + cartucheira por cima. Os arquivos já vêm preparados: nada é analisado.
fila_q = request.queue(id="fila")
cart = request.queue(id="cart")
# Crossfade fixo de 2 s só entre duas músicas; o resto toca em sequência.
def transicao(a, b) =
  if a.metadata["tipo"] == "musica" and b.metadata["tipo"] == "musica" then
    cross.simple(fade_in=2., fade_out=2., a.source, b.source)
  else
    sequence([a.source, b.source])
  end
end
# Volume do fade de troca (fim do ducking, lista "Imediato", link). Fica ANTES do
# crossfade: o pedaço do fim da música que o crossfade guarda (2 s) já sai com o
# volume baixo do fade. Depois dele, voltar o volume ao normal não traz a música
# antiga de volta alta (era o "pulo" no fim do fade).
# O volume só volta ao normal quando a PRÓXIMA faixa começa. Se o programa
# devolvesse logo depois do pulo, o último pedaço (0,2 s) da faixa que saía
# já vinha com o volume cheio: a música "levantava" no finalzinho do fade.
vol_troca = interactive.float("vol_troca", 1.0)
fila_q.on_track(synchronous=true, fun (_) -> vol_troca.set(1.0))
fila = cross(duration=2., transicao, amplify(override=null, vol_troca, fila_q))
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
fala = rms(duration=0.2, fala)
fala_ar = fallback(track_sensitive=false, [fala, blank()])
fala_ar.on_track(synchronous=true, fun (m) -> print("@@FALA #{{m['filename']}}"))
ultima_voz = ref(-100.)
# Volume da música em dB (0 = normal, -14 = 20%). O volume só muda de pedaço em
# pedaço de áudio ({quadro:.2} s): anda em degraus iguais em dB, pequenos o
# bastante para não se ouvir o "salto". Desce em 0,6 s e sobe em 2 s.
ganho_db = ref(0.)
desce_db = {desce:.3}
sobe_db = {sobe:.3}
def ganho() =
  if fala.is_ready() and fala.rms() > 0.02 then
    ultima_voz := time()
  end
  alvo = if time() - ultima_voz() < 1.5 then -14. else 0. end
  atual = ganho_db()
  ganho_db := if alvo < atual then max(alvo, atual - desce_db) else min(alvo, atual + sobe_db) end
  lin_of_dB(ganho_db())
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
# Ajuste de volume de cada link (o programa manda antes de ligar).
ganho_link = interactive.float("ganho_link", 1.0)
link_ar = fallback(track_sensitive=false, [amplify(override=null, fun () -> vol_link() * ganho_link(), aovivo), blank()])

radio = add(normalize=false, [musica, link_ar, fala_ar, cart])
{saida_ar}

# Pré-escuta: saída separada, não mexe no que está no ar.
pre = request.queue(id="pre")
previa = fallback(id="previa", track_sensitive=false, [pre, blank()])
previa.on_track(synchronous=true, fun (m) -> print("@@PRE #{{m['filename']}}"))
{saida_pre}
"#,
        quadro = quadro(),
        desce = 14.0 / (0.6 / quadro()),
        sobe = 14.0 / (2.0 / quadro()),
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
        crate::sistema::amarrar(&processo);

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
                // Anota o aviso no instante em que chega, antes de o programa tratar.
                if let Some(aviso) = linha.strip_prefix("@@") {
                    let (tipo, resto) = aviso.split_once(' ').unwrap_or((aviso, ""));
                    let arq = resto.split('\t').next().unwrap_or("").trim();
                    let arq = if arq.is_empty() { "(vazio)".to_string() } else { crate::registro::nome(arq) };
                    crate::registro::anotar(format!("← motor: {tipo} {arq}"));
                }
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
    /// Envia um comando e anota no `historico.log` (menos os passos de volume
    /// do fade, que são muitos; esses só se der erro).
    fn comando(&self, cmd: &str) -> Result<String, String> {
        let r = self.comando_sem_registro(cmd);
        let (verbo, resto) = cmd.split_once(' ').unwrap_or((cmd, ""));
        let resumo = if resto.contains(['/', '\\']) {
            let musica = if resto.contains("tipo=\"musica\"") { " (música)" } else { "" };
            format!("{verbo} {}{musica}", crate::registro::nome(resto))
        } else {
            cmd.to_string()
        };
        match &r {
            Err(e) => crate::registro::anotar(format!("→ motor: {resumo} — ERRO: {e}")),
            Ok(_) if !cmd.starts_with("var.set vol") => crate::registro::anotar(format!("→ motor: {resumo}")),
            Ok(_) => {}
        }
        r
    }

    fn comando_sem_registro(&self, cmd: &str) -> Result<String, String> {
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

    /// Entrega uma faixa ao ar. Música vai marcada (`tipo="musica"`) para se
    /// misturar com a próxima música; o resto vai como está e toca inteiro.
    pub fn enfileirar(&self, caminho: &str, musica: bool) -> Result<(), String> {
        let prefixo = if musica { "annotate:tipo=\"musica\":" } else { "" };
        self.comando(&format!("fila.push {prefixo}{}", crate::sistema::para_motor(caminho))).map(|_| ())
    }

    pub fn pular(&self) -> Result<(), String> {
        self.comando("fila.skip").map(|_| ())
    }

    /// Esvazia a fila do motor e pula a faixa atual.
    pub fn esvaziar_e_pular(&self) -> Result<(), String> {
        self.comando("fila.flush_and_skip").map(|_| ())
    }

    /// Volume da música no ar (1.0 = normal): o botão do microfone. Vale na
    /// hora. Não afeta a cartucheira.
    pub fn volume(&self, v: f64) -> Result<(), String> {
        self.comando(&format!("var.set vol = {v:.3}")).map(|_| ())
    }

    /// Volume do fade de troca (1.0 = normal). Vale antes do crossfade, com
    /// ~2 s de atraso, e não traz de volta o fim da música que saiu.
    pub fn volume_troca(&self, v: f64) -> Result<(), String> {
        self.comando(&format!("var.set vol_troca = {v:.3}")).map(|_| ())
    }

    /// Liga o link ao vivo (tenta reconectar sozinho se cair).
    /// `ganho_db`: ajuste de volume do link (0 = como chega).
    pub fn link_ligar(&self, url: &str, ganho_db: f64) -> Result<(), String> {
        let ganho = 10f64.powf(ganho_db.clamp(-30.0, 12.0) / 20.0);
        self.comando(&format!("var.set ganho_link = {ganho:.4}"))?;
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
            self.comando(&format!("fala.push {}", crate::sistema::para_motor(c)))?;
        }
        Ok(())
    }

    pub fn cartucho(&self, caminho: &str) -> Result<(), String> {
        self.comando(&format!("cart.push {}", crate::sistema::para_motor(caminho))).map(|_| ())
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



