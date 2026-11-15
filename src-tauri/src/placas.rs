//! Placas de som do computador: saídas (para tocar) e entradas (da mesa).
//!
//! - Linux: PulseAudio (`pactl`). O id é o nome do dispositivo no PulseAudio.
//! - Windows: PortAudio, pela lista do próprio Liquidsoap
//!   (`--list-portaudio-devices`). O id é o nome da placa.
//! - macOS (só para desenvolver): `ffmpeg` lista os dispositivos; o id é o nome
//!   que aparece, e o número do dispositivo é procurado na hora de abrir.

use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize)]
pub struct Placa {
    pub id: String,
    pub nome: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Placas {
    pub saidas: Vec<Placa>,
    pub entradas: Vec<Placa>,
}

pub fn listar() -> Placas {
    if cfg!(target_os = "macos") {
        Placas {
            saidas: mac_saidas().into_iter().map(|(_, n)| Placa { id: n.clone(), nome: n }).collect(),
            entradas: mac_entradas().into_iter().map(|(_, n)| Placa { id: n.clone(), nome: n }).collect(),
        }
    } else if cfg!(windows) {
        windows()
    } else {
        Placas { saidas: pulse("sinks"), entradas: pulse("sources") }
    }
}

// ---------- Windows ----------

fn windows() -> Placas {
    let Some(exe) = crate::motor::localizar() else { return Placas::default() };
    let Ok(saida) = crate::sistema::liquidsoap(&exe).arg("--list-portaudio-devices").output() else {
        return Placas::default();
    };
    parse_portaudio(&String::from_utf8_lossy(&saida.stdout))
}

/// Lê os blocos `Device ID n:` / `- name:` / `- max input channels:` /
/// `- max output channels:`. A mesma placa aparece uma vez por sistema de som do
/// Windows (MME, DirectSound, WASAPI…): fica só a primeira de cada nome.
fn parse_portaudio(txt: &str) -> Placas {
    let mut p = Placas::default();
    let mut nome: Option<String> = None;
    let mut entrada = 0;
    let fechar = |nome: &mut Option<String>, entrada: &mut u32, saida: u32, p: &mut Placas| {
        if let Some(n) = nome.take().filter(|n| !n.is_empty()) {
            if *entrada > 0 && !p.entradas.iter().any(|x| x.id == n) {
                p.entradas.push(Placa { id: n.clone(), nome: n.clone() });
            }
            if saida > 0 && !p.saidas.iter().any(|x| x.id == n) {
                p.saidas.push(Placa { id: n.clone(), nome: n });
            }
        }
        *entrada = 0;
    };
    for linha in txt.lines().map(str::trim) {
        if let Some(n) = linha.strip_prefix("- name:") {
            nome = Some(n.trim().to_string());
        } else if let Some(c) = linha.strip_prefix("- max input channels:") {
            entrada = c.trim().parse().unwrap_or(0);
        } else if let Some(c) = linha.strip_prefix("- max output channels:") {
            let saida = c.trim().parse().unwrap_or(0);
            fechar(&mut nome, &mut entrada, saida, &mut p);
        }
    }
    p
}

// ---------- Linux ----------

fn pulse(tipo: &str) -> Vec<Placa> {
    let Ok(saida) = crate::sistema::comando("pactl").env("LC_ALL", "C").args(["list", tipo]).output() else { return vec![] };
    parse_pulse(&String::from_utf8_lossy(&saida.stdout))
}

/// Lê os pares `Name:` / `Description:` da saída de `pactl list sinks|sources`.
fn parse_pulse(txt: &str) -> Vec<Placa> {
    let mut v = Vec::new();
    let mut nome: Option<String> = None;
    for linha in txt.lines().map(str::trim) {
        if let Some(n) = linha.strip_prefix("Name:") {
            nome = Some(n.trim().to_string());
        } else if let Some(d) = linha.strip_prefix("Description:") {
            if let Some(id) = nome.take() {
                v.push(Placa { id, nome: d.trim().to_string() });
            }
        }
    }
    v
}

// ---------- macOS ----------

fn ffmpeg() -> Option<PathBuf> {
    // Programas abertos pelo Finder não recebem o PATH do terminal.
    let mut c: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).map(|d| d.join("ffmpeg")).collect())
        .unwrap_or_default();
    c.push("/opt/homebrew/bin/ffmpeg".into());
    c.push("/usr/local/bin/ffmpeg".into());
    c.into_iter().find(|p| p.is_file())
}

