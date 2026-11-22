//! Agendamentos por horário e dia da semana, guardados em `agendamentos.json`.
//!
//! Tipos:
//! - **Lista de músicas**: a partir do horário, a fila puxa aleatório de uma pasta.
//! - **Comercial**: espera a música acabar e toca as pastas anexadas.
//! - **Comercial com ducking**: entra por cima da música (que abaixa quando há voz).
//! - **Hora certa**: toca o arquivo da hora atual.
//! - **Link**: entra um link ao vivo (MP3/AAC) até a próxima lista ou link.

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
    Link,
}

/// Como a lista de músicas entra. Nos dois casos a fila é limpa e recebe
/// músicas novas da pasta.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModoLista {
    /// A música atual termina normalmente.
    #[default]
    DepoisDesta,
    /// Fade de 3 s na música atual e a lista já começa.
    Fade,
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
    /// "HH:MM", em ordem e sem repetir. A hora certa não usa.
    #[serde(default)]
    pub horarios: Vec<String>,
    /// Segunda a domingo.
    pub dias: [bool; 7],
    pub tipo: TipoAgenda,
    /// Na hora certa, a pasta dos arquivos de hora (opcional).
    pub pastas: Vec<PastaAnexada>,
    /// Hora certa e link: pasta da abertura, toca um arquivo antes (opcional).
    #[serde(default)]
    pub abertura: Option<String>,
    /// Hora certa: pasta do comercial, toca um arquivo depois da hora (opcional).
    #[serde(default)]
    pub comercial_hora: Option<String>,
    /// Hora certa: toca a cada tantos minutos, alinhado ao relógio
    /// (15 = :00, :15, :30, :45). O horário não é usado.
    #[serde(default)]
    pub intervalo: Option<u32>,
    /// Lista: como entra.
    #[serde(default)]
    pub modo_lista: ModoLista,
    /// Link: endereço do áudio ao vivo (MP3/AAC).
    #[serde(default)]
    pub url: Option<String>,
    /// Link: ajuste de volume em dB (0 = como chega; -6 = abaixa).
    #[serde(default)]
    pub volume_db: f64,
    /// Desativado: fica na lista mas não toca (nem conta como horário ocupado).
    #[serde(default = "ativo_padrao")]
    pub ativo: bool,
    /// Não toca nos feriados de data fixa nem nas `outras_datas`.
    #[serde(default)]
    pub pula_feriado: bool,
    /// Datas a mais em que não toca ("AAAA-MM-DD"), com `pula_feriado` ligado.
    #[serde(default)]
    pub outras_datas: Vec<String>,
}

fn ativo_padrao() -> bool {
    true
}

/// Feriados nacionais de data fixa (dia, mês).
pub const FERIADOS_FIXOS: [(u32, u32); 9] = [(1, 1), (21, 4), (1, 5), (7, 9), (12, 10), (2, 11), (15, 11), (20, 11), (25, 12)];

pub fn feriado_fixo(data: NaiveDate) -> bool {
    FERIADOS_FIXOS.contains(&(data.day(), data.month()))
}

fn data_de(txt: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(txt.trim(), "%Y-%m-%d").ok()
}

/// Intervalos da hora certa: a cada 30 min ou só na hora cheia.
pub const INTERVALOS: [u32; 2] = [30, 60];

/// "HH:MM" → minutos do dia (0 = 00:00).
fn minuto_de(txt: &str) -> Option<u32> {
    let (h, m) = txt.split_once(':')?;
    let (h, m): (u32, u32) = (h.trim().parse().ok()?, m.trim().parse().ok()?);
    (h < 24 && m < 60).then_some(h * 60 + m)
}

fn hhmm(minuto: u32) -> String {
    format!("{:02}:{:02}", minuto / 60, minuto % 60)
}

