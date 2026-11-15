//! Lê a duração de um arquivo de áudio sem decodificar o som.
//! Só roda para o que entra na fila (nunca para o acervo inteiro).

use std::fs::File;
use std::path::Path;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Duração em segundos.
pub fn duracao(caminho: &Path) -> Option<f64> {
    let arquivo = File::open(caminho).ok()?;
    let mss = MediaSourceStream::new(Box::new(arquivo), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = caminho.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let sondado = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .ok()?;
    let mut formato = sondado.format;
    let trilha = formato.default_track()?;
    let id = trilha.id;
    let taxa = trilha.codec_params.sample_rate? as f64;

    // Quase sempre o cabeçalho já diz quantas amostras o arquivo tem.
    if let Some(n) = trilha.codec_params.n_frames {
        return (n > 0).then(|| n as f64 / taxa);
    }

    // Senão, soma o tamanho dos pacotes (lê o arquivo, mas não decodifica).
    let mut quadros: u64 = 0;
    while let Ok(pacote) = formato.next_packet() {
        if pacote.track_id() == id {
            quadros += pacote.dur;
        }
    }
    (quadros > 0).then(|| quadros as f64 / taxa)
}
