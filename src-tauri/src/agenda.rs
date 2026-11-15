//! Agendamentos por horário e dia da semana, guardados em `agendamentos.json`.
//!
//! Tipos:
//! - **Lista de músicas**: a partir do horário, a fila puxa aleatório de uma pasta.
//! - **Comercial**: espera a música acabar e toca as pastas anexadas.
//! - **Comercial com ducking**: entra por cima da música (que abaixa quando há voz).
//! - **Hora certa**: toca o arquivo da hora atual.

use crate::biblioteca::eh_audio;
use crate::ordem;
use chrono::{Datelike, Local, NaiveDate, NaiveTime, Timelike};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TipoAgenda {
    Lista,
    Comercial,
    Ducking,
    HoraCerta,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Modo {
    Todos,
    Um,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PastaAnexada {
    pub caminho: String,
    pub modo: Modo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agendamento {
    pub id: u64,
    /// "HH:MM"
    pub horario: String,
    /// Segunda a domingo.
    pub dias: [bool; 7],
    pub tipo: TipoAgenda,
    pub pastas: Vec<PastaAnexada>,
}

impl Agendamento {
    pub fn hora_minuto(&self) -> Option<(u32, u32)> {
        let (h, m) = self.horario.split_once(':')?;
        let (h, m) = (h.trim().parse().ok()?, m.trim().parse().ok()?);
        (h < 24 && m < 60).then_some((h, m))
    }

    fn vale_no_dia(&self, dia_semana: usize) -> bool {
        self.dias.get(dia_semana).copied().unwrap_or(false)
    }
}

// ---------- arquivo ----------

pub fn carregar(dir: &Path) -> Vec<Agendamento> {
    let mut v: Vec<Agendamento> = fs::read_to_string(dir.join("agendamentos.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    ordenar(&mut v);
    v
}

pub fn salvar(dir: &Path, v: &[Agendamento]) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    let tmp = dir.join("agendamentos.json.tmp");
    fs::write(&tmp, serde_json::to_string_pretty(v).expect("serializável"))?;
    fs::rename(tmp, dir.join("agendamentos.json"))
}

pub fn ordenar(v: &mut [Agendamento]) {
    v.sort_by_key(|a| a.hora_minuto().unwrap_or((99, 99)));
}

// ---------- quando disparar ----------

/// Segunda = 0 … domingo = 6.
pub fn dia_semana(d: NaiveDate) -> usize {
    d.weekday().num_days_from_monday() as usize
}

/// Agendamentos que vencem exatamente neste minuto.
pub fn do_minuto(v: &[Agendamento], data: NaiveDate, hora: u32, minuto: u32) -> Vec<Agendamento> {
    let dia = dia_semana(data);
    v.iter()
        .filter(|a| a.vale_no_dia(dia) && a.hora_minuto() == Some((hora, minuto)))
        .cloned()
        .collect()
}

/// A lista de músicas que deveria estar valendo agora: a última que já passou
/// do horário (hoje ou, se ainda não teve hoje, nos dias anteriores).
pub fn lista_vigente(v: &[Agendamento]) -> Option<String> {
    let agora = Local::now();
    let hoje = agora.date_naive();
    let minuto_agora = agora.hour() * 60 + agora.minute();
    for volta in 0..7 {
        let data = hoje - chrono::Days::new(volta);
        let dia = dia_semana(data);
        let melhor = v
            .iter()
            .filter(|a| a.tipo == TipoAgenda::Lista && a.vale_no_dia(dia) && !a.pastas.is_empty())
            .filter_map(|a| a.hora_minuto().map(|(h, m)| (h * 60 + m, a)))
            .filter(|(m, _)| volta > 0 || *m <= minuto_agora)
            .max_by_key(|(m, _)| *m);
        if let Some((_, a)) = melhor {
            return Some(a.pastas[0].caminho.clone());
        }
    }
    None
}

/// Próximo agendamento que vai executar (listas de músicas não entram).
#[derive(Debug, Clone, Serialize)]
pub struct ProximoDto {
    pub horario: String,
    pub tipo: TipoAgenda,
    pub nome: String,
}

pub fn proximo(v: &[Agendamento]) -> Option<ProximoDto> {
    let agora = Local::now();
    let hoje = agora.date_naive();
    let minuto_agora = agora.hour() * 60 + agora.minute();
    for adiante in 0..8 {
        let data = hoje + chrono::Days::new(adiante);
        let dia = dia_semana(data);
        let achado = v
            .iter()
            .filter(|a| a.tipo != TipoAgenda::Lista && a.vale_no_dia(dia))
            .filter_map(|a| a.hora_minuto().map(|(h, m)| (h * 60 + m, a)))
            .filter(|(m, _)| adiante > 0 || *m > minuto_agora)
            .min_by_key(|(m, _)| *m);
        if let Some((_, a)) = achado {
            let nome = match a.tipo {
                TipoAgenda::HoraCerta => "Hora certa".to_string(),
                _ => a
                    .pastas
                    .iter()
                    .map(|p| Path::new(&p.caminho).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())
                    .collect::<Vec<_>>()
                    .join(" + "),
            };
            return Some(ProximoDto { horario: a.horario.clone(), tipo: a.tipo, nome });
        }
    }
    None
}

// ---------- validade no nome: "Ofertas [12-10-2025].mp3" ----------

/// Data de validade escrita no nome do arquivo, se houver.
pub fn validade(nome: &str) -> Option<NaiveDate> {
    let ini = nome.rfind('[')?;
    let fim = ini + nome[ini..].find(']')?;
    let txt = &nome[ini + 1..fim];
    NaiveDate::parse_from_str(txt.trim(), "%d-%m-%Y").ok()
}

/// Nome para mostrar na tela, sem a validade.
pub fn nome_sem_validade(nome: &str) -> String {
    match (validade(nome), nome.rfind('[')) {
        (Some(_), Some(i)) => nome[..i].trim().to_string(),
        _ => nome.to_string(),
    }
}

fn vencido(p: &Path, hoje: NaiveDate) -> bool {
    p.file_stem()
        .and_then(|s| s.to_str())
        .and_then(validade)
        .is_some_and(|d| d < hoje)
}

fn arquivos_ordenados(pasta: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(pasta)
        .map(|rd| rd.filter_map(Result::ok).map(|d| d.path()).filter(|p| p.is_file() && eh_audio(p)).collect())
        .unwrap_or_default();
    v.sort_by(|a, b| {
        let n = |p: &PathBuf| p.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        ordem::natural(&n(a), &n(b))
    });
    v
}

// ---------- rodízio ("só um") ----------

/// Último arquivo tocado de cada pasta no modo "só um" (`rodizio.json`).
#[derive(Default, Serialize, Deserialize)]
pub struct Rodizio(HashMap<String, String>);

impl Rodizio {
    pub fn carregar(dir: &Path) -> Self {
        fs::read_to_string(dir.join("rodizio.json"))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    fn salvar(&self, dir: &Path) {
        let _ = fs::write(dir.join("rodizio.json"), serde_json::to_string_pretty(self).unwrap_or_default());
    }

    /// Próximo arquivo da pasta depois do último tocado (volta ao primeiro no fim).
    fn proximo(&mut self, pasta: &str, arquivos: &[PathBuf]) -> Option<PathBuf> {
        if arquivos.is_empty() {
            return None;
        }
        let nome = |p: &PathBuf| p.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let inicio = match self.0.get(pasta) {
            // Primeiro arquivo que vem depois do último tocado na ordem alfabética.
            Some(ultimo) => arquivos
                .iter()
                .position(|p| ordem::natural(&nome(p), ultimo) == std::cmp::Ordering::Greater)
                .unwrap_or(0),
            None => 0,
        };
        let escolhido = arquivos[inicio].clone();
        self.0.insert(pasta.to_string(), nome(&escolhido));
        Some(escolhido)
    }
}

/// Monta o bloco de um comercial: pastas na ordem anexada, cada uma com todos
/// os arquivos ou o próximo do rodízio. Arquivos vencidos ficam de fora.
pub fn montar_bloco(ag: &Agendamento, rodizio: &mut Rodizio, dir_dados: &Path) -> Vec<PathBuf> {
    let hoje = Local::now().date_naive();
    let mut bloco = Vec::new();
    for pasta in &ag.pastas {
        let validos: Vec<PathBuf> = arquivos_ordenados(Path::new(&pasta.caminho))
            .into_iter()
            .filter(|p| !vencido(p, hoje))
            .collect();
        match pasta.modo {
            Modo::Todos => bloco.extend(validos),
            Modo::Um => bloco.extend(rodizio.proximo(&pasta.caminho, &validos)),
        }
    }
    rodizio.salvar(dir_dados);
    bloco
}

// ---------- hora certa ----------

/// Números que aparecem no nome do arquivo, na ordem.
fn numeros(nome: &str) -> Vec<u32> {
    nome.split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse().ok())
        .collect()
}

/// Lê hora e minuto do nome do arquivo de hora certa. Aceita, por exemplo,
/// `HRS08_MIN10.mp3`, `08-10.mp3`, `08h10.mp3`, `0810.mp3` e `8.mp3` (hora cheia).
pub fn hora_do_arquivo(nome: &str) -> Option<(u32, u32)> {
    let n = numeros(nome);
    let (h, m) = match n.as_slice() {
        [hm] if *hm >= 100 => (hm / 100, hm % 100),
        [h] => (*h, 0),
        [h, m, ..] => (*h, *m),
        [] => return None,
    };
    (h < 24 && m < 60).then_some((h, m))
}

/// Arquivo da pasta de hora certa que corresponde ao horário pedido.
pub fn arquivo_hora_certa(pasta: &Path, hora: NaiveTime) -> Option<PathBuf> {
    let alvo = (hora.hour(), hora.minute());
    arquivos_ordenados(pasta).into_iter().find(|p| {
        p.file_stem().and_then(|s| s.to_str()).and_then(hora_do_arquivo) == Some(alvo)
    })
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn le_validade_do_nome() {
        assert_eq!(validade("Ofertas xxx [12-10-2025]"), NaiveDate::from_ymd_opt(2025, 10, 12));
        assert_eq!(validade("Padaria"), None);
        assert_eq!(validade("Banda [Rock]"), None);
        assert_eq!(nome_sem_validade("Ofertas xxx [12-10-2025]"), "Ofertas xxx");
        assert_eq!(nome_sem_validade("Banda [Rock]"), "Banda [Rock]");
    }

    #[test]
    fn le_hora_certa_em_varios_formatos() {
        assert_eq!(hora_do_arquivo("HRS08_MIN10"), Some((8, 10)));
        assert_eq!(hora_do_arquivo("08-10"), Some((8, 10)));
        assert_eq!(hora_do_arquivo("08h10"), Some((8, 10)));
        assert_eq!(hora_do_arquivo("0810"), Some((8, 10)));
        assert_eq!(hora_do_arquivo("8"), Some((8, 0)));
        assert_eq!(hora_do_arquivo("hora certa"), None);
        assert_eq!(hora_do_arquivo("25-99"), None);
    }

    #[test]
    fn rodizio_anda_e_volta_ao_comeco() {
        let arq: Vec<PathBuf> = ["a.mp3", "b.mp3", "c.mp3"].iter().map(PathBuf::from).collect();
        let mut r = Rodizio::default();
        let seq: Vec<String> = (0..4)
            .map(|_| r.proximo("p", &arq).unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(seq, ["a.mp3", "b.mp3", "c.mp3", "a.mp3"]);
    }

    #[test]
    fn rodizio_continua_se_o_ultimo_saiu_da_pasta() {
        let mut r = Rodizio::default();
        r.0.insert("p".into(), "b.mp3".into());
        let arq: Vec<PathBuf> = ["a.mp3", "c.mp3"].iter().map(PathBuf::from).collect();
        assert_eq!(r.proximo("p", &arq).unwrap(), PathBuf::from("c.mp3"));
    }
}
