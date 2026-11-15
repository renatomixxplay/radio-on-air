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
}

impl Default for Config {
    fn default() -> Self {
        Self {
            pasta_musicas: None,
            pasta_programas: None,
            cartucheira: vec![None; SLOTS_CARTUCHEIRA],
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
