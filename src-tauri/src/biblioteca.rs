//! Acervo de músicas: lê só os NOMES dos arquivos (padrão `Artista - Música.mp3`),
//! guarda a lista em `indice.txt` e faz a busca em memória.

use deunicode::deunicode;
use rand::seq::IndexedRandom;
use serde::Serialize;
use std::fs;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub const EXTENSOES: &[&str] = &["mp3", "ogg", "oga", "opus", "flac", "wav", "m4a", "aac"];

pub fn eh_audio(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .map(|e| EXTENSOES.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

/// Separa `Artista - Música` a partir do nome do arquivo.
/// Fora do padrão: o nome inteiro vira a música e o artista fica vazio.
pub fn artista_e_musica(p: &Path) -> (String, String) {
    let nome = p.file_stem().and_then(|s| s.to_str()).unwrap_or("").trim();
    match nome.split_once(" - ") {
        Some((a, m)) => (a.trim().to_string(), m.trim().to_string()),
        None => (String::new(), nome.to_string()),
    }
}

/// Texto normalizado para busca: minúsculo e sem acentos ("Chão" -> "chao").
fn chave(texto: &str) -> String {
    deunicode(texto).to_lowercase()
}

pub struct Faixa {
    /// Caminho relativo à pasta de músicas.
    pub rel: String,
    pub artista: String,
    pub musica: String,
    chave: String,
}

impl Faixa {
    fn nova(rel: String) -> Self {
        let (artista, musica) = artista_e_musica(Path::new(&rel));
        let chave = chave(&format!("{artista} {musica}"));
        Self { rel, artista, musica, chave }
    }
}

#[derive(Serialize, Clone)]
pub struct FaixaDto {
    pub caminho: String,
    pub artista: String,
    pub musica: String,
}

#[derive(Default)]
pub struct Biblioteca {
    pub raiz: Option<PathBuf>,
    faixas: Vec<Faixa>,
    /// Há mudanças ainda não gravadas no `indice.txt`.
    pub sujo: bool,
}

impl Biblioteca {
    pub fn total(&self) -> usize {
        self.faixas.len()
    }

    pub fn carregar_indice(raiz: PathBuf, arquivo: &Path) -> Option<Self> {
        let f = fs::File::open(arquivo).ok()?;
        let mut linhas = BufReader::new(f).lines();
        // Primeira linha guarda a pasta raiz; se mudou, o índice não serve.
        let cabecalho = linhas.next()?.ok()?;
        if cabecalho != format!("#raiz {}", raiz.display()) {
            return None;
        }
        let faixas = linhas
            .map_while(Result::ok)
            .filter(|l| !l.is_empty())
            .map(Faixa::nova)
            .collect();
        Some(Self { raiz: Some(raiz), faixas, sujo: false })
    }

    /// Varre a pasta inteira (usado na primeira vez e no botão "Atualizar músicas").
    pub fn varrer(raiz: PathBuf) -> Self {
        let mut faixas: Vec<Faixa> = WalkDir::new(&raiz)
            .follow_links(true)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file() && eh_audio(e.path()))
            .filter_map(|e| {
                e.path()
                    .strip_prefix(&raiz)
                    .ok()
                    .and_then(|p| p.to_str())
                    .map(|s| Faixa::nova(s.to_string()))
            })
            .collect();
        faixas.sort_by(|a, b| a.rel.cmp(&b.rel));
        Self { raiz: Some(raiz), faixas, sujo: true }
    }

    pub fn salvar_indice(&mut self, arquivo: &Path) -> std::io::Result<()> {
        let Some(raiz) = &self.raiz else { return Ok(()) };
        if let Some(dir) = arquivo.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp = arquivo.with_extension("tmp");
        {
            let mut w = BufWriter::new(fs::File::create(&tmp)?);
            writeln!(w, "#raiz {}", raiz.display())?;
            for f in &self.faixas {
                writeln!(w, "{}", f.rel)?;
            }
            w.flush()?;
        }
        fs::rename(tmp, arquivo)?;
        self.sujo = false;
        Ok(())
    }

    /// Busca por música ou artista. Todas as palavras digitadas precisam aparecer.
    pub fn buscar(&self, termo: &str, limite: usize) -> Vec<FaixaDto> {
        let palavras: Vec<String> = chave(termo).split_whitespace().map(String::from).collect();
        self.faixas
            .iter()
            .filter(|f| palavras.iter().all(|p| f.chave.contains(p.as_str())))
            .take(limite)
            .map(|f| self.dto(f))
            .collect()
    }

    pub fn aleatoria(&self) -> Option<FaixaDto> {
        self.faixas.choose(&mut rand::rng()).map(|f| self.dto(f))
    }

    /// Música aleatória de uma pasta (e subpastas). Dentro do acervo, sai do
    /// índice; fora dele, lê a pasta no disco. Pasta vazia: `None`.
    pub fn aleatoria_em(&self, pasta: &Path) -> Option<FaixaDto> {
        let Some(rel) = self.raiz.as_ref().and_then(|r| pasta.strip_prefix(r).ok()) else {
            return aleatoria_no_disco(pasta);
        };
        if rel.as_os_str().is_empty() {
            return self.aleatoria();
        }
        // Compara por partes do caminho: no Windows a barra é "\", não "/".
        let dentro: Vec<&Faixa> = self.faixas.iter().filter(|f| Path::new(&f.rel).starts_with(rel)).collect();
        dentro.choose(&mut rand::rng()).map(|f| self.dto(f))
    }

    fn dto(&self, f: &Faixa) -> FaixaDto {
        let raiz = self.raiz.as_deref().unwrap_or(Path::new(""));
        FaixaDto {
            caminho: raiz.join(&f.rel).to_string_lossy().into_owned(),
            artista: f.artista.clone(),
            musica: f.musica.clone(),
        }
    }

    /// Atualiza o índice a partir de um caminho que mudou no disco
    /// (arquivo ou pasta criado, apagado ou renomeado).
    pub fn atualizar_caminho(&mut self, abs: &Path) {
        let Some(raiz) = self.raiz.clone() else { return };
        let Some(rel) = abs.strip_prefix(&raiz).ok().and_then(|p| p.to_str()).map(String::from) else {
            return;
        };
        if rel.is_empty() {
            return;
        }
        // Remove o que havia nesse caminho (o próprio arquivo ou tudo dentro da pasta).
        let antes = self.faixas.len();
        self.faixas.retain(|f| !Path::new(&f.rel).starts_with(&rel));
        let mut mudou = self.faixas.len() != antes;

        // Adiciona de novo o que existe agora.
        if abs.exists() {
            let novos: Vec<Faixa> = WalkDir::new(abs)
                .follow_links(true)
                .into_iter()
                .filter_map(Result::ok)
                .filter(|e| e.file_type().is_file() && eh_audio(e.path()))
                .filter_map(|e| e.path().strip_prefix(&raiz).ok()?.to_str().map(String::from))
                .map(Faixa::nova)
                .collect();
            if !novos.is_empty() {
                mudou = true;
                self.faixas.extend(novos);
                self.faixas.sort_by(|a, b| a.rel.cmp(&b.rel));
            }
        }
        if mudou {
            self.sujo = true;
        }
    }
}

/// Música aleatória de uma pasta fora do acervo, lida direto do disco.
fn aleatoria_no_disco(pasta: &Path) -> Option<FaixaDto> {
    let arquivos: Vec<PathBuf> = WalkDir::new(pasta)
        .follow_links(true)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file() && eh_audio(e.path()))
        .map(|e| e.into_path())
        .collect();
    let p = arquivos.choose(&mut rand::rng())?;
    let (artista, musica) = artista_e_musica(p);
    Some(FaixaDto { caminho: p.to_string_lossy().into_owned(), artista, musica })
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn separa_artista_e_musica() {
        let (a, m) = artista_e_musica(Path::new("Gaúchas/Renato - Fogo de Chão.mp3"));
        assert_eq!(a, "Renato");
        assert_eq!(m, "Fogo de Chão");
    }

    #[test]
    fn fora_do_padrao_vira_musica() {
        let (a, m) = artista_e_musica(Path::new("fogo de chao.mp3"));
        assert_eq!(a, "");
        assert_eq!(m, "fogo de chao");
    }

    #[test]
    fn busca_ignora_acento_e_maiuscula() {
        let b = Biblioteca {
            raiz: Some(PathBuf::from("/m")),
            faixas: vec![
                Faixa::nova("A/Renato - Fogo de Chão.mp3".into()),
                Faixa::nova("B/Maria - Rio.mp3".into()),
            ],
            sujo: false,
        };
        assert_eq!(b.buscar("fogo chao", 10).len(), 1);
        assert_eq!(b.buscar("RENATO", 10).len(), 1);
        // Compara como caminho: no Windows a junção usa "\\" ("/m\\B/Maria - Rio.mp3").
        assert_eq!(PathBuf::from(&b.buscar("maria rio", 10)[0].caminho), Path::new("/m").join("B/Maria - Rio.mp3"));
        assert_eq!(b.buscar("xyz", 10).len(), 0);
    }

    /// Agendamento de lista: só sai música de dentro da pasta escolhida,
    /// com a barra do sistema (no Windows o índice guarda "Gauchas\\x.mp3").
    #[test]
    fn sorteia_so_dentro_da_pasta() {
        let raiz = PathBuf::from("/m");
        let gauchas = Path::new("Gauchas").join("Renato - Fogo.mp3");
        let b = Biblioteca {
            raiz: Some(raiz.clone()),
            faixas: vec![
                Faixa::nova(gauchas.to_string_lossy().into_owned()),
                Faixa::nova(Path::new("Gauchas2").join("X - Y.mp3").to_string_lossy().into_owned()),
                Faixa::nova(Path::new("Sertanejo").join("A - B.mp3").to_string_lossy().into_owned()),
            ],
            sujo: false,
        };
        for _ in 0..20 {
            let f = b.aleatoria_em(&raiz.join("Gauchas")).unwrap();
            assert_eq!(PathBuf::from(f.caminho), raiz.join(&gauchas));
        }
        assert!(b.aleatoria_em(&raiz.join("Vazia")).is_none());
    }
}
