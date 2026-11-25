//! Pesquisa e baixa músicas do YouTube com o yt-dlp.
//!
//! O yt-dlp do Debian envelhece rápido (o YouTube muda e ele para de baixar).
//! Por isso o programa guarda a PRÓPRIA cópia, a oficial do projeto, na pasta
//! de dados, e pede a ela para se atualizar uma vez por dia.
//!
//! Só o áudio é baixado (o vídeo nem vem). O áudio cru vira MP3 com o volume
//! igualado e o silêncio das pontas cortado; depois é apagado.
//!
//! Para não baixar repetida, cada vídeo baixado é anotado em
//! `.youtube-baixados.txt` dentro da pasta do YouTube (`id<TAB>arquivo`).

use crate::{registro, sistema};
use deunicode::deunicode;
use serde::Serialize;
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::sync::mpsc;
use std::time::Duration;

/// Quantos vídeos a pesquisa pede ao YouTube.
pub const QUANTOS: usize = 50;
/// Vídeos mais longos que isso (shows, coletâneas) nem aparecem.
pub const DURACAO_MAXIMA: f64 = 30.0 * 60.0;
/// Volume do MP3 salvo.
const ALVO_LUFS: f64 = -14.0;
const BITRATE: &str = "128k";
/// Abaixo disso, nas pontas, é silêncio (cortado).
const SILENCIO_DB: i32 = -50;
const REGISTRO: &str = ".youtube-baixados.txt";
pub const UM_DIA: Duration = Duration::from_secs(24 * 3600);

/// Só um preparo/atualização do yt-dlp por vez.
static PREPARO: Mutex<()> = Mutex::new(());

/// Arquivo oficial do yt-dlp. No Linux/Mac, a versão leve (3 MB) que roda com
/// o Python do sistema: abre em ~1 s. A versão "completa" (35 MB) se
/// descompacta inteira a cada uso, e no Mac levava ~25 s só para abrir.
const ARQUIVO_OFICIAL: &str = if cfg!(windows) { "yt-dlp.exe" } else { "yt-dlp" };

fn pasta_ytdlp(dir: &Path) -> PathBuf {
    dir.join("yt-dlp")
}

fn exe(dir: &Path) -> PathBuf {
    pasta_ytdlp(dir).join(if cfg!(windows) { "yt-dlp.exe" } else { "yt-dlp" })
}

fn marca_verificado(dir: &Path) -> PathBuf {
    pasta_ytdlp(dir).join("verificado")
}

fn travar_preparo() -> std::sync::MutexGuard<'static, ()> {
    PREPARO.lock().unwrap_or_else(|p| p.into_inner())
}

