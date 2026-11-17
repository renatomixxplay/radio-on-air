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
    /// Abrir o programa já em tela cheia.
    pub abrir_tela_cheia: bool,
    /// Senha (hash SHA-256) para mexer nas áreas bloqueadas. `None` = sem senha.
    pub senha_hash: Option<String>,
    pub bloqueios: Bloqueios,
    /// Nome que aparece no topo do programa (e na transmissão).
    pub nome_radio: String,
    /// Entre uma música e outra, toca uma vinheta sorteada da cartucheira.
    pub vinheta_entre_musicas: bool,
    pub audio: Audio,
    pub gravacao: Gravacao,
    pub transmissao: Transmissao,
}

/// Placas de som escolhidas. `None` = a padrão do sistema. O valor é o nome do
/// dispositivo (no Linux, o nome do PulseAudio; no Mac, o nome que aparece).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Audio {
    pub saida: Option<String>,
    pub pre_escuta: Option<String>,
    /// Entrada da placa (o que vem da mesa): usada na gravação e na transmissão.
    pub entrada: Option<String>,
}

/// Gravação da entrada em MP3, um arquivo por hora (ou meia hora), numa pasta por dia.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Gravacao {
    pub ligada: bool,
    pub pasta: Option<PathBuf>,
    /// kbps (mono): 16, 24 ou 32.
    pub bitrate: u32,
    /// Tamanho de cada arquivo: 60 ou 30 minutos.
    pub minutos: u32,
}

impl Default for Gravacao {
    fn default() -> Self {
        Self { ligada: false, pasta: None, bitrate: 24, minutos: 30 }
    }
}

/// Dias que a gravação fica guardada antes de ser apagada.
pub const DIAS_GRAVACAO: i64 = 30;

/// Transmissão da entrada para um servidor Icecast, em MP3.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Transmissao {
    pub ligada: bool,
    pub servidor: String,
    pub porta: u16,
    pub senha: String,
    /// Ponto de montagem, ex.: "/live".
    pub ponto: String,
    pub bitrate: u32,
    pub samplerate: u32,
    pub estereo: bool,
}

impl Default for Transmissao {
    fn default() -> Self {
        Self {
            ligada: false,
            servidor: String::new(),
            porta: 8000,
            senha: String::new(),
            ponto: "/live".into(),
            bitrate: 128,
            samplerate: 44100,
            estereo: true,
        }
    }
}

impl Gravacao {
    pub fn validar(&mut self) -> Result<(), String> {
        if ![30, 60].contains(&self.minutos) {
            self.minutos = 60;
        }
        if ![16, 24, 32].contains(&self.bitrate) {
            self.bitrate = 24;
        }
        if self.ligada && self.pasta.is_none() {
            return Err("Escolha a pasta das gravações.".into());
        }
        Ok(())
    }
}

impl Transmissao {
    pub fn validar(&mut self) -> Result<(), String> {
        self.servidor = self.servidor.trim().trim_start_matches("http://").trim_start_matches("https://").trim_end_matches('/').to_string();
        self.ponto = self.ponto.trim().to_string();
        if !self.ponto.starts_with('/') {
            self.ponto.insert(0, '/');
        }
        if ![32, 48, 64, 96, 128, 192, 256, 320].contains(&self.bitrate) {
            self.bitrate = 128;
        }
        if ![22050, 32000, 44100, 48000].contains(&self.samplerate) {
            self.samplerate = 44100;
        }
        if self.ligada {
            if self.servidor.is_empty() || self.servidor.contains(char::is_whitespace) {
                return Err("Coloque o endereço do servidor.".into());
            }
            if self.porta == 0 {
                return Err("Coloque a porta do servidor.".into());
            }
            if self.senha.is_empty() {
                return Err("Coloque a senha da transmissão.".into());
            }
        }
        Ok(())
    }
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
            abrir_tela_cheia: false,
            senha_hash: None,
            bloqueios: Bloqueios::default(),
            nome_radio: "Radio On Air".into(),
            vinheta_entre_musicas: true,
            audio: Audio::default(),
            gravacao: Gravacao::default(),
            transmissao: Transmissao::default(),
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
        // Gravação nunca configurada (sem pasta): segue o padrão atual.
        if cfg.gravacao.pasta.is_none() {
            cfg.gravacao = Gravacao { ligada: cfg.gravacao.ligada, ..Gravacao::default() };
        }
        // Gravação já configurada com qualidade que não existe mais: a maior que existe.
        if cfg.gravacao.bitrate > 32 {
            cfg.gravacao.bitrate = 32;
        }
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
