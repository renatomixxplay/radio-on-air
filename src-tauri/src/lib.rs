mod agenda;
mod analise;
mod biblioteca;
mod config;
mod motor;
mod ordem;

use agenda::{Agendamento, Rodizio, TipoAgenda};
use biblioteca::{Biblioteca, FaixaDto};
use config::Config;
use motor::{EventoMotor, Motor};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, State};

/// A fila cabe até 10 itens.
const FILA_MAXIMO: usize = 10;
/// O automático garante este mínimo: quando a fila chega a 3, puxa mais uma.
const FILA_MINIMO: usize = 3;
/// Quantos segundos antes do fim a próxima faixa é entregue ao motor.
const ANTECEDENCIA: f64 = 5.0;
/// Volume da música com o botão de ducking ligado (0.2 = 20%).
const DUCKING_NIVEL: f64 = 0.2;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Tipo {
    Musica,
    Programa,
    Vinheta,
    Comercial,
    HoraCerta,
}

#[derive(Serialize, Clone, Debug)]
pub struct Item {
    id: u64,
    caminho: String,
    artista: String,
    titulo: String,
    tipo: Tipo,
    duracao: Option<f64>,
    /// Já foi entregue ao motor (não pode mais ser movido/removido).
    travado: bool,
}

struct Tocando {
    item: Item,
    inicio: Instant,
    pausa_desde: Option<Instant>,
    pausado_total: Duration,
}

impl Tocando {
    fn decorrido(&self) -> f64 {
        let fim = self.pausa_desde.unwrap_or_else(Instant::now);
        fim.duration_since(self.inicio).saturating_sub(self.pausado_total).as_secs_f64()
    }
}

/// Arquivo tocando na pré-escuta (saída separada, não vai ao ar).
struct Previa {
    caminho: String,
    titulo: String,
    artista: String,
    duracao: Option<f64>,
    /// O motor já avisou que começou.
    iniciada: bool,
    inicio: Instant,
    base: f64,
}

impl Previa {
    fn decorrido(&self) -> f64 {
        self.base + self.inicio.elapsed().as_secs_f64()
    }
}

pub struct Estado {
    dir: PathBuf,
    config: Config,
    bib: Biblioteca,
    carregando: bool,
    fila: Vec<Item>,
    tocando: Option<Tocando>,
    no_ar: bool,
    pausado: bool,
    /// A saída do motor foi desligada pelo botão Parar.
    saida_parada: bool,
    /// Botão repetir: a música atual toca de novo no fim.
    repetir: bool,
    /// A repetição já foi entregue ao motor.
    repeticao_enviada: bool,
    /// Botão de ducking: música abaixada para o locutor falar.
    ducking: bool,
    previa: Option<Previa>,
    agenda: Vec<Agendamento>,
    rodizio: Rodizio,
    /// Pasta da lista de músicas que vale agora (a fila puxa dela).
    lista_ativa: Option<String>,
    /// Comercial com ducking tocando por cima agora.
    por_cima: Option<String>,
    /// Comerciais e hora certa esperando a música acabar. Não aparecem na fila
    /// (agendamento não se exclui, se executa) e tocam antes dela.
    pendentes: Vec<Item>,
    /// Repetição ligada sozinha porque o ducking não cabia no fim da música.
    repetir_auto: bool,
    prox_id: u64,
    motor: Option<Motor>,
    /// O motor está sendo ligado em segundo plano.
    iniciando_motor: bool,
    erro_motor: Option<String>,
    vigia: Option<RecommendedWatcher>,
}

type Compartilhado = Arc<Mutex<Estado>>;

fn travar(e: &Compartilhado) -> MutexGuard<'_, Estado> {
    e.lock().unwrap_or_else(|p| p.into_inner())
}

// ---------- o que a tela recebe ----------

#[derive(Serialize, Clone)]
struct TocandoDto {
    item: Item,
    decorrido: f64,
}

#[derive(Serialize, Clone)]
struct CartuchoDto {
    nome: String,
    caminho: String,
}

#[derive(Serialize, Clone)]
struct PreviaDto {
    caminho: String,
    titulo: String,
    artista: String,
    duracao: Option<f64>,
    decorrido: f64,
    iniciada: bool,
}

#[derive(Serialize, Clone)]
struct Retrato {
    no_ar: bool,
    pausado: bool,
    tocando: Option<TocandoDto>,
    fila: Vec<Item>,
    total_musicas: usize,
    carregando: bool,
    pasta_musicas: Option<String>,
    pasta_programas: Option<String>,
    cartucheira: Vec<Option<CartuchoDto>>,
    motor_ok: bool,
    iniciando_motor: bool,
    erro_motor: Option<String>,
    repetir: bool,
    ducking: bool,
    previa: Option<PreviaDto>,
    lista_ativa: Option<String>,
    por_cima: Option<String>,
    /// Comercial/hora certa que entra quando a música acabar.
    aguardando: Option<String>,
    repetir_auto: bool,
    proximo_agendamento: Option<agenda::ProximoDto>,
}