/// Garante o yt-dlp na pasta de dados: na primeira vez, baixa do site oficial.
pub fn preparar(dir: &Path) -> Result<PathBuf, String> {
    let _vez = travar_preparo();
    let exe = exe(dir);
    if exe.is_file() {
        return Ok(exe);
    }
    fs::create_dir_all(pasta_ytdlp(dir)).map_err(|e| e.to_string())?;
    let url = format!("https://github.com/yt-dlp/yt-dlp/releases/latest/download/{ARQUIVO_OFICIAL}");
    let parcial = pasta_ytdlp(dir).join("yt-dlp.baixando");
    registro::anotar(format!("youtube: baixando o yt-dlp ({url})"));
    let ok = sistema::comando("curl")
        .args(["-fsSL", "--retry", "2", "--max-time", "300", "-o"])
        .arg(&parcial)
        .arg(&url)
        .stdin(Stdio::null())
        .status()
        .map_err(|_| "Falta o curl para baixar o yt-dlp.".to_string())?;
    if !ok.success() {
        let _ = fs::remove_file(&parcial);
        return Err("Não consegui baixar o yt-dlp. A internet está funcionando?".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&parcial, fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
    }
    fs::rename(&parcial, &exe).map_err(|e| e.to_string())?;
    let _ = fs::write(marca_verificado(dir), "");
    Ok(exe)
}

/// Pede ao yt-dlp para se atualizar, se a última conferência foi há mais de
/// `intervalo`. Sem internet, segue com a versão que tem.
pub fn atualizar(dir: &Path, intervalo: Duration) {
    let exe = match preparar(dir) {
        Ok(e) => e,
        Err(e) => {
            registro::anotar(format!("youtube: {e}"));
            return;
        }
    };
    let _vez = travar_preparo();
    let idade = fs::metadata(marca_verificado(dir)).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok());
    if idade.is_some_and(|t| t < intervalo) {
        return;
    }
    match sistema::comando(&exe).args(["-U", "--no-color"]).stdin(Stdio::null()).output() {
        Ok(s) => {
            let txt = String::from_utf8_lossy(&s.stdout);
            let ultima = txt.lines().filter(|l| !l.trim().is_empty()).last().unwrap_or("").trim().to_string();
            registro::anotar(format!("youtube: atualização do yt-dlp: {ultima}"));
            // Só marca como conferido se deu certo: sem internet, tenta de novo depois.
            if s.status.success() {
                let _ = fs::write(marca_verificado(dir), "");
            }
        }
        Err(e) => registro::anotar(format!("youtube: yt-dlp não abriu para atualizar: {e}")),
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct Video {
    pub id: String,
    /// Título como está no YouTube.
    pub titulo: String,
    pub canal: String,
    pub duracao: f64,
    /// Nome com que vai ser salvo (sem o `.mp3`): "Artista - Música".
    pub nome: String,
    /// Já está no computador: o arquivo (não precisa baixar).
    pub caminho: Option<String>,
}

/// Pesquisa por texto no YouTube (como a caixa de busca do site).
pub fn pesquisar(dir: &Path, termo: &str) -> Result<Vec<Video>, String> {
    let termo = termo.trim();
    if termo.is_empty() {
        return Ok(vec![]);
    }
    let exe = preparar(dir)?;
    let mut videos = vec![];
    // Às vezes o YouTube responde sem o nome dos canais: pergunta de novo uma vez.
    for _ in 0..2 {
        let saida = sistema::comando(&exe)
            .args(["--flat-playlist", "-J", "--no-warnings", "--no-update", "--no-color"])
            .arg(format!("ytsearch{QUANTOS}:{termo}"))
            .stdin(Stdio::null())
            .output()
            .map_err(|e| format!("O yt-dlp não abriu: {e}"))?;
        if !saida.status.success() {
            return Err(erro_do_ytdlp(&String::from_utf8_lossy(&saida.stderr)));
        }
        let json: serde_json::Value =
            serde_json::from_slice(&saida.stdout).map_err(|_| "O YouTube respondeu algo estranho.".to_string())?;
        let entradas = json["entries"].as_array().map(Vec::as_slice).unwrap_or_default();
        videos = entradas.iter().filter_map(video_da_busca).collect();
        let sem_canal = videos.iter().filter(|v| v.canal.is_empty()).count();
        if sem_canal * 2 <= videos.len() {
            break;
        }
    }
    Ok(videos)
}

fn video_da_busca(e: &serde_json::Value) -> Option<Video> {
    let id = e["id"].as_str().filter(|i| id_valido(i))?.to_string();
    // Sem duração = transmissão ao vivo.
    let duracao = e["duration"].as_f64().filter(|d| *d > 0.0 && *d <= DURACAO_MAXIMA)?;
    let titulo = e["title"].as_str()?.trim().to_string();
    let canal = e["channel"].as_str().or(e["uploader"].as_str()).unwrap_or("").trim().to_string();
    let nome = nome_do_arquivo(&titulo, &canal);
    Some(Video { id, titulo, canal, duracao, nome, caminho: None })
}

/// Id de vídeo do YouTube: letras, números, `-` e `_`.
pub fn id_valido(id: &str) -> bool {
    !id.is_empty() && id.len() <= 20 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// A linha `ERROR:` do yt-dlp, mais legível.
fn erro_do_ytdlp(stderr: &str) -> String {
    let linha = stderr.lines().filter(|l| l.contains("ERROR:")).last().unwrap_or("").trim();
    let motivo = linha.split_once("ERROR:").map(|(_, m)| m.trim()).unwrap_or("");
    if motivo.is_empty() {
        "O YouTube não respondeu. A internet está funcionando?".into()
    } else {
        format!("YouTube: {motivo}")
    }
}

/// Músicas já baixadas na pasta do YouTube, em ordem alfabética.
pub fn musicas_da_pasta(pasta: &Path) -> Vec<crate::biblioteca::FaixaDto> {
    let mut arquivos: Vec<PathBuf> = fs::read_dir(pasta)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_file() && crate::biblioteca::eh_audio(p))
        .collect();
    let nome = |p: &Path| p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    arquivos.sort_by(|a, b| crate::ordem::natural(&nome(a), &nome(b)));
    arquivos
        .into_iter()
        .map(|p| {
            let (artista, musica) = crate::biblioteca::artista_e_musica(&p);
            crate::biblioteca::FaixaDto { caminho: p.to_string_lossy().into_owned(), artista, musica }
        })
        .collect()
}

// ─── Repetidas ───────────────────────────────────────────────────────────────

/// Vídeos já baixados nesta pasta: id → arquivo (só os que ainda existem).
pub fn baixados(pasta: &Path) -> HashMap<String, PathBuf> {
    let Ok(txt) = fs::read_to_string(pasta.join(REGISTRO)) else {
        return HashMap::new();
    };
    txt.lines()
        .filter_map(|l| l.split_once('\t'))
        .map(|(id, arq)| (id.trim().to_string(), pasta.join(arq.trim())))
        .filter(|(_, p)| p.is_file())
        .collect()
}

fn anotar_baixado(pasta: &Path, id: &str, arquivo: &Path) {
    let Some(nome) = arquivo.file_name() else { return };
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(pasta.join(REGISTRO)) {
        let _ = writeln!(f, "{id}\t{}", nome.to_string_lossy());
    }
}

// ─── Baixar e converter ──────────────────────────────────────────────────────

/// Baixa o áudio do vídeo, converte e salva `<pasta>/<Artista - Música>.mp3`.
/// O nome sai do título e do canal (o canal vem do próprio vídeo; o da
/// pesquisa é reserva). Se outro vídeo já usou esse nome, este ganha um
/// número: "Artista - Música (2).mp3". `progresso` recebe de 0 a 100 enquanto baixa.
pub fn baixar(
    dir: &Path,
    pasta: &Path,
    id: &str,
    titulo: &str,
    canal: &str,
    progresso: impl Fn(u8),
) -> Result<PathBuf, String> {
    if !id_valido(id) {
        return Err("Vídeo inválido.".into());
    }
    let exe = preparar(dir)?;
    fs::create_dir_all(pasta).map_err(|e| format!("Não consegui usar a pasta do YouTube: {e}"))?;
    let tmp = dir.join("youtube-tmp");
    fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let url = format!("https://www.youtube.com/watch?v={id}");
    registro::anotar(format!("youtube: baixando {id} ({titulo})"));

    // Primeiro com os clientes que o yt-dlp escolhe; se o YouTube recusar,
    // tenta pelo "web_embedded" (que já salvou downloads com erro 403).
    let tentativas: [&[&str]; 2] = [&[], &["--extractor-args", "youtube:player_client=web_embedded,default"]];
    let mut erro = String::new();
    let mut cru = None;
    for extra in tentativas {
        match baixar_audio(&exe, extra, &tmp, id, &url, &progresso) {
            Ok(p) => {
                cru = Some(p);
                break;
            }
            Err(e) => {
                registro::anotar(format!("youtube: {id} falhou: {e}"));
                erro = e;
            }
        }
    }
    let (cru, canal_do_video) = cru.ok_or(erro)?;
    let nome = nome_do_arquivo(titulo, canal_do_video.as_deref().unwrap_or(canal));
    let destino = nome_livre(pasta, &nome);

    let pronto = tmp.join(format!("{id}.mp3"));
    let convertido = converter(&cru, &pronto, &nome);
    let _ = fs::remove_file(&cru);
    if let Err(e) = convertido {
        let _ = fs::remove_file(&pronto);
        return Err(e);
    }
    // Só entra na pasta pronto (o vigia do acervo não vê arquivo pela metade).
    mover(&pronto, &destino).map_err(|e| format!("Não consegui salvar na pasta do YouTube: {e}"))?;
    anotar_baixado(pasta, id, &destino);
    registro::anotar(format!("youtube: pronto {}", destino.display()));
    Ok(destino)
}

/// `<pasta>/<nome>.mp3`; se já existir, `<nome> (2).mp3`, `(3)`…
fn nome_livre(pasta: &Path, nome: &str) -> PathBuf {
    (1..)
        .map(|n| if n == 1 { pasta.join(format!("{nome}.mp3")) } else { pasta.join(format!("{nome} ({n}).mp3")) })
        .find(|p| !p.exists())
        .expect("sempre acha um nome livre")
}

/// Renomeia; se a pasta for em outro disco, copia e apaga.
fn mover(de: &Path, para: &Path) -> std::io::Result<()> {
    if fs::rename(de, para).is_ok() {
        return Ok(());
    }
    let parcial = para.with_extension("mp3.parcial");
    fs::copy(de, &parcial)?;
    fs::rename(&parcial, para)?;
    let _ = fs::remove_file(de);
    Ok(())
}

/// Baixa só o áudio, como vem do YouTube. Devolve o arquivo cru e o canal.
fn baixar_audio(
    exe: &Path,
    extra: &[&str],
    tmp: &Path,
    id: &str,
    url: &str,
    progresso: &impl Fn(u8),
) -> Result<(PathBuf, Option<String>), String> {
    let mut filho = sistema::comando(exe)
        .args(["-f", "bestaudio/best", "--no-playlist", "--no-update", "--no-color", "--newline", "--progress"])
        .args(["--js-runtimes", "node"])
        .args(extra)
        .args(["--progress-template", "download:PROGRESSO %(progress._percent_str)s"])
        .args(["--print", "after_move:CANAL %(channel)s", "--print", "after_move:ARQUIVO %(filepath)s", "-o"])
        .arg(tmp.join(format!("{id}.cru.%(ext)s")))
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("O yt-dlp não abriu: {e}"))?;

    // As duas saídas chegam aqui, linha a linha.
    let (tx, rx) = mpsc::channel::<String>();
    let mut leitores = vec![];
    if let Some(out) = filho.stdout.take() {
        let tx = tx.clone();
        leitores.push(std::thread::spawn(move || BufReader::new(out).lines().map_while(Result::ok).for_each(|l| drop(tx.send(l)))));
    }
    if let Some(err) = filho.stderr.take() {
        let tx = tx.clone();
        leitores.push(std::thread::spawn(move || BufReader::new(err).lines().map_while(Result::ok).for_each(|l| drop(tx.send(l)))));
    }
    drop(tx);

    let mut arquivo = None;
    let mut canal = None;
    let mut erros = String::new();
    let mut ultimo = None;
    for linha in rx {
        if let Some(caminho) = linha.strip_prefix("ARQUIVO ") {
            arquivo = Some(PathBuf::from(caminho.trim()));
        } else if let Some(c) = linha.strip_prefix("CANAL ") {
            canal = Some(c.trim().to_string()).filter(|c| !c.is_empty() && c != "NA");
        } else if let Some(pct) = linha.strip_prefix("PROGRESSO ") {
            if let Ok(p) = pct.trim().trim_end_matches('%').trim().parse::<f64>() {
                let p = p.clamp(0.0, 100.0) as u8;
                if ultimo != Some(p) {
                    ultimo = Some(p);
                    progresso(p);
                }
            }
        } else if linha.contains("ERROR:") {
            erros.push_str(&linha);
            erros.push('\n');
        }
    }
    leitores.into_iter().for_each(|l| drop(l.join()));
    let status = filho.wait().map_err(|e| e.to_string())?;
    match arquivo.filter(|p| p.is_file()) {
        Some(p) if status.success() => Ok((p, canal)),
        _ => Err(erro_do_ytdlp(&erros)),
    }
}

/// Áudio cru → MP3 estéreo 44,1 kHz com o volume igualado em `ALVO_LUFS` e o
/// silêncio das pontas cortado. Duas passadas: a 1ª mede a música inteira
/// (volume e silêncios); a 2ª corta, aplica o ganho exato e segura os picos.
fn converter(cru: &Path, saida: &Path, nome: &str) -> Result<(), String> {
    let medida = ffmpeg(
        Command::new("ffmpeg")
            .args(["-hide_banner", "-nostdin", "-i"])
            .arg(cru)
            .args(["-vn", "-af"])
            .arg(format!("silencedetect=noise={SILENCIO_DB}dB:d=0.1,ebur128=framelog=quiet"))
            .args(["-f", "null", "-"]),
    )?;
    let volume = loudness(&medida).ok_or("Não consegui medir o volume da música.")?;
    let ganho = (ALVO_LUFS - volume).clamp(-20.0, 20.0);
    let (inicio, fim) = pontas(&medida);
    let corte = match fim {
        Some(f) if f > inicio + 1.0 => format!("atrim=start={inicio:.3}:end={f:.3},asetpts=PTS-STARTPTS,"),
        _ if inicio > 0.0 => format!("atrim=start={inicio:.3},asetpts=PTS-STARTPTS,"),
        _ => String::new(),
    };
    // Limitador só para os picos que o ganho empurrar acima de -1 dBFS.
    let filtro = format!("{corte}volume={ganho:.2}dB,alimiter=limit=0.89:attack=5:release=50:level=false");

    let (artista, musica) = nome.split_once(" - ").unwrap_or(("", nome));
    let mut cmd = Command::new("ffmpeg");
    cmd.args(["-y", "-hide_banner", "-nostdin", "-i"])
        .arg(cru)
        .args(["-vn", "-map_metadata", "-1", "-af", &filtro])
        .args(["-ar", "44100", "-ac", "2", "-c:a", "libmp3lame", "-b:a", BITRATE, "-id3v2_version", "3"])
        .args(["-metadata", &format!("title={musica}")]);
    if !artista.is_empty() {
        cmd.args(["-metadata", &format!("artist={artista}")]);
    }
    cmd.args(["-f", "mp3"]).arg(saida);
    ffmpeg(&mut cmd)?;
    Ok(())
}

/// Roda o ffmpeg escondido; devolve o que ele escreveu no stderr.
fn ffmpeg(cmd: &mut Command) -> Result<String, String> {
    let mut c = sistema::comando(cmd.get_program());
    c.args(cmd.get_args());
    let saida = c.stdin(Stdio::null()).output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            "Falta o ffmpeg no computador.".to_string()
        } else {
            format!("O ffmpeg não abriu: {e}")
        }
    })?;
    let stderr = String::from_utf8_lossy(&saida.stderr).into_owned();
    if saida.status.success() {
        Ok(stderr)
    } else {
        let fim: String = stderr.lines().rev().take(3).collect::<Vec<_>>().join(" | ");
        Err(format!("Não consegui converter a música ({fim})."))
    }
}

