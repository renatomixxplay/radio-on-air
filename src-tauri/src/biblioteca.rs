//! Acervo de músicas: lê só os NOMES dos arquivos (padrão `Artista - Música.mp3`),
//! guarda a lista em `indice.txt` e faz a busca em memória.

use deunicode::deunicode;
use rand::seq::SliceRandom;
use serde::Serialize;
use std::collections::HashMap;
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
    /// "artista música" normalizado para a busca.
    chave: String,
    /// Onde começa a música dentro da `chave`.
    inicio_musica: usize,
}

impl Faixa {
    fn nova(rel: String) -> Self {
        let (artista, musica) = artista_e_musica(Path::new(&rel));
        let artista_chave = chave(&artista);
        let inicio_musica = artista_chave.len() + 1;
        let chave = format!("{artista_chave} {}", chave(&musica));
        Self { rel, artista, musica, chave, inicio_musica }
    }

    /// Quão certeira é para a busca (menor = melhor): 0 = a música ou o
    /// artista começa com o que foi digitado; 1 = alguma palavra começa com a
    /// primeira palavra digitada; 2 = o resto.
    fn nota(&self, frase: &str, primeira: &str) -> u8 {
        let (artista, musica) = (&self.chave[..self.inicio_musica - 1], &self.chave[self.inicio_musica..]);
        if musica.starts_with(frase) || artista.starts_with(frase) {
            0
        } else if self.chave.split(|c: char| !c.is_alphanumeric()).any(|p| p.starts_with(primeira)) {
            1
        } else {
            2
        }
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
    /// Sempre em ordem de `rel` (a busca por nome usa isso).
    faixas: Vec<Faixa>,
    /// Há mudanças ainda não gravadas no `indice.txt`.
    pub sujo: bool,
    /// Um baralho por pasta (são poucas: as das listas e o acervo todo).
    baralhos: HashMap<PathBuf, Baralho>,
}

/// "Baralho" da pasta que está tocando: a pasta é embaralhada e as músicas saem
/// na ordem do baralho; só quando acabar todas é embaralhada de novo. Assim
/// nenhuma música repete antes de todas as outras da pasta tocarem.
/// Fica só na memória: ao abrir o programa, embaralha de novo.
#[derive(Default)]
struct Baralho {
    /// Cartas que ainda vão sair (a próxima é a última do vetor).
    cartas: Vec<PathBuf>,
    /// Última que saiu: não abre a rodada seguinte.
    ultima: Option<PathBuf>,
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
        let mut faixas: Vec<Faixa> = faixas;
        faixas.sort_by(|a, b| a.rel.cmp(&b.rel));
        Some(Self { raiz: Some(raiz), faixas, sujo: false, baralhos: HashMap::new() })
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
        Self { raiz: Some(raiz), faixas, sujo: true, baralhos: HashMap::new() }
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

    /// Busca por música ou artista. Todas as palavras digitadas precisam
    /// aparecer, em qualquer ordem; traço e outros sinais são ignorados. As
    /// mais certeiras vêm primeiro (veja `Faixa::nota`).
    pub fn buscar(&self, termo: &str, limite: usize) -> Vec<FaixaDto> {
        let termo = chave(termo);
        let palavras: Vec<&str> = termo.split(|c: char| !c.is_alphanumeric()).filter(|p| !p.is_empty()).collect();
        let Some(primeira) = palavras.first() else {
            return self.faixas.iter().take(limite).map(|f| self.dto(f)).collect();
        };
        let frase = palavras.join(" ");
        let mut achadas: Vec<(u8, &Faixa)> = self
            .faixas
            .iter()
            .filter(|f| palavras.iter().all(|p| f.chave.contains(p)))
            .map(|f| (f.nota(&frase, primeira), f))
            .collect();
        // Estável: dentro da mesma nota, segue a ordem do acervo.
        achadas.sort_by_key(|(nota, _)| *nota);
        achadas.into_iter().take(limite).map(|(_, f)| self.dto(f)).collect()
    }

