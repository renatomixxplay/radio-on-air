mod agenda;
mod analise;
mod biblioteca;
mod captura;
mod config;
mod motor;
mod ordem;
mod placas;
mod registro;
mod sistema;

use agenda::{Agendamento, ModoLista, Rodizio, TipoAgenda};
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
/// Nada é analisado (começa na hora): basta chegar antes do crossfade de 2 s.
const ANTECEDENCIA: f64 = 5.0;
/// Ducking que esperou: entra este tempo depois que a próxima música começa.
const DUCKING_DEPOIS: f64 = 10.0;
/// Ducking esperando há mais que isso (rádio parada, link no ar) é descartado.
const DUCKING_VALIDADE: Duration = Duration::from_secs(600);
/// Com menos que isso para acabar, o botão Repetir não liga (a próxima já
/// pode ter ido para o motor).
const REPETIR_MINIMO: f64 = 10.0;
/// Ao abrir, espera no máximo isso pela procura de músicas novas; depois a
/// rádio começa com o que já tem e a procura termina em segundo plano.
const LIMITE_PROCURA: Duration = Duration::from_secs(15);
/// Fim da fala com a música repetindo: ela volta ao normal e toca mais este
/// tempo antes do fade para a próxima.
const DEPOIS_DO_DUCKING: Duration = Duration::from_secs(10);
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
    /// Ponto da música (s) em que começou a tocar: 0 ou onde a pessoa clicou.
    base: f64,
}

impl Previa {
    fn decorrido(&self) -> f64 {
        self.base + self.inicio.elapsed().as_secs_f64()
    }
}

struct DuckingAdiado {
    arquivos: Vec<PathBuf>,
    /// O que tocava quando chegou: o ducking entra na música seguinte.
    depois_de: Option<u64>,
    desde: Instant,
}

pub struct Estado {
    dir: PathBuf,
    config: Config,
    bib: Biblioteca,
    carregando: bool,
    /// Ao abrir: relendo a pasta de músicas atrás de mudanças. A rádio só
    /// começa a tocar quando termina (ou passa `LIMITE_PROCURA`).
    procurando_musicas: bool,
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
    /// Ducking que chegou com a próxima já entregue (ou sem música tocando):
    /// espera a próxima música e entra `DUCKING_DEPOIS` segundos depois.
    ducking_adiado: Option<DuckingAdiado>,
    /// Conta os fins de ducking: a troca marcada só vale se for a última.
    vez_ducking: u64,
    /// A fala acabou e a troca para a próxima está marcada: sem vinheta até lá.
    trocando_pos_ducking: bool,
    prox_id: u64,
    motor: Option<Motor>,
    /// O motor está sendo ligado em segundo plano.
    iniciando_motor: bool,
    erro_motor: Option<String>,
    vigia: Option<RecommendedWatcher>,
    /// Link ao vivo agendado. Enquanto existe, a fila de músicas espera e
    /// comercial/hora certa são descartados.
    link: Option<LinkAtivo>,
    /// Última vinheta sorteada (para não repetir a mesma em seguida).
    ultima_vinheta: Option<PathBuf>,
    /// Muda a cada motor novo: eventos de um motor antigo são ignorados.
    geracao_motor: u64,
    /// Gravação e transmissão da entrada (processo separado).
    captura: Option<captura::Captura>,
    geracao_captura: u64,
    /// "desligada", "conectando", "conectado", "caiu" ou "erro …".
    transmissao_estado: String,
    erro_captura: Option<String>,
}

struct LinkAtivo {
    url: String,
    /// Já está no ar (antes disso, toca a abertura).
    ligado: bool,
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
    /// Segundos até o som chegar à saída (a barra só anda depois disso).
    espera: f64,
}