impl Estado {
    fn retrato(&self) -> Retrato {
        let caminho = |p: &Option<PathBuf>| p.as_ref().map(|p| p.to_string_lossy().into_owned());
        Retrato {
            no_ar: self.no_ar,
            pausado: self.pausado,
            tocando: self.tocando.as_ref().map(|t| TocandoDto { item: t.item.clone(), decorrido: t.decorrido() }),
            fila: self.fila.clone(),
            total_musicas: self.bib.total(),
            carregando: self.carregando,
            pasta_musicas: caminho(&self.config.pasta_musicas),
            pasta_programas: caminho(&self.config.pasta_programas),
            cartucheira: self
                .config
                .cartucheira
                .iter()
                .map(|c| {
                    c.as_ref().map(|p| CartuchoDto {
                        nome: p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(),
                        caminho: p.to_string_lossy().into_owned(),
                    })
                })
                .collect(),
            motor_ok: self.motor.is_some(),
            iniciando_motor: self.iniciando_motor,
            erro_motor: self.erro_motor.clone(),
            repetir: self.repetir,
            ducking: self.ducking,
            lista_ativa: self.lista_ativa.as_ref().map(|p| {
                Path::new(p).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
            }),
            por_cima: self.por_cima.clone(),
            aguardando: self.pendentes.first().map(|p| {
                let pasta = if p.artista.is_empty() { p.titulo.clone() } else { p.artista.clone() };
                if self.pendentes.len() > 1 { format!("{pasta} ({})", self.pendentes.len()) } else { pasta }
            }),
            repetir_auto: self.repetir_auto,
            proximo_agendamento: agenda::proximo(&self.agenda),
            previa: self.previa.as_ref().map(|p| PreviaDto {
                caminho: p.caminho.clone(),
                titulo: p.titulo.clone(),
                artista: p.artista.clone(),
                duracao: p.duracao,
                decorrido: if p.iniciada { p.decorrido() } else { 0.0 },
                iniciada: p.iniciada,
            }),
        }
    }

    fn arquivo_indice(&self) -> PathBuf {
        self.dir.join("indice.txt")
    }

    fn novo_item(&mut self, caminho: String, artista: String, titulo: String, tipo: Tipo) -> Item {
        self.prox_id += 1;
        Item { id: self.prox_id, caminho, artista, titulo, tipo, duracao: None, travado: false }
    }

    /// Entrega o primeiro da fila ao motor, se ainda não foi.
    /// Já tem algo entregue ao motor esperando para tocar.
    fn tem_entregue(&self) -> bool {
        self.pendentes.iter().chain(self.fila.iter()).any(|i| i.travado)
    }

    /// Entrega ao motor o próximo: primeiro os agendados pendentes, depois a fila.
    fn enviar_proximo(&mut self) {
        if self.tem_entregue() {
            return;
        }
        let Some(motor) = &self.motor else { return };
        let prox = match self.pendentes.first_mut() {
            Some(p) => Some(p),
            None => self.fila.first_mut(),
        };
        if let Some(prox) = prox {
            match motor.enfileirar(&prox.caminho) {
                Ok(()) => prox.travado = true,
                Err(e) => self.erro_motor = Some(e),
            }
        }
    }

    fn destravar_tudo(&mut self) {
        self.pendentes.iter_mut().chain(self.fila.iter_mut()).for_each(|i| i.travado = false);
    }
}

fn emitir(app: &AppHandle, e: &Estado) {
    let _ = app.emit("estado", e.retrato());
}

/// Lê a duração em segundo plano e atualiza o item.
fn analisar_em_segundo_plano(app: &AppHandle, estado: &Compartilhado, id: u64, caminho: String) {
    let app = app.clone();
    let estado = estado.clone();
    std::thread::spawn(move || {
        let duracao = analise::duracao(Path::new(&caminho));
        let mut e = travar(&estado);
        let Estado { fila, tocando, pendentes, .. } = &mut *e;
        let alvo = fila
            .iter_mut()
            .chain(pendentes.iter_mut())
            .find(|i| i.id == id)
            .or_else(|| tocando.as_mut().map(|t| &mut t.item).filter(|i| i.id == id));
        if let Some(item) = alvo {
            item.duracao = duracao;
        }
        emitir(&app, &e);
    });
}

