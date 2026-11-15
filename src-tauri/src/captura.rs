//! Captura da entrada da placa (a mesa): grava em MP3 e transmite para o Icecast.
//!
//! Roda num Liquidsoap separado do que toca as músicas: mudar a gravação ou a
//! transmissão não interrompe o som no ar.

use crate::config::{Audio, Gravacao, Transmissao, DIAS_GRAVACAO};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};

pub enum EventoCaptura {
    /// Estado da transmissão: "conectado", "caiu" ou "erro <mensagem>".
    Transmissao(String),
    /// Erro ao gravar.
    Gravacao(String),
    /// O processo terminou (com a última linha de erro).
    Parou(String),
}

pub struct Captura {
    processo: Child,
}

impl Drop for Captura {
    fn drop(&mut self) {
        let _ = self.processo.kill();
        let _ = self.processo.wait();
    }
}

/// Liquidsoap entende strings com aspas e `\"` / `\\`, como o Rust mostra com `{:?}`.
fn texto(s: &str) -> String {
    format!("{s:?}")
}

fn entrada(placa: Option<&str>) -> String {
    if cfg!(target_os = "macos") {
        let dispositivo = placa
            .and_then(|p| crate::placas::mac_indice(&crate::placas::mac_entradas(), p))
            .map_or("default".to_string(), |i| i.to_string());
        format!(r#"input.ffmpeg(id="entrada", format="avfoundation", ":{dispositivo}")"#)
    } else if cfg!(windows) {
        let device = placa
            .filter(|p| crate::placas::listar().entradas.iter().any(|s| s.id == *p))
            .map(|p| format!("device_name={}, ", texto(p)))
            .unwrap_or_default();
        format!(r#"input.portaudio(id="entrada", buflen=2048, {device})"#).replace(", )", ")")
    } else {
        let device = placa.map(|p| format!("device={}, ", texto(p))).unwrap_or_default();
        format!(r#"input.pulseaudio(id="entrada", {device}fallible=true)"#)
    }
}

/// Codificador MP3. O Liquidsoap do Windows não tem o codificador `%mp3`
/// (lame): lá o MP3 sai pelo FFmpeg.
fn mp3(estereo: bool, kbps: u32, samplerate: u32) -> String {
    if cfg!(windows) {
        let canais = if estereo { 2 } else { 1 };
        format!(r#"%ffmpeg(format="mp3", %audio(codec="libmp3lame", channels={canais}, samplerate={samplerate}, b="{kbps}k"))"#)
    } else {
        let canais = if estereo { "stereo" } else { "mono" };
        format!("%mp3({canais}, bitrate={kbps}, samplerate={samplerate})")
    }
}

/// Caminho com data e hora: `<pasta>/15-11-2026/14-00.mp3` (hora do início do arquivo).
pub fn caminho_gravacao(pasta: &Path) -> String {
    pasta.join("%d-%m-%Y").join("%H-%M.mp3").to_string_lossy().into_owned()
}

fn gravacao(g: &Gravacao) -> String {
    let Some(pasta) = g.pasta.as_deref().filter(|_| g.ligada) else { return String::new() };
    // Pasta com acento (Windows): grava pelo nome curto dela.
    let _ = std::fs::create_dir_all(pasta);
    let pasta = std::path::PathBuf::from(crate::sistema::para_motor(&pasta.to_string_lossy()));
    let pasta = pasta.as_path();
    let virada = if g.minutos == 30 { "0m or 30m" } else { "0m" };
    format!(
        r#"
# Gravação: MP3 mono, um arquivo novo na virada da hora (ou da meia hora).
output.file(
  id="gravacao", fallible=true,
  reopen_when=fun () -> {virada},
  reopen_on_error=fun (e) -> begin print("@@GRAV #{{e}}") 10. end,
  {codificador},
  {{time.string({caminho})}}, mean(entrada))
"#,
        codificador = mp3(false, g.bitrate, 22050),
        caminho = texto(&caminho_gravacao(pasta)),
    )
}

fn transmissao(t: &Transmissao, nome_radio: &str) -> String {
    if !t.ligada {
        return String::new();
    }
    let fonte = if t.estereo { "entrada" } else { "mean(entrada)" };
    format!(
        r#"
# Transmissão: Icecast em MP3. Se cair, tenta de novo a cada 5 s.
output.icecast(
  id="transmissao", fallible=true,
  {codificador},
  host={host}, port={porta}, password={senha}, mount={ponto}, name={nome},
  on_connect=fun () -> print("@@TX conectado"),
  on_disconnect=fun () -> print("@@TX caiu"),
  on_error=fun (~restart_in, e) -> begin print("@@TX erro #{{e}}") restart_in(5.) end,
  {fonte})
"#,
        codificador = mp3(t.estereo, t.bitrate, t.samplerate),
        host = texto(&t.servidor),
        porta = t.porta,
        senha = texto(&t.senha),
        ponto = texto(&t.ponto),
        nome = texto(nome_radio),
    )
}

pub fn script(log: &str, audio: &Audio, g: &Gravacao, t: &Transmissao, nome_radio: &str) -> String {
    format!(
        r#"
settings.log.stdout.set(false)
settings.log.level.set(2)
settings.log.file.set(true)
settings.log.file.append.set(false)
settings.log.file.path.set({log})

entrada = mksafe({entrada})
{gravacao}
{transmissao}
"#,
        log = texto(log),
        entrada = entrada(audio.entrada.as_deref()),
        gravacao = gravacao(g),
        transmissao = transmissao(t, nome_radio),
    )
}

impl Captura {
    pub fn iniciar(
        exe: &Path,
        dir_dados: &Path,
        script: String,
        ao_evento: impl Fn(EventoCaptura) + Send + 'static,
    ) -> Result<Self, String> {
        std::fs::create_dir_all(dir_dados).map_err(|e| e.to_string())?;
        let arquivo = dir_dados.join("captura.liq");
        std::fs::write(&arquivo, script).map_err(|e| e.to_string())?;

        // Se o programa fechou de mau jeito, a captura antiga pode ter ficado viva.
        let arquivo_pid = dir_dados.join("captura.pid");
        crate::sistema::fechar_antigo(&arquivo_pid, "captura.liq");

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
        // Guarda as últimas mensagens do Liquidsoap: aparecem no aviso se ele cair.
        let mensagens = crate::motor::Mensagens::default();
        let guarda = mensagens.clone();
        std::thread::spawn(move || {
            for linha in BufReader::new(stderr).lines().map_while(Result::ok) {
                guarda.anotar(&linha);
            }
        });
        std::thread::spawn(move || {
            for linha in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Some(t) = linha.strip_prefix("@@TX ") {
                    ao_evento(EventoCaptura::Transmissao(t.trim().to_string()));
                } else if let Some(t) = linha.strip_prefix("@@GRAV ") {
                    ao_evento(EventoCaptura::Gravacao(t.trim().to_string()));
                } else {
                    mensagens.anotar(&linha);
                }
            }
            ao_evento(EventoCaptura::Parou(mensagens.texto()));
        });
        Ok(Self { processo })
    }
}

/// Erro da transmissão em palavras simples. O Liquidsoap manda algo como
/// `error(kind="source",message="Cry.Timeout",positions="…")`.
pub fn erro_legivel(bruto: &str) -> String {
    let msg = bruto
        .split_once("message=\"")
        .and_then(|(_, r)| r.split_once('"'))
        .map_or(bruto, |(m, _)| m);
    let m = msg.to_lowercase();
    if m.contains("timeout") {
        "o servidor não respondeu".into()
    } else if m.contains("password") || m.contains("401") || m.contains("unauthorized") || m.contains("forbidden") || m.contains("403") {
        "senha ou ponto de montagem recusados".into()
    } else if m.contains("mount") || m.contains("in use") || m.contains("409") {
        "ponto de montagem já está em uso".into()
    } else if m.contains("refused") || m.contains("unix_error") || m.contains("resolve") || m.contains("host") {
        "não conectou no servidor (endereço ou porta)".into()
    } else {
        msg.to_string()
    }
}

/// Apaga as pastas de dia (`dd-mm-aaaa`) com mais de 30 dias. Só mexe em pastas
/// com esse formato de nome, dentro da pasta de gravações.
pub fn apagar_antigas(pasta: &Path, hoje: chrono::NaiveDate) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(pasta) else { return vec![] };
    let limite = hoje - chrono::Duration::days(DIAS_GRAVACAO);
    let mut apagadas = Vec::new();
    for d in rd.filter_map(Result::ok).map(|d| d.path()).filter(|p| p.is_dir()) {
        let Some(nome) = d.file_name().and_then(|n| n.to_str()) else { continue };
        let Ok(data) = chrono::NaiveDate::parse_from_str(nome, "%d-%m-%Y") else { continue };
        if data < limite && std::fs::remove_dir_all(&d).is_ok() {
            apagadas.push(d);
        }
    }
    apagadas
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn script_da_captura_e_valido() {
        let Some(exe) = crate::motor::localizar() else { return };
        let dir = std::env::temp_dir().join(format!("radio-captura-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let g = Gravacao { ligada: true, pasta: Some(dir.join("grav \"x\"")), bitrate: 24, minutos: 30 };
        let t = Transmissao { ligada: true, servidor: "exemplo.com".into(), senha: "s\"e".into(), estereo: false, ..Default::default() };
        let arq = dir.join("teste.liq");
        std::fs::write(&arq, script(&dir.join("log").to_string_lossy(), &Audio::default(), &g, &t, "Rádio Teste")).unwrap();
        let saida = std::process::Command::new(exe).arg("--check").arg(&arq).output().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(saida.status.success(), "{}{}", String::from_utf8_lossy(&saida.stdout), String::from_utf8_lossy(&saida.stderr));
    }

    #[test]
    fn erro_da_transmissao_em_palavras_simples() {
        assert_eq!(erro_legivel(r#"error(kind="source",message="Cry.Timeout",positions="x")"#), "o servidor não respondeu");
        assert_eq!(erro_legivel("coisa estranha"), "coisa estranha");
    }

    #[test]
    fn apaga_so_pastas_de_dia_antigas() {
        let dir = std::env::temp_dir().join(format!("radio-grav-{}", std::process::id()));
        for n in ["01-01-2026", "14-10-2026", "16-10-2026", "outra coisa"] {
            std::fs::create_dir_all(dir.join(n)).unwrap();
        }
        let hoje = chrono::NaiveDate::from_ymd_opt(2026, 11, 15).unwrap();
        let mut apagadas: Vec<String> =
            apagar_antigas(&dir, hoje).iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
        apagadas.sort();
        assert_eq!(apagadas, ["01-01-2026", "14-10-2026"]);
        assert!(dir.join("16-10-2026").exists() && dir.join("outra coisa").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
