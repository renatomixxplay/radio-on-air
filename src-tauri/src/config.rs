//! Configurações do programa, guardadas em `config.json` (arquivo de texto) na pasta de dados.

use chrono::NaiveDate;
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
    /// Arquivo de cada botão da cartucheira padrão (sempre 8 posições).
    pub cartucheira: Vec<Option<PathBuf>>,
    /// Cartucheiras com data de início e fim: no período, entram no lugar da padrão.
    pub cartucheiras_agendadas: Vec<CartucheiraAgendada>,
    /// Pastas que aparecem no painel "Pastas" da tela No ar.
    pub pastas: Vec<PathBuf>,
    /// Abertura: ao abrir o programa, toca todos os áudios desta pasta (em
    /// ordem) antes de qualquer outra coisa. Com pasta, entra no ar sozinho.
    pub pasta_abertura: Option<PathBuf>,
    pub encerramento: Encerramento,
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

/// Cartucheira que vale só entre duas datas (incluindo os dois dias).
/// As datas ficam como texto "AAAA-MM-DD", o mesmo do calendário da tela.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CartucheiraAgendada {
    pub id: u64,
    pub nome: String,
    pub inicio: String,
    pub fim: String,
    /// Sempre 8 posições. Botão vazio fica vazio (não usa o da padrão).
    pub botoes: Vec<Option<PathBuf>>,
}

impl CartucheiraAgendada {
    fn periodo(&self) -> Option<(NaiveDate, NaiveDate)> {
        let d = |t: &str| NaiveDate::parse_from_str(t, "%Y-%m-%d").ok();
        Some((d(&self.inicio)?, d(&self.fim)?))
    }

    pub fn vale_em(&self, dia: NaiveDate) -> bool {
        self.periodo().is_some_and(|(i, f)| i <= dia && dia <= f)
    }
}

/// Encerramento: no horário do dia, a música sai com fade, tocam todos os
/// áudios da pasta (em ordem) e a rádio para até alguém apertar Play.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Encerramento {
    pub pasta: Option<PathBuf>,
    /// Horário "HH:MM" de cada dia, de segunda a domingo. `None` = não encerra nesse dia.
    pub horarios: [Option<String>; 7],
}

/// "HH:MM" → minutos do dia (0 = 00:00).
fn minuto_de(txt: &str) -> Option<u32> {
    let (h, m) = txt.split_once(':')?;
    let (h, m): (u32, u32) = (h.trim().parse().ok()?, m.trim().parse().ok()?);
    (h < 24 && m < 60).then_some(h * 60 + m)
}

impl Encerramento {
    /// Encerra nesta data, neste minuto (pela regra do dia da semana).
    pub fn vence(&self, data: NaiveDate, hora: u32, minuto: u32) -> bool {
        use chrono::Datelike;
        let dia = data.weekday().num_days_from_monday() as usize;
        self.pasta.is_some() && self.horarios[dia].as_deref().and_then(minuto_de) == Some(hora * 60 + minuto)
    }

    /// Horários no formato HH:MM; dia marcado pede pasta.
    pub fn validar(&mut self) -> Result<(), String> {
        const DIAS: [&str; 7] = ["segunda", "terça", "quarta", "quinta", "sexta", "sábado", "domingo"];
        for (i, h) in self.horarios.iter_mut().enumerate() {
            let Some(txt) = h.as_deref() else { continue };
            let m = minuto_de(txt).ok_or_else(|| format!("Horário inválido na {}: {txt}.", DIAS[i]))?;
            *h = Some(format!("{:02}:{:02}", m / 60, m % 60));
        }
        if self.pasta.is_none() && self.horarios.iter().any(Option::is_some) {
            return Err("Escolha a pasta do encerramento.".into());
        }
        Ok(())
    }
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
    /// Qual canal da entrada vale para a gravação e a transmissão.
    pub canal: Canal,
}