/// Mantém o mínimo da fila puxando músicas aleatórias do acervo.
fn completar_fila(app: &AppHandle, estado: &Compartilhado, e: &mut Estado) {
    while e.fila.len() <= FILA_MINIMO && e.fila.len() < FILA_MAXIMO {
        let da_lista = e.lista_ativa.as_ref().and_then(|p| e.bib.aleatoria_em(Path::new(p)));
        let Some(f) = da_lista.or_else(|| e.bib.aleatoria()) else { break };
        let item = e.novo_item(f.caminho, f.artista, f.musica, Tipo::Musica);
        analisar_em_segundo_plano(app, estado, item.id, item.caminho.clone());
        e.fila.push(item);
    }
}

fn ao_evento_motor(app: &AppHandle, estado: &Compartilhado, ev: EventoMotor) {
    let mut e = travar(estado);
    match ev {
        EventoMotor::Faixa(caminho) if caminho.is_empty() => {
            // O motor ficou sem nada: entrega a próxima na hora.
            e.tocando = None;
            if e.no_ar && !e.pausado {
                completar_fila(app, estado, &mut e);
                e.enviar_proximo();
            }
        }
        EventoMotor::Faixa(caminho) => {
            let repeticao = e.repeticao_enviada && e.tocando.as_ref().is_some_and(|t| t.item.caminho == caminho);
            e.repeticao_enviada = false;
            let pend = e.pendentes.iter().position(|i| i.travado && i.caminho == caminho);
            let pos = e.fila.iter().position(|i| i.travado && i.caminho == caminho);
            let item = match (repeticao, pend, pos) {
                (true, _, _) => e.tocando.as_ref().map(|t| t.item.clone()).expect("tocando"),
                (false, Some(p), _) => e.pendentes.remove(p),
                (false, None, Some(p)) => e.fila.remove(p),
                (false, None, None) => {
                    let (artista, titulo) = biblioteca::artista_e_musica(Path::new(&caminho));
                    let item = e.novo_item(caminho.clone(), artista, titulo, Tipo::Musica);
                    analisar_em_segundo_plano(app, estado, item.id, caminho);
                    item
                }
            };
            e.tocando = Some(Tocando { item, inicio: Instant::now(), pausa_desde: None, pausado_total: Duration::ZERO });
            completar_fila(app, estado, &mut e);
        }
        EventoMotor::Fala(caminho) => {
            e.por_cima = (!caminho.is_empty()).then(|| {
                let p = Path::new(&caminho);
                agenda::nome_sem_validade(&p.file_stem().map(|s| s.to_string_lossy()).unwrap_or_default())
            });
            if caminho.is_empty() && e.repetir_auto {
                e.repetir_auto = false;
                fade_e_proxima(app.clone(), estado.clone(), if e.ducking { DUCKING_NIVEL } else { 1.0 });
            }
        }
        EventoMotor::Previa(caminho) => {
            if caminho.is_empty() {
                if e.previa.as_ref().is_some_and(|p| p.iniciada) {
                    e.previa = None;
                }
            } else if let Some(p) = e.previa.as_mut().filter(|p| p.caminho == caminho) {
                p.iniciada = true;
                p.inicio = Instant::now();
                p.base = 0.0;
            }
        }
        EventoMotor::Parou(_) if e.motor.is_none() => {
            // Falhou ao iniciar: a mensagem já foi dada por quem tentou ligar.
        }
        EventoMotor::Parou(erro) => {
            e.motor = None;
            e.previa = None;
            e.por_cima = None;
            e.ducking = false;
            e.no_ar = false;
            e.tocando = None;
            e.destravar_tudo();
            let erro = erro.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").to_string();
            e.erro_motor = Some(if erro.is_empty() { "O motor de áudio parou.".into() } else { erro });
        }
    }
    emitir(app, &e);
}

/// Liga o motor em segundo plano (sem travar a tela). Quando ficar pronto,
/// se a rádio estiver no ar, já entrega a próxima faixa.
fn iniciar_motor(app: &AppHandle, estado: &Compartilhado, e: &mut Estado) {
    if e.motor.is_some() || e.iniciando_motor {
        return;
    }
    let Some(exe) = motor::localizar() else {
        e.erro_motor = Some("Liquidsoap não encontrado. Instale o pacote liquidsoap.".into());
        return;
    };
    e.iniciando_motor = true;
    e.erro_motor = None;
    emitir(app, e);
    let (app, estado, dir) = (app.clone(), estado.clone(), e.dir.clone());
    std::thread::spawn(move || {
        let (app2, estado2) = (app.clone(), estado.clone());
        let resultado = Motor::iniciar(&exe, &dir, move |ev| ao_evento_motor(&app2, &estado2, ev));
        let mut e = travar(&estado);
        e.iniciando_motor = false;
        match resultado {
            Ok(m) => {
                e.motor = Some(m);
                e.erro_motor = None;
                if e.no_ar && !e.pausado && e.tocando.is_none() {
                    completar_fila(&app, &estado, &mut e);
                    e.enviar_proximo();
                }
            }
            Err(err) => {
                e.no_ar = false;
                e.erro_motor = Some(err);
            }
        }
        emitir(&app, &e);
    });
}