    /// Próxima música do baralho de uma pasta (com subpastas); `None` = acervo
    /// todo. Pasta fora do acervo é lida no disco. Pasta vazia: `None`.
    pub fn proxima_em(&mut self, pasta: Option<&Path>) -> Option<FaixaDto> {
        let pasta = pasta.map(Path::to_path_buf).or_else(|| self.raiz.clone())?;
        let mut baralho = self.baralhos.remove(&pasta).unwrap_or_default();
        let carta = self.tirar(&pasta, &mut baralho);
        self.baralhos.insert(pasta, baralho);
        let carta = carta?;
        let (artista, musica) = artista_e_musica(&carta);
        Some(FaixaDto { caminho: carta.to_string_lossy().into_owned(), artista, musica })
    }

    fn tirar(&self, pasta: &Path, baralho: &mut Baralho) -> Option<PathBuf> {
        let mut embaralhou = false;
        loop {
            match baralho.cartas.pop() {
                // Pula o que saiu da pasta (apagado ou renomeado) desde que embaralhou.
                Some(carta) if self.ainda_existe(&carta) => {
                    baralho.ultima = Some(carta.clone());
                    return Some(carta);
                }
                Some(_) => {}
                None if embaralhou => return None,
                None => {
                    embaralhou = true;
                    let mut cartas = self.cartas_da_pasta(pasta);
                    cartas.shuffle(&mut rand::rng());
                    // A última da rodada anterior não abre a nova (sairia duas seguidas).
                    if cartas.len() > 1 && cartas.last() == baralho.ultima.as_ref() {
                        let fim = cartas.len() - 1;
                        cartas.swap(0, fim);
                    }
                    baralho.cartas = cartas;
                }
            }
        }
    }

    /// Todas as músicas de uma pasta: do índice, se estiver no acervo; senão, do disco.
    fn cartas_da_pasta(&self, pasta: &Path) -> Vec<PathBuf> {
        let Some(raiz) = self.raiz.as_ref().filter(|r| pasta.starts_with(r)) else {
            return WalkDir::new(pasta)
                .follow_links(true)
                .into_iter()
                .filter_map(Result::ok)
                .filter(|e| e.file_type().is_file() && eh_audio(e.path()))
                .map(|e| e.into_path())
                .collect();
        };
        let rel = pasta.strip_prefix(raiz).unwrap_or(Path::new(""));
        // Compara por partes do caminho: no Windows a barra é "\", não "/".
        self.faixas.iter().filter(|f| Path::new(&f.rel).starts_with(rel)).map(|f| raiz.join(&f.rel)).collect()
    }

    fn ainda_existe(&self, carta: &Path) -> bool {
        match self.raiz.as_ref().and_then(|r| carta.strip_prefix(r).ok()).and_then(|r| r.to_str()) {
            Some(rel) => self.faixas.binary_search_by(|f| f.rel.as_str().cmp(rel)).is_ok(),
            None => carta.is_file(),
        }
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
            baralhos: HashMap::new(),
        };
        assert_eq!(b.buscar("fogo chao", 10).len(), 1);
        assert_eq!(b.buscar("RENATO", 10).len(), 1);
        // Compara como caminho: no Windows a junção usa "\\" ("/m\\B/Maria - Rio.mp3").
        assert_eq!(PathBuf::from(&b.buscar("maria rio", 10)[0].caminho), Path::new("/m").join("B/Maria - Rio.mp3"));
        assert_eq!(b.buscar("xyz", 10).len(), 0);
        // Traço e outros sinais não atrapalham.
        assert_eq!(b.buscar("renato - fogo", 10).len(), 1);
        assert_eq!(b.buscar("renato-fogo!", 10).len(), 1);
        assert_eq!(b.buscar(" - ", 10).len(), 2);
    }

