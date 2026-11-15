//! Lê a duração e desenha a forma de onda de um arquivo de áudio.
//! Só roda para o que entra na fila (nunca para o acervo inteiro).

use serde::Serialize;
use std::fs::File;
use std::path::Path;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

pub const BARRAS: usize = 120;

#[derive(Debug, Clone, Serialize, Default)]
pub struct Analise {
    /// Duração em segundos.
    pub duracao: f64,
    /// Pico de cada trecho, 0–100.
    pub onda: Vec<u8>,
}

pub fn analisar(caminho: &Path) -> Option<Analise> {
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
    let canais = trilha.codec_params.channels.map(|c| c.count()).unwrap_or(2).max(1);
    let mut decoder = symphonia::default::get_codecs()
        .make(&trilha.codec_params, &DecoderOptions::default())
        .ok()?;

    // Picos em blocos de ~50 ms; depois reagrupa em BARRAS.
    let bloco = (taxa * 0.05) as usize;
    let mut picos: Vec<f32> = Vec::new();
    let mut atual = 0f32;
    let mut no_bloco = 0usize;
    let mut quadros: u64 = 0;
    let mut buf: Option<SampleBuffer<f32>> = None;

    loop {
        let pacote = match formato.next_packet() {
            Ok(p) => p,
            Err(Error::IoError(_)) | Err(Error::ResetRequired) => break,
            Err(_) => break,
        };
        if pacote.track_id() != id {
            continue;
        }
        let decodificado = match decoder.decode(&pacote) {
            Ok(d) => d,
            Err(Error::DecodeError(_)) => continue,
            Err(_) => break,
        };
        let b = buf.get_or_insert_with(|| {
            SampleBuffer::<f32>::new(decodificado.capacity() as u64, *decodificado.spec())
        });
        if b.capacity() < decodificado.capacity() {
            *b = SampleBuffer::<f32>::new(decodificado.capacity() as u64, *decodificado.spec());
        }
        b.copy_interleaved_ref(decodificado);
        for quadro in b.samples().chunks(canais) {
            let v = quadro.iter().fold(0f32, |m, s| m.max(s.abs()));
            atual = atual.max(v);
            no_bloco += 1;
            quadros += 1;
            if no_bloco >= bloco {
                picos.push(atual);
                atual = 0.0;
                no_bloco = 0;
            }
        }
    }
    if no_bloco > 0 {
        picos.push(atual);
    }
    if quadros == 0 {
        return None;
    }

    let mut onda = vec![0u8; BARRAS];
    if !picos.is_empty() {
        for (i, barra) in onda.iter_mut().enumerate() {
            let ini = i * picos.len() / BARRAS;
            let fim = ((i + 1) * picos.len() / BARRAS).max(ini + 1).min(picos.len());
            let pico = picos[ini..fim].iter().fold(0f32, |m, v| m.max(*v));
            *barra = (pico.min(1.0) * 100.0).round() as u8;
        }
    }
    Some(Analise { duracao: quadros as f64 / taxa, onda })
}
