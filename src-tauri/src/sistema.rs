//! O que muda de um sistema para outro ao abrir e fechar programas.

use std::ffi::OsStr;
use std::path::Path;
use std::process::Command;

/// Prepara um programa para rodar escondido. No Windows, sem isto cada
/// Liquidsoap (e cada consulta às placas) abriria uma janela preta de terminal.
pub fn comando(exe: impl AsRef<OsStr>) -> Command {
    #[allow(unused_mut)]
    let mut c = Command::new(exe);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        c.creation_flags(CREATE_NO_WINDOW);
    }
    c
}

/// Prepara o Liquidsoap para rodar escondido e de dentro da própria pasta: a
/// versão de Windows grava um cache na pasta de onde é aberta (que pode ser
/// uma pasta protegida do sistema).
pub fn liquidsoap(exe: &Path) -> Command {
    let mut c = comando(exe);
    if let Some(pasta) = exe.parent().filter(|p| p.is_dir()) {
        c.current_dir(pasta);
    }
    c
}

/// Se o programa fechou de mau jeito antes, o Liquidsoap antigo pode ter ficado
/// vivo (segurando a placa de som). Fecha esse processo, se ainda for o nosso:
/// no Linux/Mac confere o script (`marca`), no Windows o nome do executável.
pub fn fechar_antigo(arquivo_pid: &Path, marca: &str) {
    let Some(pid) = std::fs::read_to_string(arquivo_pid).ok().and_then(|p| p.trim().parse::<u32>().ok()) else {
        return;
    };
    let pid = pid.to_string();
    if cfg!(windows) {
        let filtro = format!("PID eq {pid}");
        let lista = comando("tasklist").args(["/FI", &filtro, "/FO", "CSV", "/NH"]).output();
        if lista.is_ok_and(|o| String::from_utf8_lossy(&o.stdout).to_lowercase().contains("liquidsoap")) {
            let _ = comando("taskkill").args(["/PID", &pid, "/F"]).status();
        }
    } else {
        let cmd = comando("ps").args(["-p", &pid, "-o", "command="]).output();
        if cmd.is_ok_and(|o| String::from_utf8_lossy(&o.stdout).contains(marca)) {
            let _ = comando("kill").arg(&pid).status();
        }
    }
}