/// Relógio interno: entrega a próxima faixa perto do fim da atual.
fn iniciar_relogio(app: AppHandle, estado: Compartilhado) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(250));
        let mut e = travar(&estado);
        if !e.no_ar || e.pausado {
            continue;
        }
        let falta = e.tocando.as_ref().and_then(|t| t.item.duracao.map(|d| d - t.decorrido()));
        if !falta.is_some_and(|f| f <= ANTECEDENCIA) {
            continue;
        }
        if e.repetir || e.repetir_auto {
            // Repetir: entrega de novo a mesma música, a fila espera.
            if !e.repeticao_enviada {
                let atual = e.tocando.as_ref().map(|t| t.item.caminho.clone());
                if let (Some(m), Some(c)) = (&e.motor, atual) {
                    if m.enfileirar(&c).is_ok() {
                        e.repeticao_enviada = true;
                        emitir(&app, &e);
                    }
                }
            }
        } else if !e.tem_entregue() && (!e.pendentes.is_empty() || !e.fila.is_empty()) {
            e.enviar_proximo();
            emitir(&app, &e);
        }
    });
}

/// Abaixa a música até zero em 3 s, passa para a próxima e volta o volume.
fn fade_e_proxima(app: AppHandle, estado: Compartilhado, volume_normal: f64) {
    std::thread::spawn(move || {
        const PASSOS: u32 = 15;
        for i in 1..=PASSOS {
            std::thread::sleep(Duration::from_millis(200));
            let e = travar(&estado);
            if let Some(m) = &e.motor {
                let _ = m.volume(volume_normal * (1.0 - f64::from(i) / f64::from(PASSOS)));
            }
        }
        let mut e = travar(&estado);
        if let Some(m) = &e.motor {
            let _ = m.esvaziar_e_pular();
            let _ = m.volume(volume_normal);
        }
        e.repeticao_enviada = false;
        e.destravar_tudo();
        emitir(&app, &e);
    });
}

// ---------- agendamentos ----------

/// Coloca arquivos logo depois do que já foi entregue ao motor (entram na sequência).
/// Agendados que esperam a música acabar: vão para os pendentes (fora da fila).
fn agendar_em_sequencia(app: &AppHandle, estado: &Compartilhado, e: &mut Estado, arquivos: Vec<PathBuf>, tipo: Tipo) {
    for arq in arquivos {
        let caminho = arq.to_string_lossy().into_owned();
        let titulo = agenda::nome_sem_validade(&arq.file_stem().map(|s| s.to_string_lossy()).unwrap_or_default());
        let pasta = arq.parent().and_then(|d| d.file_name()).map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let item = e.novo_item(caminho.clone(), pasta, titulo, tipo);
        analisar_em_segundo_plano(app, estado, item.id, caminho);
        e.pendentes.push(item);
    }
}

fn disparar(app: &AppHandle, estado: &Compartilhado, e: &mut Estado, ag: &Agendamento) {
    match ag.tipo {
        TipoAgenda::Lista => {
            if let Some(p) = ag.pastas.first() {
                e.lista_ativa = Some(p.caminho.clone());
            }
        }
        // Comerciais e hora certa só quando a rádio está no ar.
        _ if !e.no_ar => {}
        TipoAgenda::Comercial => {
            let dir = e.dir.clone();
            let bloco = agenda::montar_bloco(ag, &mut e.rodizio, &dir);
            agendar_em_sequencia(app, estado, e, bloco, Tipo::Comercial);
        }
        TipoAgenda::Ducking => {
            let dir = e.dir.clone();
            let arquivos = agenda::montar_bloco(ag, &mut e.rodizio, &dir);
            // Se o bloco não cabe no que falta da música, a música repete por baixo
            // e, quando a fala acabar, sai com fade de 3 s para a próxima.
            let total: f64 = arquivos.iter().filter_map(|p| analise::duracao(p)).sum();
            let falta = e.tocando.as_ref().and_then(|t| t.item.duracao.map(|d| d - t.decorrido()));
            if falta.is_some_and(|f| total > f - 1.0) {
                e.repetir_auto = true;
            }
            let bloco: Vec<String> = arquivos.into_iter().map(|p| p.to_string_lossy().into_owned()).collect();
            if let Some(m) = &e.motor {
                if let Err(err) = m.falar(&bloco) {
                    e.erro_motor = Some(err);
                }
            }
        }
        TipoAgenda::HoraCerta => {
            let agora = chrono::Local::now().time();
            let arquivo = ag.pastas.first().and_then(|p| agenda::arquivo_hora_certa(Path::new(&p.caminho), agora));
            agendar_em_sequencia(app, estado, e, arquivo.into_iter().collect(), Tipo::HoraCerta);
        }
    }
}