/// O volume da música inteira (LUFS integrado) do resumo do `ebur128`.
fn loudness(stderr: &str) -> Option<f64> {
    let resumo = &stderr[stderr.rfind("Summary:")?..];
    let linha = resumo.lines().find(|l| l.trim_start().starts_with("I:"))?;
    linha.trim_start().trim_start_matches("I:").trim().split_whitespace().next()?.parse().ok()
}

/// Onde a música começa e termina, pelo `silencedetect`: o silêncio que começa
/// no zero é pulado; o que vai até o fim do arquivo é cortado.
fn pontas(stderr: &str) -> (f64, Option<f64>) {
    let num = |linha: &str, chave: &str| -> Option<f64> {
        let resto = linha.split_once(chave)?.1.trim_start();
        resto.split(|c: char| c.is_whitespace() || c == '|').next()?.parse().ok()
    };
    // (início, fim) de cada silêncio; fim `None` = foi até o fim do arquivo.
    let mut silencios: Vec<(f64, Option<f64>)> = vec![];
    let mut duracao = None;
    for linha in stderr.lines() {
        if let Some(s) = num(linha, "silence_start:") {
            silencios.push((s, None));
        } else if let Some(e) = num(linha, "silence_end:") {
            if let Some(ultimo) = silencios.last_mut() {
                ultimo.1 = Some(e);
            }
        } else if duracao.is_none() && linha.trim_start().starts_with("Duration:") {
            duracao = linha.split_once("Duration:").and_then(|(_, r)| r.split(',').next()).and_then(segundos);
        }
    }
    let inicio = match silencios.first() {
        Some((s, Some(e))) if *s <= 0.05 => *e,
        _ => 0.0,
    };
    let fim = silencios.last().and_then(|(s, e)| match (e, duracao) {
        (None, _) => Some(*s),
        (Some(e), Some(d)) if *e >= d - 0.25 => Some(*s),
        _ => None,
    });
    (inicio, fim.filter(|f| *f > inicio))
}