impl Agendamento {
    /// Toca nesta data: ativo, dia da semana marcado e, se pula feriado,
    /// não é feriado de data fixa nem uma das outras datas.
    fn vale_na_data(&self, data: NaiveDate) -> bool {
        if !self.ativo || !self.dias.get(dia_semana(data)).copied().unwrap_or(false) {
            return false;
        }
        !(self.pula_feriado && (feriado_fixo(data) || self.outras_datas.iter().any(|d| data_de(d) == Some(data))))
    }

    /// Minutos do dia (0 = 00:00) em que dispara, em ordem.
    pub fn minutos_do_dia(&self) -> Vec<u32> {
        match (self.tipo, self.intervalo) {
            (TipoAgenda::HoraCerta, Some(n)) if n > 0 => (0..24 * 60).step_by(n as usize).collect(),
            (TipoAgenda::HoraCerta, _) => vec![],
            _ => {
                let mut v: Vec<u32> = self.horarios.iter().filter_map(|h| minuto_de(h)).collect();
                v.sort_unstable();
                v.dedup();
                v
            }
        }
    }

    /// Tem alguma pasta escolhida (na hora certa, qualquer um dos três campos).
    pub fn tem_pasta(&self) -> bool {
        !self.pastas.is_empty()
            || (self.tipo == TipoAgenda::HoraCerta && (self.abertura.is_some() || self.comercial_hora.is_some()))
    }

    /// Apaga os campos que não valem para o tipo e confere o que falta.
    pub fn validar(&mut self) -> Result<(), String> {
        let tipo = self.tipo;
        if !matches!(tipo, TipoAgenda::HoraCerta | TipoAgenda::Link) {
            self.abertura = None;
        }
        if tipo != TipoAgenda::HoraCerta {
            self.comercial_hora = None;
            self.intervalo = None;
        }
        if tipo != TipoAgenda::Lista {
            self.modo_lista = ModoLista::default();
        }
        if tipo == TipoAgenda::Link {
            self.pastas.clear();
            let url = self.url.as_deref().map(str::trim).unwrap_or_default().to_string();
            if !(url.starts_with("http://") || url.starts_with("https://")) || url.contains(char::is_whitespace) {
                return Err("Coloque o endereço do link (começa com http:// ou https://).".into());
            }
            self.url = Some(url);
        } else {
            self.url = None;
        }
        if tipo == TipoAgenda::HoraCerta {
            if !self.intervalo.is_some_and(|n| INTERVALOS.contains(&n)) {
                return Err("Escolha de quanto em quanto tempo toca.".into());
            }
            self.horarios.clear();
        } else {
            if let Some(ruim) = self.horarios.iter().find(|h| minuto_de(h).is_none()) {
                return Err(format!("Horário inválido: {ruim}."));
            }
            // Em ordem, no formato HH:MM e sem repetir.
            self.horarios = self.minutos_do_dia().into_iter().map(hhmm).collect();
            if self.horarios.is_empty() {
                return Err("Coloque pelo menos um horário.".into());
            }
        }
        // Outras datas: em ordem, sem repetir e sem as que já passaram.
        let hoje = Local::now().date_naive();
        let mut datas: Vec<NaiveDate> = self.outras_datas.iter().filter_map(|d| data_de(d)).filter(|d| *d >= hoje).collect();
        datas.sort_unstable();
        datas.dedup();
        self.outras_datas = datas.iter().map(|d| d.format("%Y-%m-%d").to_string()).collect();
        if !self.dias.iter().any(|d| *d) {
            return Err("Marque pelo menos um dia.".into());
        }
        if tipo != TipoAgenda::Link && !self.tem_pasta() {
            return Err("Escolha pelo menos uma pasta.".into());
        }
        Ok(())
    }
}

const DIAS_NOME: [&str; 7] = ["Seg", "Ter", "Qua", "Qui", "Sex", "Sáb", "Dom"];

/// Tipos que não podem cair no mesmo horário e dia. Comercial e hora certa
/// entram em fila e podem dividir horário com qualquer um.
fn grupo_exclusivo(t: TipoAgenda) -> Option<u8> {
    match t {
        // Dois por cima da música ao mesmo tempo atropelam a música.
        TipoAgenda::Ducking => Some(1),
        // Lista e link trocam o que está no ar: um desfaria o outro.
        TipoAgenda::Lista | TipoAgenda::Link => Some(2),
        TipoAgenda::Comercial | TipoAgenda::HoraCerta => None,
    }
}

