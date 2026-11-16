//! Registro do que acontece no ar (`historico.log`), para achar problemas.
//!
//! Anota, com horário até o milésimo de segundo, o que o programa manda ao
//! motor, o que o motor responde, os agendamentos, o ducking, os fades e os
//! botões. O arquivo continua entre uma abertura e outra; passando de 5 MB vira
//! `historico.anterior.log` e começa um novo.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

static ARQUIVO: OnceLock<Mutex<Option<File>>> = OnceLock::new();
const LIMITE: u64 = 5 * 1024 * 1024;

pub fn iniciar(dir: &Path) {
    let caminho = dir.join("historico.log");
    if fs::metadata(&caminho).is_ok_and(|m| m.len() > LIMITE) {
        let _ = fs::rename(&caminho, dir.join("historico.anterior.log"));
    }
    let arquivo = OpenOptions::new().create(true).append(true).open(&caminho).ok();
    let _ = ARQUIVO.set(Mutex::new(arquivo));
    anotar(format!("===== programa aberto (versão {}) =====", env!("CARGO_PKG_VERSION")));
}

pub fn anotar(texto: impl AsRef<str>) {
    let Some(m) = ARQUIVO.get() else { return };
    let mut arquivo = m.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(f) = arquivo.as_mut() {
        let _ = writeln!(f, "{} {}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"), texto.as_ref());
    }
}

/// Só o nome do arquivo, sem a pasta (para o registro ficar legível).
pub fn nome(caminho: &str) -> String {
    let nome = Path::new(caminho).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    if nome.is_empty() { caminho.to_string() } else { nome }
}