/// "00:03:01.25" → 181,25.
fn segundos(t: &str) -> Option<f64> {
    let mut total = 0.0;
    for parte in t.trim().split(':') {
        total = total * 60.0 + parte.parse::<f64>().ok()?;
    }
    Some(total)
}

// ─── Nome do arquivo ─────────────────────────────────────────────────────────

/// Enfeites de título de vídeo que não fazem parte do nome da música.
const ENFEITES: &[&str] = &[
    "clipe", "videoclipe", "videoclip", "clip", "video", "oficial", "official", "audio", "lyric", "lyrics",
    "letra", "legendado", "legenda", "hd", "4k", "hq", "visualizer", "ao vivo", "live", "dvd", "lancamento",
    "musica nova", "remastered", "remasterizado", "remaster",
];

/// Palavras do texto, minúsculas e sem acento, separadas por um espaço
/// (com espaço nas pontas, para achar palavra inteira).
fn palavras(texto: &str) -> String {
    let t = deunicode(texto).to_lowercase();
    let v: Vec<&str> = t.split(|c: char| !c.is_alphanumeric()).filter(|p| !p.is_empty()).collect();
    format!(" {} ", v.join(" "))
}

fn eh_enfeite(texto: &str) -> bool {
    let p = palavras(texto);
    ENFEITES.iter().any(|e| p.contains(&format!(" {e} ")))
}