#[derive(Serialize, Clone)]
struct Retrato {
    no_ar: bool,
    pausado: bool,
    tocando: Option<TocandoDto>,
    fila: Vec<Item>,
    total_musicas: usize,
    carregando: bool,
    procurando_musicas: bool,
    pasta_musicas: Option<String>,
    /// Pastas do painel "Pastas", em ordem alfabética.
    pastas: Vec<Pasta>,
    tocar_ao_abrir: bool,
    abrir_tela_cheia: bool,
    vinheta_entre_musicas: bool,
    /// Tem senha definida.
    protegido: bool,
    bloqueios: config::Bloqueios,
    nome_radio: String,
    audio: config::Audio,
    gravacao: config::Gravacao,
    transmissao: config::Transmissao,
    transmissao_estado: String,
    erro_captura: Option<String>,
    /// A gravação está ligada e a captura rodando sem erro.
    gravando: bool,
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
    /// Fala acabou e a troca para a próxima está marcada (botões travados).
    pos_ducking: bool,
    proximo_agendamento: Option<agenda::ProximoDto>,
    /// Endereço do link ao vivo no ar.
    link: Option<String>,
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
            procurando_musicas: self.procurando_musicas,
            pasta_musicas: caminho(&self.config.pasta_musicas),
            pastas: {
                let mut v: Vec<Pasta> = self
                    .config
                    .pastas
                    .iter()
                    .map(|p| Pasta {
                        nome: p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                        caminho: p.to_string_lossy().into_owned(),
                    })
                    .collect();
                v.sort_by(|a, b| ordem::natural(&a.nome, &b.nome));
                v
            },
            tocar_ao_abrir: self.config.tocar_ao_abrir,
            abrir_tela_cheia: self.config.abrir_tela_cheia,
            vinheta_entre_musicas: self.config.vinheta_entre_musicas,
            protegido: self.config.senha_hash.is_some(),
            bloqueios: self.config.bloqueios.clone(),
            nome_radio: self.config.nome_radio.clone(),
            audio: self.config.audio.clone(),
            gravacao: self.config.gravacao.clone(),
            transmissao: self.config.transmissao.clone(),
            transmissao_estado: self.transmissao_estado.clone(),
            erro_captura: self.erro_captura.clone(),
            gravando: self.captura.is_some()
                && self.erro_captura.is_none()
                && self.config.gravacao.ligada
                && self.config.gravacao.pasta.is_some(),
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
            // A vinheta entre músicas não é "agendamento esperando".
            aguardando: {
                let agendados: Vec<&Item> = self.pendentes.iter().filter(|p| p.tipo != Tipo::Vinheta).collect();
                agendados.first().map(|p| {
                    let pasta = if p.artista.is_empty() { p.titulo.clone() } else { p.artista.clone() };
                    if agendados.len() > 1 { format!("{pasta} ({})", agendados.len()) } else { pasta }
                })
            },
            repetir_auto: self.repetir_auto,
            pos_ducking: self.trocando_pos_ducking,
            proximo_agendamento: agenda::proximo(&self.agenda),
            link: self.link.as_ref().filter(|l| l.ligado).map(|l| l.url.clone()),
            previa: self.previa.as_ref().map(|p| PreviaDto {
                caminho: p.caminho.clone(),
                titulo: p.titulo.clone(),
                artista: p.artista.clone(),
                duracao: p.duracao,
                decorrido: if p.iniciada { p.decorrido() } else { p.base },
                espera: p.inicio.saturating_duration_since(Instant::now()).as_secs_f64(),
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
    /// Agendamento no ar que os botões não mexem: comercial ou hora certa
    /// tocando, ou o ducking (falando por cima, música repetindo ou os 10 s
    /// até a troca). Pausar, Repetir, Próximo e microfone ficam travados.
    fn travado_por_agendamento(&self) -> bool {
        let comercial = self.tocando.as_ref().is_some_and(|t| matches!(t.item.tipo, Tipo::Comercial | Tipo::HoraCerta));
        comercial || self.por_cima.is_some() || self.repetir_auto || self.trocando_pos_ducking
    }

    fn tem_entregue(&self) -> bool {
        self.pendentes.iter().chain(self.fila.iter()).any(|i| i.travado)
    }

    /// Vinheta sorteada dos botões da cartucheira (sem repetir a última).
    fn sortear_vinheta(&mut self) -> Option<PathBuf> {
        use rand::seq::IndexedRandom;
        let todas: Vec<&PathBuf> = self.config.cartucheira.iter().flatten().filter(|p| p.is_file()).collect();
        let outras: Vec<&PathBuf> = todas.iter().copied().filter(|p| Some(*p) != self.ultima_vinheta.as_ref()).collect();
        let escolhida = (*outras.choose(&mut rand::rng()).or_else(|| todas.first())?).clone();
        self.ultima_vinheta = Some(escolhida.clone());
        Some(escolhida)
    }

    /// Entre música e música entra uma vinheta da cartucheira. A vinheta e a
    /// música seguinte vão juntas ao motor: a música é analisada (corte de
    /// silêncio e volume) enquanto a vinheta toca.
    fn entregar_com_vinheta(&mut self) -> bool {
        let entre_musicas = self.tocando.as_ref().is_some_and(|t| t.item.tipo == Tipo::Musica)
            && self.fila.first().is_some_and(|i| i.tipo == Tipo::Musica);
        // Com ducking (falando por cima ou repetindo a música) não entra vinheta.
        let ducking = self.por_cima.is_some() || self.repetir_auto || self.trocando_pos_ducking;
        if !self.config.vinheta_entre_musicas || !entre_musicas || !self.pendentes.is_empty() || self.link.is_some() || ducking {
            return false;
        }
        let Some(arq) = self.sortear_vinheta() else { return false };
        let titulo = arq.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let mut vinheta = self.novo_item(arq.to_string_lossy().into_owned(), "Vinheta".into(), titulo, Tipo::Vinheta);
        vinheta.duracao = analise::duracao(&arq);
        let Some(motor) = &self.motor else { return false };
        if let Err(e) = motor.enfileirar(&vinheta.caminho, false) {
            self.erro_motor = Some(e);
            return false;
        }
        vinheta.travado = true;
        self.pendentes.push(vinheta);
        if let Some(musica) = self.fila.first_mut() {
            match motor.enfileirar(&musica.caminho, true) {
                Ok(()) => musica.travado = true,
                Err(e) => self.erro_motor = Some(e),
            }
        }
        true
    }

    /// Entrega ao motor o próximo: primeiro os agendados pendentes, depois a fila.
    fn enviar_proximo(&mut self) {
        if self.tem_entregue() {
            let presos: Vec<String> =
                self.pendentes.iter().chain(self.fila.iter()).filter(|i| i.travado).map(|i| registro::nome(&i.caminho)).collect();
            registro::anotar(format!("programa: não entrega, já entregue ao motor: {}", presos.join(", ")));
            return;
        }
        if self.entregar_com_vinheta() {
            return;
        }
        let Some(motor) = &self.motor else { return };
        let prox = match self.pendentes.first_mut() {
            Some(p) => Some(p),
            // Durante o link a fila de músicas espera.
            None if self.link.is_some() => None,
            None => self.fila.first_mut(),
        };
        if let Some(prox) = prox {
            match motor.enfileirar(&prox.caminho, prox.tipo == Tipo::Musica) {
                Ok(()) => prox.travado = true,
                Err(e) => self.erro_motor = Some(e),
            }
        }
    }

    /// Liga o link quando a abertura acabou (ou não havia abertura).
    fn ligar_link_se_pronto(&mut self) {
        if !self.pendentes.is_empty() || self.tem_entregue() {
            return;
        }
        let (Some(link), Some(motor)) = (self.link.as_mut(), &self.motor) else { return };
        if link.ligado {
            return;
        }
        match motor.link_ligar(&link.url) {
            Ok(()) => link.ligado = true,
            Err(e) => self.erro_motor = Some(e),
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
            // Tocando com o tempo do corte de silêncio: não volta para o do arquivo.
            .or_else(|| tocando.as_mut().map(|t| &mut t.item).filter(|i| i.id == id && i.duracao.is_none()));
        if let Some(item) = alvo {
            item.duracao = duracao;
        }
        emitir(&app, &e);
    });
}

/// Mantém o mínimo da fila puxando músicas aleatórias do acervo.
fn completar_fila(app: &AppHandle, estado: &Compartilhado, e: &mut Estado) {
    while e.fila.len() <= FILA_MINIMO && e.fila.len() < FILA_MAXIMO {
        // Baralho da pasta da lista (sem repetir até tocar todas); pasta vazia
        // ou sem lista: baralho do acervo todo.
        let lista = e.lista_ativa.clone();
        let da_lista = lista.and_then(|p| e.bib.proxima_em(Some(Path::new(&p))));
        let Some(f) = da_lista.or_else(|| e.bib.proxima_em(None)) else { break };
        let item = e.novo_item(f.caminho, f.artista, f.musica, Tipo::Musica);
        analisar_em_segundo_plano(app, estado, item.id, item.caminho.clone());
        e.fila.push(item);
    }
}

fn ao_evento_motor(app: &AppHandle, estado: &Compartilhado, geracao: u64, ev: EventoMotor) {
    let mut e = travar(estado);
    if geracao != e.geracao_motor {
        return;
    }
    match ev {
        EventoMotor::Faixa(caminho, _) if caminho.is_empty() => {
            // O motor ficou sem nada: entrega a próxima na hora (ao abrir,
            // só depois da procura de músicas novas).
            e.tocando = None;
            if e.no_ar && !e.pausado && !e.procurando_musicas {
                registro::anotar("programa: motor vazio → manda a próxima");
                completar_fila(app, estado, &mut e);
                e.enviar_proximo();
                e.ligar_link_se_pronto();
            } else {
                registro::anotar(format!(
                    "programa: motor vazio, não manda (no ar={}, pausado={}, procurando músicas={})",
                    e.no_ar, e.pausado, e.procurando_musicas
                ));
            }
        }
        EventoMotor::Faixa(caminho, toca) => {
            let repeticao = e.repeticao_enviada && e.tocando.as_ref().is_some_and(|t| t.item.caminho == caminho);
            e.repeticao_enviada = false;
            let pend = e.pendentes.iter().position(|i| i.travado && i.caminho == caminho);
            let pos = e.fila.iter().position(|i| i.travado && i.caminho == caminho);
            let origem = match (repeticao, pend, pos) {
                (true, _, _) => "repetição",
                (false, Some(_), _) => "agendado",
                (false, None, Some(_)) => "fila",
                (false, None, None) => "não estava na fila",
            };
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
            let mut item = item;
            // Música com o silêncio cortado: vale o tempo que vai tocar de fato.
            if toca.is_some() {
                item.duracao = toca;
            }
            registro::anotar(format!(
                "programa: tocando {} ({origem}, {:?}, duração {})",
                registro::nome(&item.caminho),
                item.tipo,
                item.duracao.map(|d| format!("{d:.1} s")).unwrap_or_else(|| "?".into())
            ));
            e.tocando = Some(Tocando { item, inicio: Instant::now(), pausa_desde: None, pausado_total: Duration::ZERO });
            completar_fila(app, estado, &mut e);
        }
        EventoMotor::Fala(caminho) => {
            e.por_cima = (!caminho.is_empty()).then(|| {
                let p = Path::new(&caminho);
                agenda::nome_sem_validade(&p.file_stem().map(|s| s.to_string_lossy()).unwrap_or_default())
            });
            if caminho.is_empty() {
                registro::anotar(format!("programa: fala acabou (repetindo a música={})", e.repetir_auto));
            }
            if caminho.is_empty() && e.repetir_auto {
                e.repetir_auto = false;
                trocar_depois_do_ducking(app, estado, &mut e);
            }
        }
        EventoMotor::Previa(caminho) => {
            if caminho.is_empty() {
                if e.previa.as_ref().is_some_and(|p| p.iniciada) {
                    e.previa = None;
                }
            } else if let Some(p) = e.previa.as_mut().filter(|p| p.caminho == caminho) {
                // Começou (do início ou do ponto clicado, que já está em `base`).
                // O som chega à saída um pouco depois: a barra espera junto.
                p.iniciada = true;
                p.inicio = Instant::now() + Duration::from_secs_f64(motor::ATRASO_SAIDA);
            }
        }
        EventoMotor::Parou(_) if e.motor.is_none() => {
            // Falhou ao iniciar: a mensagem já foi dada por quem tentou ligar.
        }
        EventoMotor::Parou(erro) => {
            registro::anotar(format!("programa: o motor caiu: {}", erro.lines().last().unwrap_or("")));
            // O motor caiu. Se a rádio estava no ar, continua no ar: o motor é
            // religado em 3 s e segue com a PRÓXIMA da fila (a que tocava não
            // volta: se foi ela que derrubou o motor, cairia de novo).
            if e.pausado {
                e.no_ar = false;
                e.pausado = false;
                e.saida_parada = false;
            }
            e.geracao_motor += 1;
            e.motor = None;
            e.previa = None;
            e.por_cima = None;
            e.ducking = false;
            e.tocando = None;
            e.repeticao_enviada = false;
            e.repetir_auto = false;
            e.destravar_tudo();
            if let Some(l) = e.link.as_mut() {
                l.ligado = false;
            }
            let erro = erro.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").to_string();
            let erro = if erro.is_empty() { "O motor de áudio parou".into() } else { erro };
            e.erro_motor = Some(format!("{erro} (religando…)"));
            religar_motor_depois(app, estado, e.geracao_motor, Duration::from_secs(3));
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
    let (app, estado, dir, audio, geracao) = (app.clone(), estado.clone(), e.dir.clone(), e.config.audio.clone(), e.geracao_motor);
    std::thread::spawn(move || {
        let (app2, estado2) = (app.clone(), estado.clone());
        let resultado = Motor::iniciar(&exe, &dir, &audio, move |ev| ao_evento_motor(&app2, &estado2, geracao, ev));
        let mut e = travar(&estado);
        e.iniciando_motor = false;
        if geracao != e.geracao_motor {
            // A placa mudou enquanto ligava: descarta este e liga outro.
            drop(resultado);
            iniciar_motor(&app, &estado, &mut e);
            return;
        }
        match resultado {
            Ok(m) => {
                e.motor = Some(m);
                e.erro_motor = None;
                if e.no_ar && !e.pausado && e.tocando.is_none() && !e.procurando_musicas {
                    completar_fila(&app, &estado, &mut e);
                    e.enviar_proximo();
                    e.ligar_link_se_pronto();
                }
            }
            // Rádio no ar e o motor não liga (ex.: placa USB desconectada):
            // continua tentando a cada 10 s, sem tirar do ar.
            Err(err) if e.no_ar => {
                e.erro_motor = Some(format!("{err} (tentando de novo…)"));
                religar_motor_depois(&app, &estado, geracao, Duration::from_secs(10));
            }
            Err(err) => e.erro_motor = Some(err),
        }
        emitir(&app, &e);
    });
}

/// Religa o motor depois de um tempo, se ninguém o religou nesse meio tempo
/// (por exemplo, trocando a placa de som).
fn religar_motor_depois(app: &AppHandle, estado: &Compartilhado, geracao: u64, espera: Duration) {
    let (app, estado) = (app.clone(), estado.clone());
    std::thread::spawn(move || {
        std::thread::sleep(espera);
        let mut e = travar(&estado);
        if geracao == e.geracao_motor && e.motor.is_none() {
            iniciar_motor(&app, &estado, &mut e);
        }
    });
}

/// Troca de placa: fecha o motor e liga outro com a placa nova. O que estava
/// tocando é interrompido e a rádio segue com a próxima da fila.
fn reiniciar_motor(app: &AppHandle, estado: &Compartilhado, e: &mut Estado) {
    e.geracao_motor += 1;
    e.motor = None;
    e.tocando = None;
    e.previa = None;
    e.por_cima = None;
    e.repeticao_enviada = false;
    e.repetir_auto = false;
    e.destravar_tudo();
    if let Some(l) = e.link.as_mut() {
        l.ligado = false;
    }
    iniciar_motor(app, estado, e);
}

// ---------- gravação e transmissão ----------

/// (Re)liga a captura da entrada com as configurações atuais.
fn iniciar_captura(app: &AppHandle, estado: &Compartilhado, e: &mut Estado) {
    e.geracao_captura += 1;
    e.captura = None;
    e.erro_captura = None;
    let (g, t) = (&e.config.gravacao, &e.config.transmissao);
    let grava = g.ligada && g.pasta.is_some();
    e.transmissao_estado = if t.ligada { "conectando".into() } else { "desligada".into() };
    if !grava && !t.ligada {
        return;
    }
    let Some(exe) = motor::localizar() else {
        e.erro_captura = Some("Liquidsoap não encontrado. Instale o pacote liquidsoap.".into());
        return;
    };
    let geracao = e.geracao_captura;
    let dir = e.dir.clone();
    let (audio, g, t, nome) = (e.config.audio.clone(), g.clone(), t.clone(), e.config.nome_radio.clone());
    let (app, estado) = (app.clone(), estado.clone());
    // Em segundo plano: no Mac, achar a placa leva um segundo.
    std::thread::spawn(move || {
        let log = dir.join("captura.log").to_string_lossy().into_owned();
        let script = captura::script(&log, &audio, &g, &t, &nome);
        let (app2, estado2) = (app.clone(), estado.clone());
        let resultado = captura::Captura::iniciar(&exe, &dir, script, move |ev| ao_evento_captura(&app2, &estado2, geracao, ev));
        let mut e = travar(&estado);
        if geracao != e.geracao_captura {
            return;
        }
        match resultado {
            Ok(c) => e.captura = Some(c),
            Err(err) => e.erro_captura = Some(err),
        }
        emitir(&app, &e);
    });
}

fn ao_evento_captura(app: &AppHandle, estado: &Compartilhado, geracao: u64, ev: captura::EventoCaptura) {
    use captura::EventoCaptura as Ev;
    let mut e = travar(estado);
    if geracao != e.geracao_captura {
        return;
    }
    match ev {
        Ev::Transmissao(t) => {
            e.transmissao_estado = match t.strip_prefix("erro") {
                Some(bruto) => format!("erro: {}", captura::erro_legivel(bruto)),
                None => t,
            }
        }
        Ev::Gravacao(erro) => e.erro_captura = Some(format!("Gravação: {erro}")),
        Ev::Parou(erro) => {
            e.captura = None;
            let erro = if erro.is_empty() { "a captura da entrada parou".to_string() } else { erro };
            e.erro_captura = Some(format!("{erro} (tentando de novo…)"));
            // Tenta de novo em 5 s, se nada mudou nesse meio tempo.
            let (app, estado) = (app.clone(), estado.clone());
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_secs(5));
                let mut e = travar(&estado);
                if geracao == e.geracao_captura {
                    iniciar_captura(&app, &estado, &mut e);
                    emitir(&app, &e);
                }
            });
        }
    }
    emitir(app, &e);
}

/// Uma vez por hora, apaga as pastas de gravação com mais de 30 dias.
fn iniciar_limpeza_gravacoes(estado: Compartilhado) {
    std::thread::spawn(move || loop {
        let pasta = travar(&estado).config.gravacao.pasta.clone();
        if let Some(p) = pasta {
            captura::apagar_antigas(&p, chrono::Local::now().date_naive());
        }
        std::thread::sleep(Duration::from_secs(3600));
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
        if let Some(ad) = &e.ducking_adiado {
            let na_proxima = e.tocando.as_ref().is_some_and(|t| {
                t.item.tipo == Tipo::Musica && Some(t.item.id) != ad.depois_de && t.decorrido() >= DUCKING_DEPOIS
            });
            if ad.desde.elapsed() > DUCKING_VALIDADE {
                registro::anotar("ducking que esperava: descartado (mais de 10 min)");
                e.ducking_adiado = None;
            } else if na_proxima && e.link.is_none() && !e.tem_entregue() {
                registro::anotar("ducking que esperava: entra agora (10 s depois do início da música)");
                let ad = e.ducking_adiado.take().expect("adiado");
                falar_por_cima(&mut e, ad.arquivos);
                emitir(&app, &e);
            }
        }
        let falta = e.tocando.as_ref().and_then(|t| t.item.duracao.map(|d| d - t.decorrido()));
        if !falta.is_some_and(|f| f <= ANTECEDENCIA) {
            continue;
        }
        if e.link.is_none() && (e.repetir || e.repetir_auto) {
            // Repetir: entrega de novo a mesma música, a fila espera.
            if !e.repeticao_enviada {
                let atual = e.tocando.as_ref().map(|t| (t.item.caminho.clone(), t.item.tipo == Tipo::Musica));
                if let (Some(m), Some((c, musica))) = (&e.motor, atual) {
                    registro::anotar(format!(
                        "relógio: repete {} ({})",
                        registro::nome(&c),
                        if e.repetir_auto { "ducking não coube" } else { "botão Repetir" }
                    ));
                    if m.enfileirar(&c, musica).is_ok() {
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
/// O volume do fade vale antes do crossfade (`vol_troca`), então o fim da
/// música que sai continua baixo enquanto a próxima entra.
/// `pos_ducking`: é a troca depois do ducking (descarta a vinheta que esperava).
fn fade_e_proxima(app: AppHandle, estado: Compartilhado, pos_ducking: bool) {
    registro::anotar(format!("fade de 3 s começou ({})", if pos_ducking { "depois do ducking" } else { "lista ou link" }));
    std::thread::spawn(move || {
        const PASSOS: u32 = 15;
        for i in 1..=PASSOS {
            std::thread::sleep(Duration::from_millis(200));
            let e = travar(&estado);
            if let Some(m) = &e.motor {
                let _ = m.volume_troca(1.0 - f64::from(i) / f64::from(PASSOS));
            }
        }
        let mut e = travar(&estado);
        registro::anotar("fade terminou: pula a música no motor");
        if let Some(m) = &e.motor {
            let _ = m.esvaziar_e_pular();
            let _ = m.volume_troca(1.0);
        }
        if pos_ducking {
            // Como no Passar: vai direto para a próxima, sem a vinheta que esperava.
            // (Só aqui: no link, a "vinheta" pendente é a abertura, que fica.)
            e.pendentes.retain(|i| i.tipo != Tipo::Vinheta);
            e.trocando_pos_ducking = false;
        }
        e.repeticao_enviada = false;
        e.destravar_tudo();
        // Não espera o motor avisar que ficou vazio (o aviso pode não vir e a
        // rádio ficava muda): manda a próxima já. No link, quem entra é a
        // abertura/link, pelo aviso do motor, como antes.
        if e.no_ar && e.link.is_none() {
            registro::anotar("fade: manda a próxima sem esperar o aviso do motor");
            e.tocando = None;
            completar_fila(&app, &estado, &mut e);
            e.enviar_proximo();
        }
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
    registro::anotar(format!(
        "agendamento: {:?} {} (no ar={}, link={})",
        ag.tipo,
        ag.pastas.iter().map(|p| registro::nome(&p.caminho)).collect::<Vec<_>>().join(" + "),
        e.no_ar,
        e.link.is_some()
    ));
    match ag.tipo {
        TipoAgenda::Lista => {
            if let Some(p) = ag.pastas.first() {
                e.lista_ativa = Some(p.caminho.clone());
            }
            // Limpa a fila e põe músicas novas da pasta. Comercial e hora certa
            // tocando não levam fade (agendamento não se corta).
            let agendado = e.tocando.as_ref().is_some_and(|t| matches!(t.item.tipo, Tipo::Comercial | Tipo::HoraCerta));
            let fade = e.no_ar && ag.modo_lista == ModoLista::Fade && e.link.is_none() && !agendado && e.tocando.is_some();
            if fade {
                e.fila.clear();
            } else {
                // O que já foi entregue ao motor (últimos segundos da música) fica.
                e.fila.retain(|i| i.travado);
            }
            completar_fila(app, estado, e);
            if e.link.is_some() {
                encerrar_link(app, estado, e);
            } else if fade {
                e.repetir_auto = false;
                fade_e_proxima(app.clone(), estado.clone(), false);
            }
        }
        // O resto só quando a rádio está no ar.
        _ if !e.no_ar => {}
        TipoAgenda::Link => iniciar_link(app, estado, e, ag),
        // Durante o link, comercial e hora certa são descartados.
        _ if e.link.is_some() => {}
        TipoAgenda::Comercial => {
            let dir = e.dir.clone();
            let bloco = agenda::montar_bloco(ag, &mut e.rodizio, &dir);
            agendar_em_sequencia(app, estado, e, bloco, Tipo::Comercial);
        }
        TipoAgenda::Ducking => {
            let dir = e.dir.clone();
            let arquivos = agenda::montar_bloco(ag, &mut e.rodizio, &dir);
            if arquivos.is_empty() {
                return;
            }
            // Entra agora só com música tocando e nada entregue ao motor ainda.
            // Senão (fim da música, a próxima já foi), espera a próxima música.
            let musica = e.tocando.as_ref().is_some_and(|t| t.item.tipo == Tipo::Musica);
            if musica && !e.tem_entregue() {
                falar_por_cima(e, arquivos);
            } else {
                let depois_de = e.tocando.as_ref().map(|t| t.item.id);
                registro::anotar(format!(
                    "ducking: espera a próxima música + 10 s (tocando música={musica}, próxima já entregue={})",
                    e.tem_entregue()
                ));
                e.ducking_adiado = Some(DuckingAdiado { arquivos, depois_de, desde: Instant::now() });
            }
        }
        TipoAgenda::HoraCerta => {
            // Espera a música acabar: a hora falada é a do fim da música (e do
            // que já estiver esperando antes), não a de agora.
            let falta = e.tocando.as_ref().and_then(|t| t.item.duracao.map(|d| d - t.decorrido())).unwrap_or(0.0);
            let antes: f64 = e.pendentes.iter().filter_map(|i| i.duracao).sum();
            let espera = (falta.max(0.0) + antes) * 1000.0;
            let inicio = (chrono::Local::now() + chrono::Duration::milliseconds(espera as i64)).time();
            let dir = e.dir.clone();
            let (hora_certa, comercial) = agenda::montar_hora_certa(ag, inicio, &mut e.rodizio, &dir);
            agendar_em_sequencia(app, estado, e, hora_certa, Tipo::HoraCerta);
            agendar_em_sequencia(app, estado, e, comercial.into_iter().collect(), Tipo::Comercial);
        }
    }
}

/// A fala acabou com a música repetindo: ela volta ao volume normal, toca
/// mais `DEPOIS_DO_DUCKING` e então sai com fade para a próxima. Se nesse meio
/// tempo a música mudou (Próximo, acabou sozinha) ou entrou outro ducking ou
/// link, a troca não acontece.
fn trocar_depois_do_ducking(app: &AppHandle, estado: &Compartilhado, e: &mut Estado) {
    registro::anotar("fim da fala: música volta ao normal, troca para a próxima em 10 s");
    e.vez_ducking += 1;
    e.trocando_pos_ducking = true;
    let vez = e.vez_ducking;
    let musica = e.tocando.as_ref().map(|t| t.item.id);
    let (app, estado) = (app.clone(), estado.clone());
    std::thread::spawn(move || {
        std::thread::sleep(DEPOIS_DO_DUCKING);
        let mut e = travar(&estado);
        let mesma = e.tocando.as_ref().map(|t| t.item.id) == musica;
        if e.vez_ducking != vez || !mesma || !e.no_ar || e.pausado || e.link.is_some() || e.repetir_auto {
            registro::anotar(format!(
                "troca depois do ducking cancelada (mesma música={mesma}, outro ducking={}, no ar={}, pausado={}, link={})",
                e.vez_ducking != vez,
                e.no_ar,
                e.pausado,
                e.link.is_some()
            ));
            if e.vez_ducking == vez {
                e.trocando_pos_ducking = false;
            }
            return;
        }
        drop(e);
        fade_e_proxima(app, estado, true);
    });
}

/// Comercial com ducking por cima da música. Se o bloco não cabe no que
/// falta, a música repete por baixo e, quando a fala acabar, sai com fade de
/// 3 s para a próxima. Só é chamado sem nada entregue ao motor, então a
/// repetição vem logo depois da música.
fn falar_por_cima(e: &mut Estado, arquivos: Vec<PathBuf>) {
    // Uma troca marcada por um ducking anterior não vale mais.
    e.vez_ducking += 1;
    let total: f64 = arquivos.iter().filter_map(|p| analise::duracao(p)).sum();
    let falta = e.tocando.as_ref().and_then(|t| t.item.duracao.map(|d| d - t.decorrido()));
    if falta.is_some_and(|f| total > f - 1.0) {
        e.repetir_auto = true;
    }
    registro::anotar(format!(
        "ducking: {} arquivo(s), {total:.0} s de fala, faltam {} da música → {}",
        arquivos.len(),
        falta.map(|f| format!("{f:.0} s")).unwrap_or_else(|| "?".into()),
        if e.repetir_auto { "não cabe, repete a música" } else { "cabe" }
    ));
    let bloco: Vec<String> = arquivos.into_iter().map(|p| p.to_string_lossy().into_owned()).collect();
    if let Some(m) = &e.motor {
        if let Err(err) = m.falar(&bloco) {
            e.erro_motor = Some(err);
        }
    }
}

/// Entra o link: fade na música, abertura (se tiver) e o link.
fn iniciar_link(app: &AppHandle, estado: &Compartilhado, e: &mut Estado, ag: &Agendamento) {
    let Some(url) = ag.url.clone() else { return };
    registro::anotar(format!("link: entra {url}"));
    // Outro link no ar: sai sem fade e entra o novo.
    if e.link.take().is_some_and(|l| l.ligado) {
        if let Some(m) = &e.motor {
            let _ = m.link_desligar();
        }
    }
    // Comercial e hora certa que esperavam a música acabar ficam de fora
    // (e o ducking que esperava a próxima música também).
    e.pendentes.clear();
    e.ducking_adiado = None;
    e.repetir_auto = false;
    e.repeticao_enviada = false;
    let dir = e.dir.clone();
    if let Some(abertura) = ag.abertura.as_deref().and_then(|p| agenda::um_da_pasta(p, &mut e.rodizio, &dir)) {
        agendar_em_sequencia(app, estado, e, vec![abertura], Tipo::Vinheta);
    }
    e.link = Some(LinkAtivo { url, ligado: false });
    if e.tocando.is_some() {
        // Quando o fade acabar, o motor fica sem nada e o evento de silêncio
        // entrega a abertura ou liga o link.
        fade_e_proxima(app.clone(), estado.clone(), false);
    } else {
        e.enviar_proximo();
        e.ligar_link_se_pronto();
    }
}

/// Tira o link do ar com fade de 3 s e a fila de músicas volta.
fn encerrar_link(app: &AppHandle, estado: &Compartilhado, e: &mut Estado) {
    let Some(link) = e.link.take() else { return };
    registro::anotar("link: sai");
    if !link.ligado {
        // Ainda na abertura: quando ela acabar, a fila segue normalmente.
        e.pendentes.retain(|i| i.travado);
        return;
    }
    let (app, estado) = (app.clone(), estado.clone());
    std::thread::spawn(move || {
        const PASSOS: u32 = 15;
        for i in 1..=PASSOS {
            std::thread::sleep(Duration::from_millis(200));
            let e = travar(&estado);
            if let Some(m) = &e.motor {
                let _ = m.link_volume(1.0 - f64::from(i) / f64::from(PASSOS));
            }
        }
        let mut e = travar(&estado);
        if let Some(m) = &e.motor {
            let _ = m.link_desligar();
            let _ = m.link_volume(1.0);
        }
        // Um novo link pode ter entrado durante o fade.
        if e.link.is_none() && e.no_ar && !e.pausado && e.tocando.is_none() {
            completar_fila(&app, &estado, &mut e);
            e.enviar_proximo();
        }
        emitir(&app, &e);
    });
}

/// Fecha o motor e a gravação/transmissão (ao sair ou reiniciar).
fn fechar_processos(e: &mut Estado) {
    // Avisos dos processos antigos passam a ser ignorados.
    e.geracao_motor += 1;
    e.geracao_captura += 1;
    e.motor = None;
    e.captura = None;
}

/// Fecha e abre o programa de novo (o motor não ligou, por exemplo).
#[tauri::command]
fn reiniciar_programa(app: AppHandle, estado: State<Compartilhado>) {
    fechar_processos(&mut travar(&estado));
    app.restart();
}

/// Fecha o programa (o motor e a gravação fecham junto, no `RunEvent::Exit`).
#[tauri::command]
fn fechar_programa(app: AppHandle) {
    app.exit(0);
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
    ag.validar()?;
    let mut e = travar(&estado);
    if let Some(msg) = agenda::conflito(&e.agenda, &ag) {
        return Err(msg);
    }
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
        // Ao abrir (lista guardada) vem a procura de músicas novas: a rádio já
        // espera por ela desde agora, antes de o motor avisar que está livre.
        e.procurando_musicas = !forcar_varredura;
        e.vigia = None;
        emitir(&app, &e);
        (raiz, e.arquivo_indice())
    };
    std::thread::spawn(move || {
        let do_indice = (!forcar_varredura).then(|| Biblioteca::carregar_indice(raiz.clone(), &indice)).flatten();
        // Veio da lista guardada: abre na hora e, em segundo plano, relê a pasta
        // procurando músicas novas, apagadas ou renomeadas desde a última vez.
        if do_indice.is_some() {
            procurar_mudancas(app.clone(), estado.clone(), raiz.clone());
        }
        let veio_do_indice = do_indice.is_some();
        let bib = do_indice.unwrap_or_else(|| Biblioteca::varrer(raiz.clone()));
        let mut e = travar(&estado);
        if !veio_do_indice {
            // Sem lista guardada (primeira vez): acabou de varrer tudo.
            e.procurando_musicas = false;
        }
        e.bib = bib;
        let _ = e.bib.salvar_indice(&indice);
        e.carregando = false;
        e.vigia = vigiar(&app, &estado, &raiz);
        completar_fila(&app, &estado, &mut e);
        // "Tocar ao abrir": se o motor já está pronto, começa assim que há músicas
        // (e a procura de músicas novas terminou).
        if e.no_ar && !e.pausado && e.tocando.is_none() && !e.procurando_musicas {
            e.enviar_proximo();
        }
        emitir(&app, &e);
    });
}

/// Relê a pasta de músicas inteira sem travar nada e, se mudou algo, troca a
/// lista de músicas (os baralhos continuam).
fn procurar_mudancas(app: AppHandle, estado: Compartilhado, raiz: PathBuf) {
    travar(&estado).procurando_musicas = true;
    // Travou (Google Drive sem resposta)? Depois do limite começa com o que tem.
    let (app2, estado2) = (app.clone(), estado.clone());
    std::thread::spawn(move || {
        std::thread::sleep(LIMITE_PROCURA);
        let mut e = travar(&estado2);
        if e.procurando_musicas {
            e.procurando_musicas = false;
            comecar_a_tocar(&app2, &estado2, &mut e);
            emitir(&app2, &e);
        }
    });
    std::thread::spawn(move || {
        let nova = Biblioteca::varrer(raiz);
        let mut e = travar(&estado);
        if !e.carregando && e.bib.trocar_faixas(nova) {
            let arq = e.arquivo_indice();
            let _ = e.bib.salvar_indice(&arq);
        }
        if e.procurando_musicas {
            e.procurando_musicas = false;
            comecar_a_tocar(&app, &estado, &mut e);
        }
        emitir(&app, &e);
    });
}

/// Rádio no ar e nada tocando: completa a fila e entrega a primeira ao motor.
fn comecar_a_tocar(app: &AppHandle, estado: &Compartilhado, e: &mut Estado) {
    if e.no_ar && !e.pausado && e.tocando.is_none() {
        completar_fila(app, estado, e);
        e.enviar_proximo();
        e.ligar_link_se_pronto();
    }
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

#[derive(Serialize, Clone)]
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
    travar(&estado).retrato().pastas
}

#[tauri::command]
fn pasta_adicionar(app: AppHandle, estado: State<Compartilhado>, caminho: String) -> Result<(), String> {
    let mut e = travar(&estado);
    let p = PathBuf::from(caminho);
    if !e.config.pastas.contains(&p) {
        e.config.pastas.push(p);
        e.config.salvar(&e.dir).map_err(|err| err.to_string())?;
    }
    emitir(&app, &e);
    Ok(())
}

#[tauri::command]
fn pasta_remover(app: AppHandle, estado: State<Compartilhado>, caminho: String) -> Result<(), String> {
    let mut e = travar(&estado);
    e.config.pastas.retain(|p| p != Path::new(&caminho));
    e.config.salvar(&e.dir).map_err(|err| err.to_string())?;
    emitir(&app, &e);
    Ok(())
}

#[tauri::command]
fn nome_radio_definir(app: AppHandle, estado: State<Compartilhado>, nome: String) -> Result<(), String> {
    let mut e = travar(&estado);
    let nome = nome.trim();
    e.config.nome_radio = if nome.is_empty() { "Radio On Air".into() } else { nome.into() };
    e.config.salvar(&e.dir).map_err(|err| err.to_string())?;
    // O nome também vai na transmissão.
    if e.config.transmissao.ligada {
        iniciar_captura(&app, estado.inner(), &mut e);
    }
    emitir(&app, &e);
    Ok(())
}

#[tauri::command]
async fn placas() -> placas::Placas {
    tauri::async_runtime::spawn_blocking(placas::listar).await.unwrap_or_default()
}

#[tauri::command]
fn audio_definir(app: AppHandle, estado: State<Compartilhado>, audio: config::Audio) -> Result<(), String> {
    let compartilhado = estado.inner().clone();
    let mut e = travar(&compartilhado);
    let antes = std::mem::replace(&mut e.config.audio, audio);
    e.config.salvar(&e.dir).map_err(|err| err.to_string())?;
    let novo = e.config.audio.clone();
    if antes.saida != novo.saida || antes.pre_escuta != novo.pre_escuta {
        reiniciar_motor(&app, &compartilhado, &mut e);
    }
    if antes.entrada != novo.entrada {
        iniciar_captura(&app, &compartilhado, &mut e);
    }
    emitir(&app, &e);
    Ok(())
}

#[tauri::command]
fn gravacao_definir(app: AppHandle, estado: State<Compartilhado>, mut gravacao: config::Gravacao) -> Result<(), String> {
    gravacao.validar()?;
    let mut e = travar(&estado);
    e.config.gravacao = gravacao;
    e.config.salvar(&e.dir).map_err(|err| err.to_string())?;
    iniciar_captura(&app, estado.inner(), &mut e);
    emitir(&app, &e);
    Ok(())
}

#[tauri::command]
fn transmissao_definir(app: AppHandle, estado: State<Compartilhado>, mut transmissao: config::Transmissao) -> Result<(), String> {
    transmissao.validar()?;
    let mut e = travar(&estado);
    e.config.transmissao = transmissao;
    e.config.salvar(&e.dir).map_err(|err| err.to_string())?;
    iniciar_captura(&app, estado.inner(), &mut e);
    emitir(&app, &e);
    Ok(())
}

#[tauri::command]
fn definir_vinheta_entre_musicas(app: AppHandle, estado: State<Compartilhado>, valor: bool) -> Result<(), String> {
    let mut e = travar(&estado);
    e.config.vinheta_entre_musicas = valor;
    e.config.salvar(&e.dir).map_err(|err| err.to_string())?;
    emitir(&app, &e);
    Ok(())
}

#[tauri::command]
fn definir_abrir_tela_cheia(app: AppHandle, estado: State<Compartilhado>, valor: bool) -> Result<(), String> {
    let mut e = travar(&estado);
    e.config.abrir_tela_cheia = valor;
    e.config.salvar(&e.dir).map_err(|err| err.to_string())?;
    emitir(&app, &e);
    Ok(())
}

/// Botão do topo e tecla F11: entra ou sai da tela cheia. Devolve como ficou.
#[tauri::command]
fn alternar_tela_cheia(janela: tauri::WebviewWindow) -> bool {
    let nova = !janela.is_fullscreen().unwrap_or(false);
    let _ = janela.set_fullscreen(nova);
    nova
}

#[tauri::command]
fn tela_cheia_agora(janela: tauri::WebviewWindow) -> bool {
    janela.is_fullscreen().unwrap_or(false)
}

#[tauri::command]
fn definir_tocar_ao_abrir(app: AppHandle, estado: State<Compartilhado>, valor: bool) -> Result<(), String> {
    let mut e = travar(&estado);
    e.config.tocar_ao_abrir = valor;
    e.config.salvar(&e.dir).map_err(|err| err.to_string())?;
    emitir(&app, &e);
    Ok(())
}

fn senha_confere(e: &Estado, senha: &str) -> bool {
    e.config.senha_hash.as_deref().is_none_or(|h| h == config::hash_senha(senha))
}

#[tauri::command]
fn senha_conferir(estado: State<Compartilhado>, senha: String) -> bool {
    senha_confere(&travar(&estado), &senha)
}

/// Cria, troca ou remove (nova vazia) a senha. Pede a atual se já houver uma.
#[tauri::command]
fn senha_definir(app: AppHandle, estado: State<Compartilhado>, atual: String, nova: String) -> Result<(), String> {
    let mut e = travar(&estado);
    if !senha_confere(&e, &atual) {
        return Err("Senha atual incorreta.".into());
    }
    e.config.senha_hash = (!nova.is_empty()).then(|| config::hash_senha(&nova));
    e.config.salvar(&e.dir).map_err(|err| err.to_string())?;
    emitir(&app, &e);
    Ok(())
}

#[tauri::command]
fn bloqueios_definir(
    app: AppHandle,
    estado: State<Compartilhado>,
    bloqueios: config::Bloqueios,
    senha: String,
) -> Result<(), String> {
    let mut e = travar(&estado);
    if !senha_confere(&e, &senha) {
        return Err("Senha incorreta.".into());
    }
    e.config.bloqueios = bloqueios;
    e.config.salvar(&e.dir).map_err(|err| err.to_string())?;
    emitir(&app, &e);
    Ok(())
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
    registro::anotar(format!("botão Tocar (pausado={}, no ar={})", e.pausado, e.no_ar));
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
    registro::anotar("botão Pausar");
    if !e.no_ar || e.pausado {
        return;
    }
    if e.travado_por_agendamento() {
        registro::anotar("botão Pausar ignorado: agendamento ou ducking no ar");
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
    registro::anotar("botão Parar");
    e.no_ar = false;
    e.pausado = false;
    if let Some(m) = &e.motor {
        let _ = m.pausar();
        let _ = m.pular();
        if e.link.is_some() {
            let _ = m.link_desligar();
        }
        e.saida_parada = true;
    }
    e.link = None;
    e.tocando = None;
    emitir(&app, &e);
}

#[tauri::command]
fn proxima(app: AppHandle, estado: State<Compartilhado>) {
    let compartilhado = estado.inner().clone();
    let mut e = travar(&compartilhado);
    let tocando = e.tocando.as_ref().map(|t| registro::nome(&t.item.caminho)).unwrap_or_else(|| "(nada)".into());
    registro::anotar(format!("botão Próximo (tocando {tocando}, no ar={}, link={})", e.no_ar, e.link.is_some()));
    if !e.no_ar {
        return;
    }
    // Agendamento não se pula: comercial, hora certa e link tocam até o fim.
    if e.link.is_some() {
        return;
    }
    if e.travado_por_agendamento() {
        registro::anotar("botão Próximo ignorado: agendamento ou ducking no ar");
        return;
    }
    completar_fila(&app, &compartilhado, &mut e);
    // Passar vai direto para a próxima música, sem a vinheta entre elas.
    e.pendentes.retain(|i| i.tipo != Tipo::Vinheta);
    // Tira do motor o que já tinha sido entregue (inclusive uma repetição),
    // pula a atual e já manda a próxima, sem esperar o motor avisar que ficou
    // vazio (com o motor já parado esse aviso não vem e o Passar não fazia nada).
    if let Some(m) = &e.motor {
        let _ = m.esvaziar_e_pular();
    }
    e.repeticao_enviada = false;
    e.trocando_pos_ducking = false;
    e.destravar_tudo();
    e.tocando = None;
    e.enviar_proximo();
    emitir(&app, &e);
}

#[tauri::command]
fn alternar_repetir(app: AppHandle, estado: State<Compartilhado>) {
    let mut e = travar(&estado);
    // Ligar só com tempo de sobra: perto do fim a próxima já pode ter ido ao motor.
    let falta = e.tocando.as_ref().and_then(|t| t.item.duracao.map(|d| d - t.decorrido()));
    if !e.repetir && falta.is_some_and(|f| f < REPETIR_MINIMO) {
        return;
    }
    // Ligar não dá com agendamento, ducking ou link no ar (desligar sempre dá).
    if !e.repetir && (e.travado_por_agendamento() || e.link.is_some()) {
        return;
    }
    e.repetir = !e.repetir;
    emitir(&app, &e);
}

/// Liga/desliga o ducking, descendo/subindo o volume suavemente (~0,6 s).
#[tauri::command]
fn alternar_ducking(app: AppHandle, estado: State<Compartilhado>) {
    let compartilhado = estado.inner().clone();
    let (de, para) = {
        let mut e = travar(&compartilhado);
        // Ligar não dá com agendamento, ducking ou link no ar (desligar sempre
        // dá). No link nem faria efeito: o microfone abaixa só a música.
        if !e.ducking && (e.travado_por_agendamento() || e.link.is_some()) {
            return;
        }
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

/// Pula a pré-escuta para `segundos` a partir do início (para frente ou para trás).
#[tauri::command]
fn previa_ir(app: AppHandle, estado: State<Compartilhado>, segundos: f64) {
    let mut e = travar(&estado);
    let Estado { motor, previa, .. } = &mut *e;
    if let (Some(m), Some(p)) = (motor.as_ref(), previa.as_mut()) {
        let fim = p.duracao.map_or(f64::MAX, |d| (d - 1.0).max(0.0));
        let alvo = segundos.clamp(0.0, fim);
        // Até o motor avisar que recomeçou, a tela fica parada no ponto clicado
        // (e o aviso de "acabou" da faixa antiga é ignorado).
        p.iniciada = false;
        p.base = alvo;
        if m.previa_ir(&p.caminho, alvo).is_err() {
            p.iniciada = true;
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
            let dir = sistema::pasta_dados(app.path().app_data_dir().expect("pasta de dados"));
            registro::iniciar(&dir);
            // No Windows o Liquidsoap vem dentro do instalador.
            if let Ok(recursos) = app.path().resource_dir() {
                motor::usar_embutido(&recursos);
            }
            let config = Config::carregar(&dir);
            let agenda = agenda::carregar(&dir);
            let rodizio = Rodizio::carregar(&dir);
            let lista_ativa = agenda::lista_vigente(&agenda);
            if config.abrir_tela_cheia {
                if let Some(janela) = app.get_webview_window("main") {
                    let _ = janela.set_fullscreen(true);
                }
            }
            let estado: Compartilhado = Arc::new(Mutex::new(Estado {
                dir,
                config: config.clone(),
                bib: Biblioteca::default(),
                carregando: false,
                procurando_musicas: false,
                fila: vec![],
                tocando: None,
                no_ar: config.tocar_ao_abrir,
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
                ducking_adiado: None,
                vez_ducking: 0,
                trocando_pos_ducking: false,
                prox_id: 0,
                motor: None,
                iniciando_motor: false,
                erro_motor: None,
                vigia: None,
                link: None,
                ultima_vinheta: None,
                geracao_motor: 0,
                captura: None,
                geracao_captura: 0,
                transmissao_estado: "desligada".into(),
                erro_captura: None,
            }));
            app.manage(estado.clone());
            let handle = app.handle().clone();
            iniciar_motor(&handle, &estado, &mut travar(&estado));
            carregar_biblioteca(handle.clone(), estado.clone(), false);
            iniciar_relogio(handle.clone(), estado.clone());
            iniciar_relogio_agenda(handle.clone(), estado.clone());
            iniciar_captura(&handle, &estado, &mut travar(&estado));
            iniciar_limpeza_gravacoes(estado.clone());
            iniciar_gravador_indice(estado);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            estado,
            reiniciar_programa,
            fechar_programa,
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
            pasta_adicionar,
            pasta_remover,
            definir_tocar_ao_abrir,
            definir_abrir_tela_cheia,
            alternar_tela_cheia,
            tela_cheia_agora,
            senha_conferir,
            senha_definir,
            bloqueios_definir,
            agendamentos,
            agendamento_salvar,
            agendamento_remover,
            nome_radio_definir,
            definir_vinheta_entre_musicas,
            placas,
            audio_definir,
            gravacao_definir,
            transmissao_definir,
        ])
        .build(tauri::generate_context!())
        .expect("erro ao abrir o Radio On Air")
        .run(|app, evento| {
            // Fechou o programa: fecha junto o motor e a gravação/transmissão.
            if let tauri::RunEvent::Exit = evento {
                fechar_processos(&mut travar(&app.state::<Compartilhado>()));
            }
        });
}