fn ffmpeg_stderr(args: &[&str]) -> String {
    let Some(exe) = ffmpeg() else { return String::new() };
    crate::sistema::comando(exe)
        .args(args)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stderr).into_owned())
        .unwrap_or_default()
}

/// Linhas do tipo `[AudioToolbox @ 0x…] [3]   Alto-falantes (MacBook Pro), BuiltInSpeakerDevice`.
fn indice_e_resto(linha: &str) -> Option<(usize, &str)> {
    let depois = &linha[linha.find("] [")? + 3..];
    let fim = depois.find(']')?;
    Some((depois[..fim].parse().ok()?, depois[fim + 1..].trim()))
}

fn parse_audiotoolbox(txt: &str) -> Vec<(usize, String)> {
    txt.lines()
        .filter(|l| l.contains("[AudioToolbox"))
        .filter_map(indice_e_resto)
        .filter_map(|(i, resto)| {
            let nome = resto.rsplit_once(", ").map_or(resto, |(n, _)| n).trim();
            (!nome.is_empty() && nome != "(null)").then(|| (i, nome.to_string()))
        })
        .collect()
}

fn parse_avfoundation(txt: &str) -> Vec<(usize, String)> {
    txt.lines()
        .skip_while(|l| !l.contains("AVFoundation audio devices"))
        .skip(1)
        .filter_map(indice_e_resto)
        .map(|(i, n)| (i, n.to_string()))
        .collect()
}

pub fn mac_saidas() -> Vec<(usize, String)> {
    parse_audiotoolbox(&ffmpeg_stderr(&[
        "-hide_banner", "-f", "lavfi", "-i", "anullsrc", "-t", "0.01", "-f", "audiotoolbox", "-list_devices", "true", "-",
    ]))
}

pub fn mac_entradas() -> Vec<(usize, String)> {
    parse_avfoundation(&ffmpeg_stderr(&["-hide_banner", "-f", "avfoundation", "-list_devices", "true", "-i", ""]))
}

/// Número do dispositivo com esse nome (macOS).
pub fn mac_indice(lista: &[(usize, String)], nome: &str) -> Option<usize> {
    lista.iter().find(|(_, n)| n == nome).map(|(i, _)| *i)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn le_portaudio_do_windows() {
        let txt = "Portaudio has 3 devices:\n\nDevice ID 0:\n- name: Microfone (USB Audio)\n- host API: 0\n- max input channels: 2\n- max output channels: 0\n- default sample rate: 44100.00\n\n\
                   Device ID 1:\n- name: Alto-falantes (Realtek)\n- host API: 0\n- max input channels: 0\n- max output channels: 2\n\n\
                   Device ID 2:\n- name: Alto-falantes (Realtek)\n- host API: 2\n- max input channels: 0\n- max output channels: 2\n";
        let p = parse_portaudio(txt);
        assert_eq!(p.entradas.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(), ["Microfone (USB Audio)"]);
        assert_eq!(p.saidas.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(), ["Alto-falantes (Realtek)"]);
    }

    #[test]
    fn le_pactl() {
        let txt = "Sink #0\n\tState: SUSPENDED\n\tName: alsa_output.pci.analog-stereo\n\tDescription: Built-in Audio Analog Stereo\n\
                   Sink #1\n\tName: alsa_output.usb\n\tDescription: USB Audio\n";
        let v = parse_pulse(txt);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].id, "alsa_output.pci.analog-stereo");
        assert_eq!(v[1].nome, "USB Audio");
    }

    #[test]
    fn le_ffmpeg_do_mac() {
        let at = "[AudioToolbox @ 0x7f] CoreAudio devices:\n\
                  [AudioToolbox @ 0x7f] [1]       Fones de Ouvido Externos, BuiltInHeadphoneOutputDevice\n\
                  [AudioToolbox @ 0x7f] [5]                         (null), ~:AMS2_Aggregate:1\n";
        assert_eq!(parse_audiotoolbox(at), vec![(1, "Fones de Ouvido Externos".to_string())]);
        let av = "[AVFoundation indev @ 0x7f] AVFoundation video devices:\n\
                  [AVFoundation indev @ 0x7f] [0] Câmera FaceTime HD\n\
                  [AVFoundation indev @ 0x7f] AVFoundation audio devices:\n\
                  [AVFoundation indev @ 0x7f] [0] Reaper QCY\n\
                  [AVFoundation indev @ 0x7f] [1] Microfone (MacBook Pro)\n\
                  [in#0 @ 0x7f] Error opening input: Input/output error\n";
        assert_eq!(parse_avfoundation(av), vec![(0, "Reaper QCY".to_string()), (1, "Microfone (MacBook Pro)".to_string())]);
    }
}