/// Tira os parênteses/colchetes de enfeite: "(Clipe Oficial)", "[HD]".
fn sem_enfeites_entre_parenteses(t: &str) -> String {
    let pares = [('(', ')'), ('[', ']'), ('{', '}'), ('【', '】')];
    let mut saida = String::new();
    let mut resto = t;
    while let Some((ini, abre)) = resto.char_indices().find(|(_, c)| pares.iter().any(|(a, _)| a == c)) {
        let fecha = pares.iter().find(|(a, _)| *a == abre).map(|(_, f)| *f).unwrap_or(')');
        let depois = &resto[ini + abre.len_utf8()..];
        let Some(fim) = depois.find(fecha) else { break };
        let dentro = &depois[..fim];
        saida.push_str(&resto[..ini]);
        if !eh_enfeite(dentro) {
            saida.push(abre);
            saida.push_str(dentro);
            saida.push(fecha);
        }
        resto = &depois[fim + fecha.len_utf8()..];
    }
    saida.push_str(resto);
    saida
}

/// Nome do canal sem "Oficial", "VEVO", "- Topic".
fn artista_do_canal(canal: &str) -> String {
    let c = canal.trim().trim_end_matches(" - Topic").trim_end_matches("VEVO");
    c.split_whitespace().filter(|p| !matches!(palavras(p).trim(), "oficial" | "official" | "vevo")).collect::<Vec<_>>().join(" ")
}

