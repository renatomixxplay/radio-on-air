//! O que muda de um sistema para outro ao abrir e fechar programas.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
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

/// Prepara o Liquidsoap para rodar escondido. No Windows ele fica instalado em
/// "Arquivos de Programas", onde não pode gravar: o cache dele (e a pasta de
/// trabalho) vão para `C:\ProgramData\RadioOnAir`, sem acento e gravável.
pub fn liquidsoap(exe: &Path) -> Command {
    let mut c = comando(exe);
    if cfg!(windows) {
        let base = pasta_windows();
        let cache = base.join("cache");
        let _ = std::fs::create_dir_all(&cache);
        c.current_dir(&base).env("LIQ_CACHE_USER_DIR", &cache).env("LIQ_CACHE_SYSTEM_DIR", &cache);
    } else if let Some(pasta) = exe.parent().filter(|p| p.is_dir()) {
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

/// No Windows: amarra o Liquidsoap ao programa. Se o programa sumir de
/// qualquer jeito (fechado, travado, encerrado no Gerenciador de Tarefas), o
/// Windows fecha o Liquidsoap junto e a placa não fica tocando sozinha.
/// No Linux e no Mac, o fechamento normal já encerra o motor (e um que tenha
/// sobrado é fechado ao abrir de novo).
pub fn amarrar(_processo: &std::process::Child) {
    #[cfg(windows)]
    trabalho::amarrar(_processo);
}

#[cfg(windows)]
mod trabalho {
    use std::ffi::c_void;
    use std::os::windows::io::AsRawHandle;
    use std::sync::OnceLock;

    type Handle = *mut c_void;

    #[repr(C)]
    #[derive(Default)]
    struct LimitesBasicos {
        per_process_user_time_limit: i64,
        per_job_user_time_limit: i64,
        limit_flags: u32,
        minimum_working_set_size: usize,
        maximum_working_set_size: usize,
        active_process_limit: u32,
        affinity: usize,
        priority_class: u32,
        scheduling_class: u32,
    }

    #[repr(C)]
    #[derive(Default)]
    struct LimitesEstendidos {
        basicos: LimitesBasicos,
        io: [u64; 6],
        process_memory_limit: usize,
        job_memory_limit: usize,
        peak_process_memory_used: usize,
        peak_job_memory_used: usize,
    }

    const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION: i32 = 9;
    const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x2000;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateJobObjectW(atributos: *const c_void, nome: *const u16) -> Handle;
        fn SetInformationJobObject(job: Handle, classe: i32, info: *const c_void, tamanho: u32) -> i32;
        fn AssignProcessToJobObject(job: Handle, processo: Handle) -> i32;
    }

    /// Um "trabalho" do Windows que vive enquanto o programa viver; nunca é
    /// fechado à mão: quando o programa acaba, o Windows fecha e leva junto
    /// todos os processos dentro dele.
    fn trabalho() -> Option<usize> {
        static TRABALHO: OnceLock<Option<usize>> = OnceLock::new();
        *TRABALHO.get_or_init(|| {
            // SAFETY: chamadas simples à API do Windows com estrutura do tamanho certo.
            unsafe {
                let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if job.is_null() {
                    return None;
                }
                let mut info = LimitesEstendidos::default();
                info.basicos.limit_flags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                let ok = SetInformationJobObject(
                    job,
                    JOB_OBJECT_EXTENDED_LIMIT_INFORMATION,
                    &info as *const _ as *const c_void,
                    std::mem::size_of::<LimitesEstendidos>() as u32,
                );
                (ok != 0).then_some(job as usize)
            }
        })
    }

    pub fn amarrar(processo: &std::process::Child) -> bool {
        let Some(job) = trabalho() else { return false };
        // SAFETY: o handle do processo é válido enquanto `processo` existir.
        unsafe { AssignProcessToJobObject(job as Handle, processo.as_raw_handle() as Handle) != 0 }
    }

    #[cfg(test)]
    #[test]
    fn processo_entra_no_trabalho() {
        let mut filho = crate::sistema::comando("cmd").args(["/C", "ping -n 3 127.0.0.1 >NUL"]).spawn().unwrap();
        assert!(amarrar(&filho));
        let _ = filho.kill();
        let _ = filho.wait();
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

/// Pasta de dados no Windows: `C:\\ProgramData\\RadioOnAir`. Fica tudo nela
/// (configurações, agenda, índice, motor e registros): sem acento, que o
/// Liquidsoap de Windows não abre, e fácil de achar para fazer cópia.
fn pasta_windows() -> PathBuf {
    let base = std::env::var_os("PROGRAMDATA").map(PathBuf::from).unwrap_or_else(|| "C:\\ProgramData".into());
    base.join("RadioOnAir")
}

/// Onde o programa guarda seus dados. No Linux e no Mac, a pasta padrão do
/// sistema (`padrao`). No Windows, `C:\\ProgramData\\RadioOnAir`; quem usava a
/// versão antiga (dados em AppData) tem tudo trazido para lá na primeira vez.
pub fn pasta_dados(padrao: PathBuf) -> PathBuf {
    if !cfg!(windows) {
        return padrao;
    }
    let nova = pasta_windows();
    if !nova.join("config.json").exists() && padrao.join("config.json").exists() {
        let _ = std::fs::create_dir_all(&nova);
        let mut tudo_certo = true;
        for item in std::fs::read_dir(&padrao).into_iter().flatten().flatten() {
            let alvo = nova.join(item.file_name());
            let copiou = if alvo.exists() {
                Ok(())
            } else if item.path().is_dir() {
                copiar_pasta(&item.path(), &alvo)
            } else {
                std::fs::copy(item.path(), &alvo).map(|_| ())
            };
            tudo_certo &= copiou.is_ok();
        }
        // Só apaga a pasta antiga se tudo foi copiado.
        if tudo_certo {
            let _ = std::fs::remove_dir_all(&padrao);
        }
    }
    nova
}

/// Liquidsoap instalado numa pasta com acento (só se alguém escolher outra pasta
/// na instalação; o padrão é "Arquivos de Programas"): copia (uma vez) a pasta
/// dele para uma sem acento e devolve o novo executável.
pub fn liquidsoap_sem_acento(exe: &Path) -> PathBuf {
    if !cfg!(windows) || so_ascii(exe) {
        return exe.to_path_buf();
    }
    let (Some(origem), Some(nome)) = (exe.parent(), exe.file_name()) else { return exe.to_path_buf() };
    let destino = pasta_windows().join("liquidsoap");
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