/// Confere a agenda a cada segundo e dispara o que vence em cada minuto novo.
fn iniciar_relogio_agenda(app: AppHandle, estado: Compartilhado) {
    use chrono::Timelike;
    std::thread::spawn(move || {
        let minuto = |t: chrono::DateTime<chrono::Local>| (t.date_naive(), t.hour(), t.minute());
        let mut ultimo = minuto(chrono::Local::now());
        loop {
            std::thread::sleep(Duration::from_millis(500));
            let agora = minuto(chrono::Local::now());
            if agora == ultimo {
                continue;
            }
            ultimo = agora;
            let mut e = travar(&estado);
            let vencem = agenda::do_minuto(&e.agenda, agora.0, agora.1, agora.2);
            if vencem.is_empty() {
                continue;
            }
            for ag in &vencem {
                disparar(&app, &estado, &mut e, ag);
            }
            emitir(&app, &e);
        }
    });
}

#[tauri::command]
fn agendamentos(estado: State<Compartilhado>) -> Vec<Agendamento> {
    travar(&estado).agenda.clone()
}

#[tauri::command]
fn agendamento_salvar(app: AppHandle, estado: State<Compartilhado>, mut ag: Agendamento) -> Result<Vec<Agendamento>, String> {
    if ag.hora_minuto().is_none() {
        return Err("Horário inválido.".into());
    }
    if !ag.dias.iter().any(|d| *d) {
        return Err("Marque pelo menos um dia.".into());
    }
    if ag.pastas.is_empty() {
        return Err("Escolha pelo menos uma pasta.".into());
    }
    let mut e = travar(&estado);
    if ag.id == 0 {
        ag.id = e.agenda.iter().map(|a| a.id).max().unwrap_or(0) + 1;
        e.agenda.push(ag);
    } else if let Some(a) = e.agenda.iter_mut().find(|a| a.id == ag.id) {
        *a = ag;
    } else {
        e.agenda.push(ag);
    }
    agenda::ordenar(&mut e.agenda);
    agenda::salvar(&e.dir, &e.agenda).map_err(|err| err.to_string())?;
    e.lista_ativa = agenda::lista_vigente(&e.agenda);
    emitir(&app, &e);
    Ok(e.agenda.clone())
}

#[tauri::command]
fn agendamento_remover(app: AppHandle, estado: State<Compartilhado>, id: u64) -> Result<Vec<Agendamento>, String> {
    let mut e = travar(&estado);
    e.agenda.retain(|a| a.id != id);
    agenda::salvar(&e.dir, &e.agenda).map_err(|err| err.to_string())?;
    e.lista_ativa = agenda::lista_vigente(&e.agenda);
    emitir(&app, &e);
    Ok(e.agenda.clone())
}

// ---------- acervo ----------

fn carregar_biblioteca(app: AppHandle, estado: Compartilhado, forcar_varredura: bool) {
    let (raiz, indice) = {
        let mut e = travar(&estado);
        let Some(raiz) = e.config.pasta_musicas.clone() else { return };
        e.carregando = true;
        e.vigia = None;
        emitir(&app, &e);
        (raiz, e.arquivo_indice())
    };
    std::thread::spawn(move || {
        let bib = (!forcar_varredura)
            .then(|| Biblioteca::carregar_indice(raiz.clone(), &indice))
            .flatten()
            .unwrap_or_else(|| Biblioteca::varrer(raiz.clone()));
        let mut e = travar(&estado);
        e.bib = bib;
        let _ = e.bib.salvar_indice(&indice);
        e.carregando = false;
        e.vigia = vigiar(&app, &estado, &raiz);
        completar_fila(&app, &estado, &mut e);
        emitir(&app, &e);
    });
}

/// Acompanha a pasta de músicas: arquivo novo, apagado ou renomeado entra no índice na hora.
fn vigiar(app: &AppHandle, estado: &Compartilhado, raiz: &Path) -> Option<RecommendedWatcher> {
    let (app, estado) = (app.clone(), estado.clone());
    let mut w = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        let Ok(ev) = res else { return };
        let relevante = matches!(
            ev.kind,
            EventKind::Create(_) | EventKind::Remove(_) | EventKind::Modify(notify::event::ModifyKind::Name(_))
        );
        if !relevante {
            return;
        }
        let mut e = travar(&estado);
        for p in &ev.paths {
            e.bib.atualizar_caminho(p);
        }
        if e.bib.sujo {
            emitir(&app, &e);
        }
    })
    .ok()?;
    w.watch(raiz, RecursiveMode::Recursive).ok()?;
    Some(w)
}