/// Toda palavra começa com maiúscula. Pedaço todo em CAIXA ALTA (com alguma
/// palavra de 4 letras ou mais) vira minúsculo antes; senão o resto da palavra
/// fica como está ("DJ Alok", "MC Kevin", "AC/DC").
fn maiusculas(t: &str) -> String {
    let caixa_alta = !t.chars().any(char::is_lowercase)
        && t.split(|c: char| !c.is_alphabetic()).any(|p| p.chars().count() >= 4);
    t.split_whitespace()
        .map(|p| {
            let p = if caixa_alta { p.to_lowercase() } else { p.to_string() };
            let mut feito = false;
            p.chars()
                .map(|c| {
                    if !feito && c.is_alphanumeric() {
                        feito = true;
                        c.to_uppercase().collect::<String>()
                    } else {
                        c.to_string()
                    }
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Sem os caracteres que não podem ir em nome de arquivo.
fn seguro(t: &str) -> String {
    let limpo: String = t.chars().map(|c| if "/\\:*?\"<>|".contains(c) || c.is_control() { ' ' } else { c }).collect();
    let limpo = limpo.split_whitespace().collect::<Vec<_>>().join(" ");
    let limpo: String = limpo.chars().take(150).collect();
    limpo.trim_matches(|c: char| c == '.' || c.is_whitespace()).to_string()
}

/// O texto começa com o nome do canal ("Érlon Péricles e Elton Saldanha"
/// começa com o canal "Érlon Péricles").
fn comeca_com(texto: &str, canal: &str) -> bool {
    let c = palavras(canal);
    c.trim().len() >= 3 && palavras(texto).starts_with(&c)
}

/// Título do YouTube → "Artista - Música" (sem o `.mp3`).
/// Ex.: "ALBERI SILVA - SOU FEIO MAIS SOU FELIZ (Clipe Oficial) [HD]"
///   → "Alberi Silva - Sou Feio Mais Sou Feliz".
pub fn nome_do_arquivo(titulo: &str, canal: &str) -> String {
    // Símbolos de enfeite (♪, ★, emoji) saem; travessões viram hífen.
    let mut t: String = titulo
        .replace(['–', '—', '‒'], "-")
        .chars()
        .map(|c| if c.is_alphanumeric() || c.is_whitespace() || "-&'.,()[]{}!?+/:|\"#“”‘’【】$%@_".contains(c) { c } else { ' ' })
        .collect();
    // Sem " - ", a barra separa a música do artista: "Sou Um Milagre | Eliana Ribeiro".
    if !t.contains(" - ") {
        if let Some((m, a)) = t.split_once(" | ") {
            let a = a.split(" | ").next().unwrap_or(a);
            if !eh_enfeite(a) {
                t = format!("{} - {}", a.trim(), m.trim());
            }
        }
    }
    // Depois de " | " ou " / " vem comentário ("/ letra / legendado /").
    for sep in [" | ", " / ", " // ", "｜"] {
        if let Some(i) = t.find(sep) {
            t.truncate(i);
        }
    }
    let t: String = t.split_whitespace().filter(|p| !p.starts_with('#')).collect::<Vec<_>>().join(" ");
    let t = sem_enfeites_entre_parenteses(&t);
    let tira_aspas = |s: &str| s.trim().trim_matches(|c| "\"'“”‘’".contains(c)).trim().to_string();

    let canal = artista_do_canal(canal);
    let mut partes: Vec<String> = t.split(" - ").map(tira_aspas).filter(|p| !p.is_empty()).collect();
    // "Música - Artista": se o 2º pedaço é o canal, troca a ordem.
    if partes.len() >= 2 && comeca_com(&partes[1], &canal) && !comeca_com(&partes[0], &canal) {
        partes.swap(0, 1);
    }
    let (artista, musica) = match partes.as_slice() {
        [] => (canal, tira_aspas(titulo)),
        [so] => (canal, so.clone()),
        [a, resto @ ..] => {
            // "Artista - Música - Ao Vivo em Goiânia": o pedaço de enfeite sai.
            let m: Vec<&str> = resto.iter().filter(|p| !eh_enfeite(p)).map(String::as_str).collect();
            let m = if m.is_empty() { resto[0].clone() } else { m.join(" - ") };
            (a.clone(), m)
        }
    };
    let artista = seguro(&maiusculas(&artista));
    let musica = seguro(&maiusculas(&musica));
    match (artista.is_empty(), musica.is_empty()) {
        (_, true) => seguro(&maiusculas(titulo)),
        (true, false) => musica,
        _ => format!("{artista} - {musica}"),
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn nomes_do_youtube() {
        let casos = [
            ("Gusttavo Lima - Bloqueado (Buteco In Boston)", "Gusttavo Lima Oficial", "Gusttavo Lima - Bloqueado (Buteco In Boston)"),
            ("Gusttavo Lima - Bloqueado #Embaixador15Anos", "Gusttavo Lima Oficial", "Gusttavo Lima - Bloqueado"),
            ("Gusttavo Lima - Bloqueado / letra / legendado /", "André", "Gusttavo Lima - Bloqueado"),
            ("ALBERI SILVA - SOU FEIO MAIS SOU FELIZ (Clipe Oficial) [HD]", "x", "Alberi Silva - Sou Feio Mais Sou Feliz"),
            ("alberi silva - sou feio mais sou feliz", "x", "Alberi Silva - Sou Feio Mais Sou Feliz"),
            ("Teixeirinha – Coração de Luto (Áudio Oficial)", "x", "Teixeirinha - Coração De Luto"),
            ("Bloqueado", "Gusttavo Lima Oficial", "Gusttavo Lima - Bloqueado"),
            ("Bloqueado", "Gusttavo Lima - Topic", "Gusttavo Lima - Bloqueado"),
            ("Jorge & Mateus - Amo Noite E Dia - Ao Vivo Em Goiânia", "x", "Jorge & Mateus - Amo Noite E Dia"),
            ("AC/DC - Back In Black (Official Video)", "x", "AC DC - Back In Black"),
            ("Hoje Eu Sei Quem Sou ( Ao vivo ) - Yana Araújo - DVD Essência", "Yana Araújo", "Yana Araújo - Hoje Eu Sei Quem Sou"),
            ("Sou um milagre | Eliana Ribeiro", "Lancee Digital", "Eliana Ribeiro - Sou Um Milagre"),
            ("AMIGOS DO TEMPO ANTIGO - Érlon Péricles e Elton Saldanha", "Érlon Péricles", "Érlon Péricles E Elton Saldanha - Amigos Do Tempo Antigo"),
            ("BLOQUEADO - GUSTTAVO LIMA (MÚSICA OFICIAL)", "Gusttavo Lima Oficial", "Gusttavo Lima - Bloqueado"),
            ("MC Kevin - Sou Favela (DJ Nenê)", "Mc Kevin Oficial", "MC Kevin - Sou Favela (DJ Nenê)"),
            ("DJ Alok - Hear Me Now [Official Lyric Video]", "x", "DJ Alok - Hear Me Now"),
            ("Artista - \"Música\" (Live)", "x", "Artista - Música"),
        ];
        for (titulo, canal, esperado) in casos {
            assert_eq!(nome_do_arquivo(titulo, canal), esperado, "título: {titulo}");
        }
    }

    #[test]
    fn pontas_do_silencio() {
        let stderr = "  Duration: 00:03:10.00, start: 0.000000, bitrate: 128 kb/s\n\
            [silencedetect @ 0x1] silence_start: 0\n\
            [silencedetect @ 0x1] silence_end: 1.5 | silence_duration: 1.5\n\
            [silencedetect @ 0x1] silence_start: 60.2\n\
            [silencedetect @ 0x1] silence_end: 60.4 | silence_duration: 0.2\n\
            [silencedetect @ 0x1] silence_start: 185.3\n\
            [silencedetect @ 0x1] silence_end: 190 | silence_duration: 4.7\n";
        assert_eq!(pontas(stderr), (1.5, Some(185.3)));
        // Sem silêncio nas pontas: não corta nada.
        let s2 = "  Duration: 00:03:10.00, start\n[silencedetect @ 0x1] silence_start: 60.2\n[silencedetect @ 0x1] silence_end: 60.4 | x\n";
        assert_eq!(pontas(s2), (0.0, None));
        // Silêncio do fim sem "silence_end".
        let s3 = "[silencedetect @ 0x1] silence_start: 170.5\n";
        assert_eq!(pontas(s3), (0.0, Some(170.5)));
    }

    #[test]
    fn ids() {
        assert!(id_valido("dQw4w9WgXcQ"));
        assert!(id_valido("a-b_c"));
        assert!(!id_valido("../etc"));
        assert!(!id_valido(""));
    }
}
