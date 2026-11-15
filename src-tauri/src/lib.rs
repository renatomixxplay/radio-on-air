mod analise;
mod biblioteca;
mod config;
mod motor;
mod ordem;

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

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Tipo {
    Musica,
    Programa,
    Vinheta,
}

#[derive(Serialize, Clone, Debug)]
pub struct Item {
    id: u64,
    caminho: String,
    artista: String,
    titulo: String,
    tipo: Tipo,
    duracao: Option<f64>,
    onda: Vec<u8>,
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
    prox_id: u64,
    motor: Option<Motor>,
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
    erro_motor: Option<String>,
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
            erro_motor: self.erro_motor.clone(),
        }
    }

    fn arquivo_indice(&self) -> PathBuf {
        self.dir.join("indice.txt")
    }

    fn novo_item(&mut self, caminho: String, artista: String, titulo: String, tipo: Tipo) -> Item {
        self.prox_id += 1;
        Item { id: self.prox_id, caminho, artista, titulo, tipo, duracao: None, onda: vec![], travado: false }
    }

    /// Entrega o primeiro da fila ao motor, se ainda não foi.
    fn enviar_proximo(&mut self) {
        let Some(motor) = &self.motor else { return };
        if let Some(prox) = self.fila.first_mut() {
            if !prox.travado {
                match motor.enfileirar(&prox.caminho) {
                    Ok(()) => prox.travado = true,
                    Err(e) => self.erro_motor = Some(e),
                }
            }
        }
    }
}

fn emitir(app: &AppHandle, e: &Estado) {
    let _ = app.emit("estado", e.retrato());
}

/// Analisa duração e forma de onda em segundo plano e atualiza o item.
fn analisar_em_segundo_plano(app: &AppHandle, estado: &Compartilhado, id: u64, caminho: String) {
    let app = app.clone();
    let estado = estado.clone();
    std::thread::spawn(move || {
        let a = analise::analisar(Path::new(&caminho)).unwrap_or_default();
        let mut e = travar(&estado);
        let Estado { fila, tocando, .. } = &mut *e;
        let alvo = fila
            .iter_mut()
            .find(|i| i.id == id)
            .or_else(|| tocando.as_mut().map(|t| &mut t.item).filter(|i| i.id == id));
        if let Some(item) = alvo {
            item.duracao = (a.duracao > 0.0).then_some(a.duracao);
            item.onda = a.onda;
        }
        emitir(&app, &e);
    });
}

/// Mantém o mínimo da fila puxando músicas aleatórias do acervo.
fn completar_fila(app: &AppHandle, estado: &Compartilhado, e: &mut Estado) {
    while e.fila.len() <= FILA_MINIMO && e.fila.len() < FILA_MAXIMO {
        let Some(f) = e.bib.aleatoria() else { break };
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
            let pos = e.fila.iter().position(|i| i.travado && i.caminho == caminho);
            let item = match pos {
                Some(p) => e.fila.remove(p),
                None => {
                    let (artista, titulo) = biblioteca::artista_e_musica(Path::new(&caminho));
                    let item = e.novo_item(caminho.clone(), artista, titulo, Tipo::Musica);
                    analisar_em_segundo_plano(app, estado, item.id, caminho);
                    item
                }
            };
            e.tocando = Some(Tocando { item, inicio: Instant::now(), pausa_desde: None, pausado_total: Duration::ZERO });
            completar_fila(app, estado, &mut e);
        }
        EventoMotor::Parou(erro) => {
            e.motor = None;
            e.no_ar = false;
            e.tocando = None;
            e.fila.iter_mut().for_each(|i| i.travado = false);
            let erro = erro.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").to_string();
            e.erro_motor = Some(if erro.is_empty() { "O motor de áudio parou.".into() } else { erro });
        }
    }
    emitir(app, &e);
}

fn iniciar_motor(app: &AppHandle, estado: &Compartilhado, e: &mut Estado) {
    if e.motor.is_some() {
        return;
    }
    let Some(exe) = motor::localizar() else {
        e.erro_motor = Some("Liquidsoap não encontrado. Instale o pacote liquidsoap.".into());
        return;
    };
    let (app2, estado2) = (app.clone(), estado.clone());
    match Motor::iniciar(&exe, &e.dir, move |ev| ao_evento_motor(&app2, &estado2, ev)) {
        Ok(m) => {
            e.motor = Some(m);
            e.erro_motor = None;
        }
        Err(err) => e.erro_motor = Some(err),
    }
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
        let pronto = e.fila.first().is_some_and(|i| !i.travado);
        if pronto && falta.is_some_and(|f| f <= ANTECEDENCIA) {
            e.enviar_proximo();
            emitir(&app, &e);
        }
    });
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
    if e.motor.is_none() {
        emitir(&app, &e);
        return;
    }
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
    e.enviar_proximo();
    if let Some(m) = &e.motor {
        let _ = m.pular();
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
                prox_id: 0,
                motor: None,
                erro_motor: None,
                vigia: None,
            }));
            app.manage(estado.clone());
            let handle = app.handle().clone();
            carregar_biblioteca(handle.clone(), estado.clone(), false);
            iniciar_relogio(handle, estado.clone());
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
        ])
        .run(tauri::generate_context!())
        .expect("erro ao abrir o Radio On Air");
}
