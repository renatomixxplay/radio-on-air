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

// ---------- caminhos com acento (Windows) ----------
//
// O Liquidsoap de Windows não abre arquivos cujo caminho tem acento (ex.: um
// usuário "Rádio Cidade"): nem os próprios arquivos de apoio, nem músicas.
// Saídas: o motor roda numa pasta sem acento, e cada arquivo vai para ele pelo
// "nome curto" que o Windows guarda (ex.: GAUCHA~1), traduzido de volta ao voltar.

fn so_ascii(p: &Path) -> bool {
    p.to_str().is_some_and(|s| s.is_ascii())
}

/// Pasta sem acento para o Liquidsoap (Windows): `C:\ProgramData\RadioOnAir`.
fn pasta_sem_acento() -> std::path::PathBuf {
    let base = std::env::var_os("PROGRAMDATA").map(std::path::PathBuf::from).unwrap_or_else(|| "C:\\ProgramData".into());
    base.join("RadioOnAir")
}

/// Onde ficam script, registro e pid do motor e da captura. No Windows, se a
/// pasta de dados tiver acento, vai para uma pasta sem acento.
pub fn pasta_motor(dir_dados: &Path) -> std::path::PathBuf {
    if cfg!(windows) && !so_ascii(dir_dados) {
        pasta_sem_acento()
    } else {
        dir_dados.to_path_buf()
    }
}

/// Liquidsoap instalado numa pasta com acento: copia (uma vez) a pasta dele para
/// uma sem acento e devolve o novo executável.
pub fn liquidsoap_sem_acento(exe: &Path) -> std::path::PathBuf {
    if !cfg!(windows) || so_ascii(exe) {
        return exe.to_path_buf();
    }
    let (Some(origem), Some(nome)) = (exe.parent(), exe.file_name()) else { return exe.to_path_buf() };
    let destino = pasta_sem_acento().join("liquidsoap");
    let novo = destino.join(nome);
    // Copia de novo só se mudou (outra versão do instalador).
    let tamanho = |p: &Path| std::fs::metadata(p).map(|m| m.len()).ok();
    if tamanho(&novo) != tamanho(exe) || tamanho(&novo).is_none() {
        let _ = std::fs::remove_dir_all(&destino);
        if copiar_pasta(origem, &destino).is_err() {
            return exe.to_path_buf();
        }
    }
    novo
}

fn copiar_pasta(de: &Path, para: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(para)?;
    for item in std::fs::read_dir(de)? {
        let item = item?;
        let alvo = para.join(item.file_name());
        if item.file_type()?.is_dir() {
            copiar_pasta(&item.path(), &alvo)?;
        } else {
            std::fs::copy(item.path(), alvo)?;
        }
    }
    Ok(())
}

/// Caminho de arquivo para mandar ao motor: no Windows, com acento, vira o nome
/// curto (sem acento). Se não houver nome curto, vai como está.
pub fn para_motor(caminho: &str) -> String {
    #[cfg(windows)]
    if !caminho.is_ascii() {
        if let Some(curto) = win::curto(Path::new(caminho)).filter(|c| so_ascii(c)) {
            return curto.to_string_lossy().into_owned();
        }
    }
    caminho.to_string()
}

/// Caminho que o motor devolveu: no Windows, nome curto volta ao nome normal.
pub fn do_motor(caminho: &str) -> String {
    #[cfg(windows)]
    if caminho.contains('~') {
        if let Some(longo) = win::longo(Path::new(caminho)) {
            return longo.to_string_lossy().into_owned();
        }
    }
    caminho.to_string()
}

#[cfg(windows)]
mod win {
    use std::ffi::OsString;
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use std::path::{Path, PathBuf};

    type Conversao = unsafe extern "system" fn(*const u16, *mut u16, u32) -> u32;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetShortPathNameW(longo: *const u16, curto: *mut u16, tamanho: u32) -> u32;
        fn GetLongPathNameW(curto: *const u16, longo: *mut u16, tamanho: u32) -> u32;
    }

    fn converter(p: &Path, f: Conversao) -> Option<PathBuf> {
        let entrada: Vec<u16> = p.as_os_str().encode_wide().chain(Some(0)).collect();
        let mut saida = vec![0u16; 1024];
        loop {
            // SAFETY: entrada termina em zero; saida tem o tamanho informado.
            let n = unsafe { f(entrada.as_ptr(), saida.as_mut_ptr(), saida.len() as u32) } as usize;
            if n == 0 {
                return None;
            }
            if n < saida.len() {
                saida.truncate(n);
                return Some(PathBuf::from(OsString::from_wide(&saida)));
            }
            saida.resize(n + 1, 0);
        }
    }

    pub fn curto(p: &Path) -> Option<PathBuf> {
        converter(p, GetShortPathNameW)
    }

    pub fn longo(p: &Path) -> Option<PathBuf> {
        converter(p, GetLongPathNameW)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn caminho_sem_acento_passa_igual() {
        assert_eq!(para_motor("/m/Fogo de Chao.mp3"), "/m/Fogo de Chao.mp3");
        assert_eq!(do_motor("/m/Fogo de Chao.mp3"), "/m/Fogo de Chao.mp3");
    }

    /// No Windows: arquivo com acento vai sem acento para o motor e volta igual.
    #[cfg(windows)]
    #[test]
    fn acento_vira_nome_curto_e_volta() {
        let dir = std::env::temp_dir().join("Rádio Ção teste");
        std::fs::create_dir_all(&dir).unwrap();
        let arq = dir.join("Fogo de Chão.mp3");
        std::fs::write(&arq, b"").unwrap();
        let original = arq.to_string_lossy().into_owned();
        let curto = para_motor(&original);
        if curto != original {
            assert!(curto.is_ascii(), "{curto}");
            // Compara os dois em forma longa: a própria pasta temporária pode
            // vir com nome curto (ex.: C:\\Users\\RUNNER~1\\...).
            assert_eq!(do_motor(&curto).to_lowercase(), do_motor(&original).to_lowercase());
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