    #[test]
    fn busca_mostra_as_mais_certeiras_primeiro() {
        let b = Biblioteca {
            raiz: Some(PathBuf::from("/m")),
            faixas: vec![
                Faixa::nova("Zeca - Meu Amor Antigo.mp3".into()),
                Faixa::nova("Ana - Clamor.mp3".into()),
                Faixa::nova("Amora - Chuva.mp3".into()),
                Faixa::nova("Bia - Amor Perfeito.mp3".into()),
                Faixa::nova("Sem artista amor.mp3".into()),
            ],
            sujo: false,
            baralhos: HashMap::new(),
        };
        let nomes = |t: &str| b.buscar(t, 10).into_iter().map(|f| format!("{} - {}", f.artista, f.musica)).collect::<Vec<_>>();
        // Começa com "amor" (artista ou música) > palavra que começa com "amor" > o resto.
        assert_eq!(nomes("amor"), ["Amora - Chuva", "Bia - Amor Perfeito", "Zeca - Meu Amor Antigo", " - Sem artista amor", "Ana - Clamor"]);
        assert_eq!(nomes("amor perfeito")[0], "Bia - Amor Perfeito");
    }

    fn acervo(rels: &[&str]) -> Biblioteca {
        let mut faixas: Vec<Faixa> =
            rels.iter().map(|r| Faixa::nova(Path::new(r).components().collect::<PathBuf>().to_string_lossy().into_owned())).collect();
        faixas.sort_by(|a, b| a.rel.cmp(&b.rel));
        Biblioteca { raiz: Some(PathBuf::from("/m")), faixas, sujo: false, baralhos: HashMap::new() }
    }

    /// Agendamento de lista: só sai música de dentro da pasta escolhida (com a
    /// barra do sistema: no Windows o índice guarda "Gauchas\\x.mp3").
    #[test]
    fn sorteia_so_dentro_da_pasta() {
        let mut b = acervo(&["Gauchas/Renato - Fogo.mp3", "Gauchas2/X - Y.mp3", "Sertanejo/A - B.mp3"]);
        let gauchas = Path::new("/m").join("Gauchas");
        for _ in 0..20 {
            let f = b.proxima_em(Some(&gauchas)).unwrap();
            assert!(Path::new(&f.caminho).starts_with(&gauchas), "{}", f.caminho);
        }
        assert!(b.proxima_em(Some(&Path::new("/m").join("Vazia"))).is_none());
    }

    /// Baralho: toca todas da pasta antes de repetir, e a última de uma rodada
    /// não abre a seguinte.
    #[test]
    fn baralho_nao_repete() {
        let nomes: Vec<String> = (0..20).map(|i| format!("P/Artista - Musica {i}.mp3")).collect();
        let mut b = acervo(&nomes.iter().map(String::as_str).collect::<Vec<_>>());
        let pasta = Path::new("/m").join("P");
        let mut anterior = String::new();
        for _ in 0..10 {
            let mut rodada = std::collections::HashSet::new();
            for _ in 0..20 {
                let f = b.proxima_em(Some(&pasta)).unwrap().caminho;
                assert_ne!(f, anterior, "repetiu seguida");
                assert!(rodada.insert(f.clone()), "repetiu antes de acabar a rodada");
                anterior = f;
            }
        }
        // Acervo todo (sem lista): também em baralho.
        let mut todas = std::collections::HashSet::new();
        for _ in 0..20 {
            assert!(todas.insert(b.proxima_em(None).unwrap().caminho));
        }
    }

    /// Música apagada depois de embaralhar não sai.
    #[test]
    fn baralho_pula_apagada() {
        let mut b = acervo(&["P/A - 1.mp3", "P/A - 2.mp3", "P/A - 3.mp3"]);
        let pasta = Path::new("/m").join("P");
        let primeira = b.proxima_em(Some(&pasta)).unwrap().caminho;
        let apagar = b.baralhos[&pasta].cartas[0].clone();
        b.atualizar_caminho(&apagar);
        let resto: Vec<String> = (0..2).map(|_| b.proxima_em(Some(&pasta)).unwrap().caminho).collect();
        assert!(!resto.contains(&apagar.to_string_lossy().into_owned()));
        assert!(!resto.contains(&primeira) || resto[0] != primeira);
    }
}