fn nome_tipo(t: TipoAgenda) -> &'static str {
    match t {
        TipoAgenda::Ducking => "um comercial com ducking",
        TipoAgenda::Lista => "uma lista",
        TipoAgenda::Link => "um link",
        TipoAgenda::Comercial => "um comercial",
        TipoAgenda::HoraCerta => "uma hora certa",
    }
}

/// Confere se o agendamento pode ser salvo junto com os outros. A hora certa
/// é uma só; ducking não cai junto com ducking; lista e link não caem juntos.
pub fn conflito(v: &[Agendamento], ag: &Agendamento) -> Option<String> {
    // Desativado não ocupa horário; ao ativar de novo, confere.
    if !ag.ativo {
        return None;
    }
    let outros = v.iter().filter(|o| o.id != ag.id && o.ativo);
    if ag.tipo == TipoAgenda::HoraCerta {
        return outros
            .clone()
            .any(|o| o.tipo == TipoAgenda::HoraCerta)
            .then(|| "Já existe uma hora certa. Só pode ter uma.".to_string());
    }
    let grupo = grupo_exclusivo(ag.tipo)?;
    let meus = ag.minutos_do_dia();
    outros.filter(|o| grupo_exclusivo(o.tipo) == Some(grupo)).find_map(|o| {
        let dias: Vec<&str> = (0..7).filter(|&i| o.dias[i] && ag.dias[i]).map(|i| DIAS_NOME[i]).collect();
        if dias.is_empty() {
            return None;
        }
        let minuto = o.minutos_do_dia().into_iter().find(|m| meus.contains(m))?;
        Some(format!("Já existe {} às {} ({}).", nome_tipo(o.tipo), hhmm(minuto), dias.join(", ")))
    })
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

/// Hora certa no topo (não tem horário), o resto pelo primeiro horário.
pub fn ordenar(v: &mut [Agendamento]) {
    v.sort_by_key(|a| (a.tipo != TipoAgenda::HoraCerta, a.minutos_do_dia().first().copied().unwrap_or(u32::MAX)));
}

// ---------- quando disparar ----------

/// Segunda = 0 … domingo = 6.
pub fn dia_semana(d: NaiveDate) -> usize {
    d.weekday().num_days_from_monday() as usize
}

/// Agendamentos que vencem exatamente neste minuto, na ordem em que disparam:
/// 1. Lista de músicas: tira do ar o link que estiver tocando, para a hora
///    certa e o comercial do mesmo minuto tocarem (antes das músicas novas).
/// 2. Link: o comercial e a hora certa do mesmo minuto são descartados.
/// 3. Hora certa: não atrasa se dividir o horário com um comercial.
/// 4. Comercial e comercial com ducking.
pub fn do_minuto(v: &[Agendamento], data: NaiveDate, hora: u32, minuto: u32) -> Vec<Agendamento> {
    let agora = hora * 60 + minuto;
    let mut vencem: Vec<Agendamento> = v
        .iter()
        .filter(|a| a.vale_na_data(data) && a.minutos_do_dia().contains(&agora))
        .cloned()
        .collect();
    vencem.sort_by_key(|a| match a.tipo {
        TipoAgenda::Lista => 0,
        TipoAgenda::Link => 1,
        TipoAgenda::HoraCerta => 2,
        TipoAgenda::Comercial | TipoAgenda::Ducking => 3,
    });
    vencem
}

/// Pastas da lista de músicas que deveria estar valendo agora: a última que já
/// passou do horário (hoje ou, se ainda não teve hoje, nos dias anteriores).
/// Vazio: nenhuma lista.
pub fn lista_vigente(v: &[Agendamento]) -> Vec<String> {
    let agora = Local::now();
    let hoje = agora.date_naive();
    let minuto_agora = agora.hour() * 60 + agora.minute();
    for volta in 0..7 {
        let data = hoje - chrono::Days::new(volta);
        let melhor = v
            .iter()
            .filter(|a| a.tipo == TipoAgenda::Lista && a.vale_na_data(data) && !a.pastas.is_empty())
            .flat_map(|a| a.minutos_do_dia().into_iter().map(move |m| (m, a)))
            .filter(|(m, _)| volta > 0 || *m <= minuto_agora)
            .max_by_key(|(m, _)| *m);
        if let Some((_, a)) = melhor {
            return a.pastas.iter().map(|p| p.caminho.clone()).collect();
        }
    }
    vec![]
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
    for adiante in 0..15 {
        let data = hoje + chrono::Days::new(adiante);
        let achado = v
            .iter()
            .filter(|a| a.tipo != TipoAgenda::Lista && a.vale_na_data(data))
            .flat_map(|a| a.minutos_do_dia().into_iter().map(move |m| (m, a)))
            .filter(|(m, _)| adiante > 0 || *m > minuto_agora)
            // No mesmo minuto, a hora certa vem primeiro (é o que toca antes).
            .min_by_key(|(m, a)| (*m, a.tipo != TipoAgenda::HoraCerta));
        if let Some((minuto, a)) = achado {
            let nome = match a.tipo {
                TipoAgenda::HoraCerta => "Hora certa".to_string(),
                TipoAgenda::Link => "Link ao vivo".to_string(),
                _ => a
                    .pastas
                    .iter()
                    .map(|p| Path::new(&p.caminho).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())
                    .collect::<Vec<_>>()
                    .join(" + "),
            };
            let horario = format!("{:02}:{:02}", minuto / 60, minuto % 60);
            return Some(ProximoDto { horario, tipo: a.tipo, nome });
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

fn tem_data(p: &Path) -> bool {
    p.file_stem().and_then(|s| s.to_str()).and_then(validade).is_some()
}

fn validos(pasta: &str, hoje: NaiveDate) -> Vec<PathBuf> {
    arquivos_ordenados(Path::new(pasta)).into_iter().filter(|p| !vencido(p, hoje)).collect()
}

/// Todos os áudios da pasta, em ordem de nome, sem os vencidos (abertura e encerramento).
pub fn todos_da_pasta(pasta: &Path) -> Vec<PathBuf> {
    let hoje = Local::now().date_naive();
    arquivos_ordenados(pasta).into_iter().filter(|p| !vencido(p, hoje)).collect()
}

/// Modo "só um": arquivo com data (ainda válida) tem prioridade. Só quando
/// não há nenhum com data é que entra o rodízio dos sem data.
fn escolher_um(rodizio: &mut Rodizio, pasta: &str, validos: Vec<PathBuf>) -> Option<PathBuf> {
    let (com_data, sem_data): (Vec<PathBuf>, Vec<PathBuf>) = validos.into_iter().partition(|p| tem_data(p));
    let candidatos = if com_data.is_empty() { sem_data } else { com_data };
    rodizio.proximo(pasta, &candidatos)
}

/// Monta o bloco de um comercial: pastas na ordem anexada, cada uma com todos
/// os arquivos ou um só (data primeiro, depois rodízio). Vencidos ficam de fora.
pub fn montar_bloco(ag: &Agendamento, rodizio: &mut Rodizio, dir_dados: &Path) -> Vec<PathBuf> {
    let hoje = Local::now().date_naive();
    let mut bloco = Vec::new();
    for pasta in &ag.pastas {
        let validos = validos(&pasta.caminho, hoje);
        match pasta.modo {
            Modo::Todos => bloco.extend(validos),
            Modo::Um => bloco.extend(escolher_um(rodizio, &pasta.caminho, validos)),
        }
    }
    rodizio.salvar(dir_dados);
    bloco
}

/// Um arquivo da pasta, como no modo "só um" (com data primeiro, depois rodízio).
pub fn um_da_pasta(pasta: &str, rodizio: &mut Rodizio, dir_dados: &Path) -> Option<PathBuf> {
    let escolhido = escolher_um(rodizio, pasta, validos(pasta, Local::now().date_naive()));
    rodizio.salvar(dir_dados);
    escolhido
}

/// Hora certa: um arquivo da abertura, os arquivos da hora e um do comercial.
/// Cada parte é opcional. `inicio` é quando o bloco deve começar a tocar (fim
/// da música atual): a hora falada é a desse momento, depois da abertura.
/// Devolve (abertura + hora, comercial).
pub fn montar_hora_certa(
    ag: &Agendamento,
    inicio: NaiveTime,
    rodizio: &mut Rodizio,
    dir_dados: &Path,
) -> (Vec<PathBuf>, Option<PathBuf>) {
    let hoje = Local::now().date_naive();
    let mut um = |pasta: &Option<String>| pasta.as_ref().and_then(|p| escolher_um(rodizio, p, validos(p, hoje)));
    let abertura = um(&ag.abertura);
    let comercial = um(&ag.comercial_hora);
    rodizio.salvar(dir_dados);
    let dura_abertura = abertura.as_deref().and_then(crate::analise::duracao).unwrap_or(0.0);
    let falada = inicio + chrono::Duration::milliseconds((dura_abertura * 1000.0) as i64);
    let arquivos_hora = ag
        .pastas
        .first()
        .map(|p| arquivos_hora_certa(Path::new(&p.caminho), falada))
        .unwrap_or_default();
    (abertura.into_iter().chain(arquivos_hora).collect(), comercial)
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

fn nome_sem_ext(p: &Path) -> String {
    p.file_stem().map(|s| s.to_string_lossy().to_uppercase()).unwrap_or_default()
}

/// Arquivos da pasta de hora certa para o horário pedido. Aceita um arquivo só
/// com hora e minuto (`08-10.mp3`) ou separados: `HRS08.mp3` + `MIN10.mp3`
/// (na hora cheia, só o `HRS08`).
pub fn arquivos_hora_certa(pasta: &Path, hora: NaiveTime) -> Vec<PathBuf> {
    let (h, m) = (hora.hour(), hora.minute());
    let arquivos = arquivos_ordenados(pasta);
    let eh_minuto = |p: &PathBuf| nome_sem_ext(p).starts_with("MIN");
    // Um arquivo com a hora e o minuto juntos.
    if let Some(p) = arquivos.iter().find(|p| !eh_minuto(p) && hora_do_arquivo(&nome_sem_ext(p)) == Some((h, m))) {
        return vec![p.clone()];
    }
    // Separados: arquivo da hora e arquivo do minuto.
    let achar = |prefixo: &str, n: u32| {
        arquivos.iter().find(|p| {
            let nome = nome_sem_ext(p);
            nome.starts_with(prefixo) && numeros(&nome) == [n]
        })
    };
    let Some(arq_hora) = achar("HRS", h) else { return vec![] };
    let mut v = vec![arq_hora.clone()];
    if m > 0 {
        v.extend(achar("MIN", m).cloned());
    }
    v
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
    fn so_um_prioriza_arquivo_com_data() {
        let arq = |nomes: &[&str]| nomes.iter().map(PathBuf::from).collect::<Vec<_>>();
        let mut r = Rodizio::default();
        // Com data ganha do sem data, sempre.
        let v = arq(&["A.mp3", "Oferta [31-12-2099].mp3"]);
        assert_eq!(escolher_um(&mut r, "p", v.clone()).unwrap(), PathBuf::from("Oferta [31-12-2099].mp3"));
        assert_eq!(escolher_um(&mut r, "p", v).unwrap(), PathBuf::from("Oferta [31-12-2099].mp3"));
        // Vários com data: rodízio entre eles.
        let v = arq(&["A.mp3", "X [31-12-2099].mp3", "Y [31-12-2099].mp3"]);
        let mut r = Rodizio::default();
        let seq: Vec<_> = (0..3).map(|_| escolher_um(&mut r, "p", v.clone()).unwrap()).collect();
        assert_eq!(seq, arq(&["X [31-12-2099].mp3", "Y [31-12-2099].mp3", "X [31-12-2099].mp3"]));
        // Nenhum com data: rodízio dos sem data.
        let v = arq(&["A.mp3", "B.mp3"]);
        let mut r = Rodizio::default();
        let seq: Vec<_> = (0..3).map(|_| escolher_um(&mut r, "p", v.clone()).unwrap()).collect();
        assert_eq!(seq, arq(&["A.mp3", "B.mp3", "A.mp3"]));
    }

    fn ag(id: u64, horario: &str, dias: [bool; 7], tipo: TipoAgenda) -> Agendamento {
        let horarios = horario.split(',').map(|h| h.trim().to_string()).collect();
        Agendamento { id, horarios, dias, tipo, pastas: vec![], abertura: None, comercial_hora: None, intervalo: None, modo_lista: ModoLista::DepoisDesta, url: None, volume_db: 0.0, ativo: true, pula_feriado: false, outras_datas: vec![] }
    }

    #[test]
    fn pula_feriado_fixo_e_outras_datas() {
        let natal = NaiveDate::from_ymd_opt(2026, 12, 25).unwrap(); // sexta
        let dia_comum = NaiveDate::from_ymd_opt(2026, 12, 18).unwrap(); // sexta
        let outra = NaiveDate::from_ymd_opt(2026, 12, 21).unwrap(); // segunda
        let mut voz = ag(1, "19:00", [true, true, true, true, true, false, false], TipoAgenda::Link);
        // Sem a chave: toca no feriado.
        assert_eq!(do_minuto(&[voz.clone()], natal, 19, 0).len(), 1);
        voz.pula_feriado = true;
        voz.outras_datas = vec!["2026-12-21".into()];
        let v = [voz.clone()];
        assert!(do_minuto(&v, natal, 19, 0).is_empty());
        assert!(do_minuto(&v, outra, 19, 0).is_empty());
        assert_eq!(do_minuto(&v, dia_comum, 19, 0).len(), 1);
    }

    #[test]
    fn validar_arruma_outras_datas() {
        let mut l = ag(0, "19:00", [true; 7], TipoAgenda::Link);
        l.url = Some("https://x".into());
        l.outras_datas = vec!["2099-05-02".into(), "".into(), "2099-01-10".into(), "2099-05-02".into(), "2000-01-01".into(), "lixo".into()];
        assert!(l.validar().is_ok());
        assert_eq!(l.outras_datas, ["2099-01-10", "2099-05-02"]);
    }

    #[test]
    fn no_mesmo_minuto_lista_e_link_disparam_antes_do_comercial() {
        let seg = NaiveDate::from_ymd_opt(2026, 11, 16).unwrap();
        let tipos = |v: &[Agendamento], h| do_minuto(v, seg, h, 0).iter().map(|a| a.tipo).collect::<Vec<_>>();
        // Comercial criado antes e com horário mais cedo no dia: mesmo assim o link vai primeiro.
        let v = vec![ag(1, "08:00, 19:00", [true; 7], TipoAgenda::Comercial), ag(2, "19:00", [true; 7], TipoAgenda::Link)];
        assert_eq!(tipos(&v, 19), [TipoAgenda::Link, TipoAgenda::Comercial]);
        // A lista vai antes da hora certa e do comercial.
        let mut hora_certa = ag(4, "", [true; 7], TipoAgenda::HoraCerta);
        hora_certa.intervalo = Some(60);
        let v = vec![
            hora_certa,
            ag(1, "08:00, 20:00", [true; 7], TipoAgenda::Comercial),
            ag(2, "20:00", [true; 7], TipoAgenda::Ducking),
            ag(3, "20:00", [true; 7], TipoAgenda::Lista),
        ];
        assert_eq!(tipos(&v, 20), [TipoAgenda::Lista, TipoAgenda::HoraCerta, TipoAgenda::Comercial, TipoAgenda::Ducking]);
    }

    #[test]
    fn varios_horarios_disparam_em_cada_um() {
        let seg = NaiveDate::from_ymd_opt(2026, 11, 16).unwrap();
        let v = vec![ag(1, "18:00, 08:00, 12:30", [true; 7], TipoAgenda::Comercial)];
        assert_eq!(do_minuto(&v, seg, 8, 0).len(), 1);
        assert_eq!(do_minuto(&v, seg, 12, 30).len(), 1);
        assert_eq!(do_minuto(&v, seg, 18, 0).len(), 1);
        assert!(do_minuto(&v, seg, 9, 0).is_empty());
    }

    #[test]
    fn validar_arruma_os_horarios() {
        let mut c = ag(0, "18:00, 8:05, 08:05", [true; 7], TipoAgenda::Comercial);
        c.pastas.push(PastaAnexada { caminho: "/x".into(), modo: Modo::Todos });
        assert!(c.validar().is_ok());
        assert_eq!(c.horarios, ["08:05", "18:00"]);
        c.horarios.clear();
        assert!(c.validar().is_err());
        c.horarios = vec!["25:00".into()];
        assert!(c.validar().is_err());
    }

    /// Desativado: não toca e não ocupa o horário; ativar de novo confere conflito.
    #[test]
    fn desativado_nao_toca_nem_ocupa_horario() {
        let hoje = chrono::NaiveDate::from_ymd_opt(2026, 11, 16).unwrap(); // segunda
        let mut a = ag(1, "08:00", [true; 7], TipoAgenda::Ducking);
        a.ativo = false;
        let v = vec![a.clone()];
        assert!(do_minuto(&v, hoje, 8, 0).is_empty());
        let b = ag(2, "08:00", [true; 7], TipoAgenda::Ducking);
        assert!(conflito(&v, &b).is_none());
        let v = vec![a.clone(), b];
        a.ativo = true;
        assert!(conflito(&v, &a).is_some());
    }

    #[test]
    fn link_precisa_de_endereco_e_nao_de_pasta() {
        let mut l = ag(0, "19:00", [true; 7], TipoAgenda::Link);
        assert!(l.validar().is_err());
        l.url = Some("voz do brasil".into());
        assert!(l.validar().is_err());
        l.url = Some("  https://exemplo.com/radio.aac ".into());
        l.comercial_hora = Some("/x".into());
        assert!(l.validar().is_ok());
        assert_eq!(l.url.as_deref(), Some("https://exemplo.com/radio.aac"));
        assert_eq!(l.comercial_hora, None);
        // Outros tipos perdem o endereço.
        let mut c = ag(0, "10:00", [true; 7], TipoAgenda::Comercial);
        c.pastas.push(PastaAnexada { caminho: "/x".into(), modo: Modo::Todos });
        c.url = Some("https://x".into());
        assert!(c.validar().is_ok());
        assert_eq!(c.url, None);
    }

    #[test]
    fn hora_certa_a_cada_intervalo() {
        let mut h = ag(1, "00:00", [true; 7], TipoAgenda::HoraCerta);
        h.intervalo = Some(15);
        let m = h.minutos_do_dia();
        assert_eq!(m.len(), 96);
        assert_eq!(&m[..5], [0, 15, 30, 45, 60]);
        let data = NaiveDate::from_ymd_opt(2026, 11, 16).unwrap();
        assert_eq!(do_minuto(&[h.clone()], data, 10, 45).len(), 1);
        assert_eq!(do_minuto(&[h], data, 10, 46).len(), 0);
    }

    #[test]
    fn hora_certa_com_arquivos_de_hora_e_minuto_separados() {
        let dir = std::env::temp_dir().join(format!("radio-hc-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        for n in ["HRS00", "HRS08", "HRS23", "MIN01", "MIN08", "MIN10", "MIN59"] {
            fs::write(dir.join(format!("{n}.mp3")), b"").unwrap();
        }
        let nomes = |h: u32, m: u32| -> Vec<String> {
            arquivos_hora_certa(&dir, NaiveTime::from_hms_opt(h, m, 0).unwrap())
                .iter()
                .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
                .collect()
        };
        assert_eq!(nomes(8, 10), ["HRS08.mp3", "MIN10.mp3"]);
        assert_eq!(nomes(8, 0), ["HRS08.mp3"]);
        assert_eq!(nomes(0, 8), ["HRS00.mp3", "MIN08.mp3"]);
        assert_eq!(nomes(23, 59), ["HRS23.mp3", "MIN59.mp3"]);
        // MIN08 não é confundido com 08:00.
        assert_eq!(nomes(8, 0), ["HRS08.mp3"]);
        // Sem o arquivo da hora, não toca o minuto sozinho.
        assert!(nomes(9, 10).is_empty());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn bloqueia_mesmo_horario_no_mesmo_dia() {
        const SEG_SEX: [bool; 7] = [true, true, true, true, true, false, false];
        const SAB: [bool; 7] = [false, false, false, false, false, true, false];
        const SEG: [bool; 7] = [true, false, false, false, false, false, false];
        // Comercial entra em fila: cai junto com qualquer um.
        let v = vec![ag(1, "10:00", SEG_SEX, TipoAgenda::Comercial)];
        assert!(conflito(&v, &ag(0, "10:00", SEG, TipoAgenda::Comercial)).is_none());
        assert!(conflito(&v, &ag(0, "10:00", SEG, TipoAgenda::Ducking)).is_none());
        assert!(conflito(&v, &ag(0, "10:00", SEG, TipoAgenda::Lista)).is_none());
        // Ducking com ducking: não pode, mesmo que seja só um dos horários.
        let v = vec![ag(1, "08:00, 10:00", SEG_SEX, TipoAgenda::Ducking)];
        let msg = conflito(&v, &ag(0, "09:00, 10:00", SEG, TipoAgenda::Ducking)).unwrap();
        assert!(msg.contains("10:00") && msg.contains("Seg"), "{msg}");
        // Outro dia ou outro horário: pode.
        assert!(conflito(&v, &ag(0, "10:00", SAB, TipoAgenda::Ducking)).is_none());
        assert!(conflito(&v, &ag(0, "10:01", SEG, TipoAgenda::Ducking)).is_none());
        // Editar o próprio não conflita consigo mesmo.
        assert!(conflito(&v, &ag(1, "10:00", SEG_SEX, TipoAgenda::Ducking)).is_none());
        // Lista e link não caem juntos (nem lista com lista).
        let v = vec![ag(1, "19:00", SEG_SEX, TipoAgenda::Lista)];
        assert!(conflito(&v, &ag(0, "19:00", SEG, TipoAgenda::Lista)).is_some());
        assert!(conflito(&v, &ag(0, "19:00", SEG, TipoAgenda::Link)).is_some());
        assert!(conflito(&v, &ag(0, "19:00", SEG, TipoAgenda::Ducking)).is_none());
        // Hora certa cai junto com qualquer outro, mas só pode existir uma.
        assert!(conflito(&v, &ag(0, "10:00", SEG, TipoAgenda::HoraCerta)).is_none());
        let v = vec![ag(1, "00:00", SEG_SEX, TipoAgenda::HoraCerta)];
        assert!(conflito(&v, &ag(0, "00:00", SAB, TipoAgenda::HoraCerta)).is_some());
        assert!(conflito(&v, &ag(1, "00:00", SAB, TipoAgenda::HoraCerta)).is_none());
        assert!(conflito(&v, &ag(0, "00:00", SEG, TipoAgenda::Comercial)).is_none());
    }

    #[test]
    fn rodizio_continua_se_o_ultimo_saiu_da_pasta() {
        let mut r = Rodizio::default();
        r.0.insert("p".into(), "b.mp3".into());
        let arq: Vec<PathBuf> = ["a.mp3", "c.mp3"].iter().map(PathBuf::from).collect();
        assert_eq!(r.proximo("p", &arq).unwrap(), PathBuf::from("c.mp3"));
    }
}