/// Grava o `indice.txt` de vez em quando, se algo mudou.
fn iniciar_gravador_indice(estado: Compartilhado) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(10));
        let mut e = travar(&estado);
        if e.bib.sujo && !e.carregando {
            let arq = e.arquivo_indice();
            let _ = e.bib.salvar_indice(&arq);
        }
    });
}

// ---------- comandos chamados pela tela ----------

#[tauri::command]
fn estado(estado: State<Compartilhado>) -> Retrato {
    travar(&estado).retrato()
}

#[tauri::command]
fn buscar(estado: State<Compartilhado>, termo: String) -> Vec<FaixaDto> {
    travar(&estado).bib.buscar(&termo, 200)
}

#[derive(Serialize)]
struct Pasta {
    nome: String,
    caminho: String,
}

#[derive(Serialize)]
struct Arquivo {
    nome: String,
    caminho: String,
}

#[tauri::command]
fn programas(estado: State<Compartilhado>) -> Vec<Pasta> {
    let Some(raiz) = travar(&estado).config.pasta_programas.clone() else { return vec![] };
    let mut v: Vec<Pasta> = std::fs::read_dir(&raiz)
        .map(|rd| {
            rd.filter_map(Result::ok)
                .filter(|d| d.path().is_dir())
                .map(|d| Pasta {
                    nome: d.file_name().to_string_lossy().into_owned(),
                    caminho: d.path().to_string_lossy().into_owned(),
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort_by(|a, b| ordem::natural(&a.nome, &b.nome));
    v
}

#[tauri::command]
fn arquivos_da_pasta(caminho: String) -> Vec<Arquivo> {
    let mut v: Vec<Arquivo> = std::fs::read_dir(&caminho)
        .map(|rd| {
            rd.filter_map(Result::ok)
                .map(|d| d.path())
                .filter(|p| p.is_file() && biblioteca::eh_audio(p))
                .map(|p| Arquivo {
                    nome: p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(),
                    caminho: p.to_string_lossy().into_owned(),
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort_by(|a, b| ordem::natural(&a.nome, &b.nome));
    v
}

#[tauri::command]
fn fila_adicionar(
    app: AppHandle,
    estado: State<Compartilhado>,
    caminho: String,
    tipo: Tipo,
    posicao: Option<usize>,
) -> Result<(), String> {
    let compartilhado = estado.inner().clone();
    let mut e = travar(&compartilhado);
    if e.fila.len() >= FILA_MAXIMO {
        return Err(format!("A fila já tem {FILA_MAXIMO} itens."));
    }
    let (artista, titulo) = match tipo {
        Tipo::Musica => biblioteca::artista_e_musica(Path::new(&caminho)),
        _ => {
            let p = Path::new(&caminho);
            let pasta = p.parent().and_then(|d| d.file_name()).map(|s| s.to_string_lossy().into_owned());
            (pasta.unwrap_or_default(), p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default())
        }
    };
    let item = e.novo_item(caminho.clone(), artista, titulo, tipo);
    // Não deixa entrar na frente de quem já foi entregue ao motor.
    let travados = e.fila.iter().take_while(|i| i.travado).count();
    let pos = posicao.unwrap_or(e.fila.len()).clamp(travados, e.fila.len());
    analisar_em_segundo_plano(&app, &compartilhado, item.id, caminho);
    e.fila.insert(pos, item);
    emitir(&app, &e);
    Ok(())
}

#[tauri::command]
fn fila_remover(app: AppHandle, estado: State<Compartilhado>, id: u64) {
    let compartilhado = estado.inner().clone();
    let mut e = travar(&compartilhado);
    e.fila.retain(|i| i.id != id || i.travado);
    completar_fila(&app, &compartilhado, &mut e);
    emitir(&app, &e);
}

#[tauri::command]
fn fila_mover(app: AppHandle, estado: State<Compartilhado>, id: u64, posicao: usize) {
    let mut e = travar(&estado);
    let Some(de) = e.fila.iter().position(|i| i.id == id && !i.travado) else { return };
    let item = e.fila.remove(de);
    let travados = e.fila.iter().take_while(|i| i.travado).count();
    let pos = posicao.clamp(travados, e.fila.len());
    e.fila.insert(pos, item);
    emitir(&app, &e);
}

#[tauri::command]
fn tocar(app: AppHandle, estado: State<Compartilhado>) {
    let compartilhado = estado.inner().clone();
    let mut e = travar(&compartilhado);
    iniciar_motor(&app, &compartilhado, &mut e);
    if e.pausado || e.saida_parada {
        if let Some(m) = &e.motor {
            let _ = m.retomar();
        }
        e.pausado = false;
        e.saida_parada = false;
        if let Some(t) = e.tocando.as_mut() {
            if let Some(desde) = t.pausa_desde.take() {
                t.pausado_total += desde.elapsed();
            }
        }
    }
    if !e.no_ar {
        e.no_ar = true;
        completar_fila(&app, &compartilhado, &mut e);
        if e.tocando.is_none() {
            e.enviar_proximo();
        }
    }
    emitir(&app, &e);
}

#[tauri::command]
fn pausar(app: AppHandle, estado: State<Compartilhado>) {
    let mut e = travar(&estado);
    if !e.no_ar || e.pausado {
        return;
    }
    if let Some(m) = &e.motor {
        let _ = m.pausar();
    }
    e.pausado = true;
    if let Some(t) = e.tocando.as_mut() {
        t.pausa_desde = Some(Instant::now());
    }
    emitir(&app, &e);
}

#[tauri::command]
fn parar(app: AppHandle, estado: State<Compartilhado>) {
    let mut e = travar(&estado);
    e.no_ar = false;
    e.pausado = false;
    if let Some(m) = &e.motor {
        let _ = m.pausar();
        let _ = m.pular();
        e.saida_parada = true;
    }
    e.tocando = None;
    emitir(&app, &e);
}

#[tauri::command]
fn proxima(app: AppHandle, estado: State<Compartilhado>) {
    let compartilhado = estado.inner().clone();
    let mut e = travar(&compartilhado);
    if !e.no_ar {
        return;
    }
    completar_fila(&app, &compartilhado, &mut e);
    // Tira do motor o que já tinha sido entregue (inclusive uma repetição);
    // o motor fica sem nada e o evento de silêncio entrega a próxima da fila.
    if let Some(m) = &e.motor {
        let _ = m.esvaziar_e_pular();
    }
    e.repeticao_enviada = false;
    e.destravar_tudo();
    emitir(&app, &e);
}

#[tauri::command]
fn alternar_repetir(app: AppHandle, estado: State<Compartilhado>) {
    let mut e = travar(&estado);
    e.repetir = !e.repetir;
    emitir(&app, &e);
}

/// Liga/desliga o ducking, descendo/subindo o volume suavemente (~0,6 s).
#[tauri::command]
fn alternar_ducking(app: AppHandle, estado: State<Compartilhado>) {
    let compartilhado = estado.inner().clone();
    let (de, para) = {
        let mut e = travar(&compartilhado);
        iniciar_motor(&app, &compartilhado, &mut e);
        e.ducking = !e.ducking;
        emitir(&app, &e);
        if e.ducking { (1.0, DUCKING_NIVEL) } else { (DUCKING_NIVEL, 1.0) }
    };
    std::thread::spawn(move || {
        const PASSOS: u32 = 8;
        for i in 1..=PASSOS {
            let v = de + (para - de) * f64::from(i) / f64::from(PASSOS);
            {
                let e = travar(&compartilhado);
                if let Some(m) = &e.motor {
                    let _ = m.volume(v);
                }
            }
            std::thread::sleep(Duration::from_millis(75));
        }
    });
}

#[tauri::command]
fn previa_tocar(app: AppHandle, estado: State<Compartilhado>, caminho: String) {
    let compartilhado = estado.inner().clone();
    let mut e = travar(&compartilhado);
    iniciar_motor(&app, &compartilhado, &mut e);
    let Some(m) = &e.motor else {
        emitir(&app, &e);
        return;
    };
    if m.previa_tocar(&caminho).is_err() {
        return;
    }
    let (artista, titulo) = biblioteca::artista_e_musica(Path::new(&caminho));
    e.previa = Some(Previa {
        caminho: caminho.clone(),
        titulo,
        artista,
        duracao: None,
        iniciada: false,
        inicio: Instant::now(),
        base: 0.0,
    });
    emitir(&app, &e);
    drop(e);
    // Duração em segundo plano.
    std::thread::spawn(move || {
        let d = analise::duracao(Path::new(&caminho));
        let mut e = travar(&compartilhado);
        if let Some(p) = e.previa.as_mut().filter(|p| p.caminho == caminho) {
            p.duracao = d;
        }
        emitir(&app, &e);
    });
}

#[tauri::command]
fn previa_parar(app: AppHandle, estado: State<Compartilhado>) {
    let mut e = travar(&estado);
    if let Some(m) = &e.motor {
        let _ = m.previa_parar();
    }
    e.previa = None;
    emitir(&app, &e);
}

/// Pula a pré-escuta para `segundos` a partir do início.
#[tauri::command]
fn previa_ir(app: AppHandle, estado: State<Compartilhado>, segundos: f64) {
    let mut e = travar(&estado);
    let Estado { motor, previa, .. } = &mut *e;
    if let (Some(m), Some(p)) = (motor.as_ref(), previa.as_mut().filter(|p| p.iniciada)) {
        let alvo = segundos.clamp(0.0, p.duracao.unwrap_or(f64::MAX));
        if m.previa_andar(alvo - p.decorrido()).is_ok() {
            p.base = alvo;
            p.inicio = Instant::now();
        }
    }
    emitir(&app, &e);
}

#[tauri::command]
fn cartucheira_tocar(app: AppHandle, estado: State<Compartilhado>, posicao: usize) {
    let compartilhado = estado.inner().clone();
    let mut e = travar(&compartilhado);
    iniciar_motor(&app, &compartilhado, &mut e);
    let caminho = e.config.cartucheira.get(posicao).cloned().flatten();
    if let (Some(m), Some(c)) = (&e.motor, caminho) {
        let _ = m.cartucho(&c.to_string_lossy());
    }
    emitir(&app, &e);
}

#[tauri::command]
fn cartucheira_definir(
    app: AppHandle,
    estado: State<Compartilhado>,
    posicao: usize,
    caminho: Option<String>,
) -> Result<(), String> {
    let mut e = travar(&estado);
    let slot = e.config.cartucheira.get_mut(posicao).ok_or("posição inválida")?;
    *slot = caminho.map(PathBuf::from);
    e.config.salvar(&e.dir).map_err(|err| err.to_string())?;
    emitir(&app, &e);
    Ok(())
}

#[tauri::command]
fn definir_pasta(app: AppHandle, estado: State<Compartilhado>, qual: String, caminho: String) -> Result<(), String> {
    let compartilhado = estado.inner().clone();
    {
        let mut e = travar(&compartilhado);
        let p = Some(PathBuf::from(&caminho));
        match qual.as_str() {
            "musicas" => e.config.pasta_musicas = p,
            "programas" => e.config.pasta_programas = p,
            _ => return Err("pasta desconhecida".into()),
        }
        e.config.salvar(&e.dir).map_err(|err| err.to_string())?;
        emitir(&app, &e);
    }
    if qual == "musicas" {
        carregar_biblioteca(app, compartilhado, true);
    }
    Ok(())
}

#[tauri::command]
fn atualizar_musicas(app: AppHandle, estado: State<Compartilhado>) {
    carregar_biblioteca(app, estado.inner().clone(), true);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let dir = app.path().app_data_dir().expect("pasta de dados");
            let config = Config::carregar(&dir);
            let agenda = agenda::carregar(&dir);
            let rodizio = Rodizio::carregar(&dir);
            let lista_ativa = agenda::lista_vigente(&agenda);
            let estado: Compartilhado = Arc::new(Mutex::new(Estado {
                dir,
                config,
                bib: Biblioteca::default(),
                carregando: false,
                fila: vec![],
                tocando: None,
                no_ar: false,
                pausado: false,
                saida_parada: false,
                repetir: false,
                repeticao_enviada: false,
                ducking: false,
                previa: None,
                agenda,
                rodizio,
                lista_ativa,
                por_cima: None,
                pendentes: vec![],
                repetir_auto: false,
                prox_id: 0,
                motor: None,
                iniciando_motor: false,
                erro_motor: None,
                vigia: None,
            }));
            app.manage(estado.clone());
            let handle = app.handle().clone();
            iniciar_motor(&handle, &estado, &mut travar(&estado));
            carregar_biblioteca(handle.clone(), estado.clone(), false);
            iniciar_relogio(handle.clone(), estado.clone());
            iniciar_relogio_agenda(handle, estado.clone());
            iniciar_gravador_indice(estado);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            estado,
            buscar,
            programas,
            arquivos_da_pasta,
            fila_adicionar,
            fila_remover,
            fila_mover,
            tocar,
            pausar,
            parar,
            proxima,
            cartucheira_tocar,
            cartucheira_definir,
            definir_pasta,
            atualizar_musicas,
            alternar_repetir,
            alternar_ducking,
            previa_tocar,
            previa_parar,
            previa_ir,
            agendamentos,
            agendamento_salvar,
            agendamento_remover,
        ])
        .run(tauri::generate_context!())
        .expect("erro ao abrir o Radio On Air");
}