/// Canal da entrada. Mesa ligada só num lado da placa: com "Estéreo" o som
/// sai com metade do volume (a média com o lado mudo); "Mono L"/"Mono R"
/// usam só aquele lado, nos dois canais.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Canal {
    #[default]
    Estereo,
    Esquerdo,
    Direito,
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
            cartucheiras_agendadas: vec![],
            pastas: vec![],
            pasta_abertura: None,
            encerramento: Encerramento::default(),
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
        for a in &mut cfg.cartucheiras_agendadas {
            a.botoes.resize(SLOTS_CARTUCHEIRA, None);
        }
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

    /// A agendada que vale no dia, se houver.
    pub fn agendada_do_dia(&self, dia: NaiveDate) -> Option<&CartucheiraAgendada> {
        self.cartucheiras_agendadas.iter().find(|a| a.vale_em(dia))
    }

    /// Os 8 botões no ar no dia: os da agendada do período ou, fora dele, os da padrão.
    pub fn botoes_do_dia(&self, dia: NaiveDate) -> &[Option<PathBuf>] {
        self.agendada_do_dia(dia).map(|a| &a.botoes).unwrap_or(&self.cartucheira)
    }

    /// Cria (id 0) ou altera nome e datas de uma agendada. Recusa datas
    /// inválidas, fim antes do início e período com dia em comum com outra.
    pub fn salvar_agendada(&mut self, id: u64, nome: &str, inicio: &str, fim: &str) -> Result<u64, String> {
        let nome = nome.trim();
        if nome.is_empty() {
            return Err("Dê um nome para a cartucheira.".into());
        }
        let nova = CartucheiraAgendada { id, nome: nome.into(), inicio: inicio.into(), fim: fim.into(), botoes: vec![] };
        let (i, f) = nova.periodo().ok_or("Escolha a data de início e a de fim.")?;
        if f < i {
            return Err("A data de fim é antes da de início.".into());
        }
        let br = |d: NaiveDate| d.format("%d/%m/%Y").to_string();
        for o in self.cartucheiras_agendadas.iter().filter(|o| o.id != id) {
            if let Some((oi, of)) = o.periodo() {
                if i <= of && oi <= f {
                    return Err(format!("Tem dia em comum com \"{}\" ({} a {}).", o.nome, br(oi), br(of)));
                }
            }
        }
        if id == 0 {
            let novo_id = self.cartucheiras_agendadas.iter().map(|a| a.id).max().unwrap_or(0) + 1;
            self.cartucheiras_agendadas.push(CartucheiraAgendada {
                id: novo_id,
                botoes: vec![None; SLOTS_CARTUCHEIRA],
                ..nova
            });
            Ok(novo_id)
        } else {
            let a = self.cartucheiras_agendadas.iter_mut().find(|a| a.id == id).ok_or("Cartucheira não encontrada.")?;
            a.nome = nova.nome;
            a.inicio = nova.inicio;
            a.fim = nova.fim;
            Ok(id)
        }
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

#[cfg(test)]
mod testes_agendadas {
    use super::*;

    fn dia(t: &str) -> NaiveDate {
        NaiveDate::parse_from_str(t, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn agendada_vale_no_periodo_incluindo_os_dois_dias() {
        let mut c = Config::default();
        c.cartucheira[0] = Some("padrao.mp3".into());
        let id = c.salvar_agendada(0, "Natal", "2026-12-20", "2026-12-26").unwrap();
        c.cartucheiras_agendadas[0].botoes[1] = Some("natal.mp3".into());
        assert_eq!(c.agendada_do_dia(dia("2026-12-19")).map(|a| a.id), None);
        assert_eq!(c.agendada_do_dia(dia("2026-12-20")).map(|a| a.id), Some(id));
        assert_eq!(c.agendada_do_dia(dia("2026-12-26")).map(|a| a.id), Some(id));
        assert_eq!(c.agendada_do_dia(dia("2026-12-27")).map(|a| a.id), None);
        // Na agendada, botão vazio fica vazio.
        let b = c.botoes_do_dia(dia("2026-12-25"));
        assert_eq!(b[0], None);
        assert_eq!(b[1], Some("natal.mp3".into()));
        assert_eq!(c.botoes_do_dia(dia("2026-12-27"))[0], Some("padrao.mp3".into()));
    }

    #[test]
    fn recusa_dia_em_comum_e_datas_ruins() {
        let mut c = Config::default();
        c.salvar_agendada(0, "Natal", "2026-12-20", "2026-12-26").unwrap();
        assert!(c.salvar_agendada(0, "Ano Novo", "2026-12-26", "2027-01-02").is_err());
        assert!(c.salvar_agendada(0, "Dezembro", "2026-12-01", "2026-12-31").is_err());
        assert!(c.salvar_agendada(0, "Ao contrário", "2027-01-05", "2027-01-02").is_err());
        assert!(c.salvar_agendada(0, "Sem data", "", "2027-01-02").is_err());
        assert!(c.salvar_agendada(0, "  ", "2027-01-01", "2027-01-02").is_err());
        let id = c.salvar_agendada(0, "Ano Novo", "2026-12-27", "2027-01-02").unwrap();
        // Editar a própria não conta como dia em comum com ela mesma.
        c.salvar_agendada(id, "Ano Novo", "2026-12-28", "2027-01-03").unwrap();
        assert_eq!(c.cartucheiras_agendadas.len(), 2);
    }

    #[test]
    fn config_antiga_sem_agendadas_abre_igual() {
        let c: Config = serde_json::from_str(r#"{"cartucheira":["a.mp3",null]}"#).unwrap();
        assert!(c.cartucheiras_agendadas.is_empty());
        assert_eq!(c.cartucheira[0], Some("a.mp3".into()));
    }
}

#[cfg(test)]
mod testes_encerramento {
    use super::*;

    fn dia(t: &str) -> NaiveDate {
        NaiveDate::parse_from_str(t, "%Y-%m-%d").unwrap()
    }

    /// O exemplo da rádio: segunda a sábado às 02:00 e domingo às 00:00.
    fn exemplo() -> Encerramento {
        let mut h: [Option<String>; 7] = Default::default();
        for d in h.iter_mut().take(6) {
            *d = Some("02:00".into());
        }
        h[6] = Some("00:00".into());
        Encerramento { pasta: Some("/encerramento".into()), horarios: h }
    }

    #[test]
    fn cada_dia_no_seu_horario() {
        let e = exemplo();
        // 2026-11-23 é segunda; 2026-11-28 é sábado; 2026-11-29 é domingo.
        assert!(e.vence(dia("2026-11-23"), 2, 0));
        assert!(e.vence(dia("2026-11-28"), 2, 0));
        assert!(!e.vence(dia("2026-11-23"), 0, 0));
        assert!(!e.vence(dia("2026-11-23"), 2, 1));
        assert!(!e.vence(dia("2026-11-23"), 1, 59));
        assert!(e.vence(dia("2026-11-29"), 0, 0));
        assert!(!e.vence(dia("2026-11-29"), 2, 0));
    }

    #[test]
    fn dia_desmarcado_ou_sem_pasta_nao_encerra() {
        let mut e = exemplo();
        e.horarios[2] = None; // quarta
        assert!(!e.vence(dia("2026-11-25"), 2, 0));
        assert!(e.vence(dia("2026-11-26"), 2, 0));
        e.pasta = None;
        assert!(!e.vence(dia("2026-11-26"), 2, 0));
    }

    #[test]
    fn validar_arruma_horario_e_pede_pasta() {
        let mut e = exemplo();
        e.horarios[0] = Some("2:5".into());
        e.validar().unwrap();
        assert_eq!(e.horarios[0].as_deref(), Some("02:05"));
        e.horarios[1] = Some("25:00".into());
        assert!(e.validar().is_err());
        let mut sem_pasta = exemplo();
        sem_pasta.pasta = None;
        assert!(sem_pasta.validar().is_err());
        // Sem pasta e sem horário: desligado, pode salvar.
        assert!(Encerramento::default().validar().is_ok());
    }

    #[test]
    fn config_antiga_abre_sem_abertura_nem_encerramento() {
        let c: Config = serde_json::from_str(r#"{"tocar_ao_abrir":true,"cartucheira":["a.mp3"]}"#).unwrap();
        assert_eq!(c.cartucheira[0], Some(PathBuf::from("a.mp3")));
        assert_eq!(c.pasta_abertura, None);
        assert_eq!(c.encerramento, Encerramento::default());
    }
}
