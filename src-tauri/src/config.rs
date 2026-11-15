//! Configurações do programa, guardadas em `config.json` (arquivo de texto) na pasta de dados.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const SLOTS_CARTUCHEIRA: usize = 8;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Pasta raiz do acervo de músicas (com subpastas).
    pub pasta_musicas: Option<PathBuf>,
    /// Pasta com uma subpasta por programa/locutor.
    pub pasta_programas: Option<PathBuf>,
    /// Arquivo de cada botão da cartucheira (sempre 8 posições).
    pub cartucheira: Vec<Option<PathBuf>>,
    /// Pastas que aparecem no painel "Pastas" da tela No ar.
    pub pastas: Vec<PathBuf>,
    /// Entrar no ar sozinho ao abrir o programa.
    pub tocar_ao_abrir: bool,
    /// Senha (hash SHA-256) para mexer nas áreas bloqueadas. `None` = sem senha.
    pub senha_hash: Option<String>,
    pub bloqueios: Bloqueios,
}

/// Áreas que pedem senha quando a proteção está ligada.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Bloqueios {
    pub agendamentos: bool,
    pub pastas: bool,
    pub cartucheira: bool,
    pub configuracoes: bool,
}

pub fn hash_senha(senha: &str) -> String {
    use sha2::{Digest, Sha256};
    let d = Sha256::digest(format!("radio-on-air:{senha}").as_bytes());
    d.iter().map(|b| format!("{b:02x}")).collect()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            pasta_musicas: None,
            pasta_programas: None,
            cartucheira: vec![None; SLOTS_CARTUCHEIRA],
            pastas: vec![],
            tocar_ao_abrir: false,
            senha_hash: None,
            bloqueios: Bloqueios::default(),
        }
    }
}

impl Config {
    fn arquivo(dir: &Path) -> PathBuf {
        dir.join("config.json")
    }

    pub fn carregar(dir: &Path) -> Self {
        let mut cfg: Config = fs::read_to_string(Self::arquivo(dir))
            .ok()
            .and_then(|txt| serde_json::from_str(&txt).ok())
            .unwrap_or_default();
        cfg.cartucheira.resize(SLOTS_CARTUCHEIRA, None);
        // Antes havia uma "pasta de programas" com subpastas: vira a lista de pastas.
        if cfg.pastas.is_empty() {
            if let Some(raiz) = cfg.pasta_programas.take() {
                if let Ok(rd) = fs::read_dir(&raiz) {
                    cfg.pastas = rd.filter_map(Result::ok).map(|d| d.path()).filter(|p| p.is_dir()).collect();
                }
            }
        }
        cfg
    }

    pub fn salvar(&self, dir: &Path) -> std::io::Result<()> {
        fs::create_dir_all(dir)?;
        let txt = serde_json::to_string_pretty(self).expect("config serializável");
        // Escreve num temporário e renomeia: não corrompe se faltar luz no meio.
        let tmp = dir.join("config.json.tmp");
        fs::write(&tmp, txt)?;
        fs::rename(tmp, Self::arquivo(dir))
    }
}
