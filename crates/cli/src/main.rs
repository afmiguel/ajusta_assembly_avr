use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use ajusta_asm_core::{Opcoes, converter, desmontar, eh_intel_hex};
use anyhow::Context;
use clap::Parser;

/// Converte um arquivo Intel HEX (ou a listagem do `avr-objdump -D`) em
/// assembly para o Atmel Studio.
#[derive(Parser)]
#[command(version)]
struct Args {
    /// Arquivo .hex ou listagem do avr-objdump (padrão: entrada padrão).
    entrada: Option<PathBuf>,
    /// Arquivo de saída (padrão: saída padrão).
    #[arg(short, long)]
    saida: Option<PathBuf>,
    /// Mantém toda a sequência final de `.dw 0xffff`.
    #[arg(long)]
    manter_preenchimento: bool,
    /// Acrescenta o endereço (em words) de cada instrução como comentário.
    #[arg(long)]
    comentar_endereco: bool,
    /// Só desmonta o .hex, gerando a listagem como o `avr-objdump -D -z -m avr5`.
    #[arg(long)]
    listagem: bool,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let listagem = match &args.entrada {
        Some(caminho) => std::fs::read_to_string(caminho)
            .with_context(|| format!("não foi possível ler {}", caminho.display()))?,
        None => {
            let mut texto = String::new();
            std::io::stdin().read_to_string(&mut texto)?;
            texto
        }
    };

    if args.listagem {
        if !eh_intel_hex(&listagem) {
            anyhow::bail!("--listagem exige um arquivo Intel HEX");
        }
        let nome = args
            .entrada
            .as_deref()
            .and_then(|c| c.file_name())
            .map_or("stdin".into(), |n| n.to_string_lossy());
        let texto = desmontar(&listagem, &nome)?;
        return escrever(args.saida.as_deref(), &texto);
    }

    let opcoes = Opcoes {
        compactar_preenchimento: !args.manter_preenchimento,
        comentar_endereco: args.comentar_endereco,
    };
    let resultado = converter(&listagem, &opcoes);

    for aviso in &resultado.avisos {
        eprintln!("aviso: linha {}: {}", aviso.linha, aviso.mensagem);
    }

    escrever(args.saida.as_deref(), &resultado.texto)
}

fn escrever(saida: Option<&Path>, texto: &str) -> anyhow::Result<()> {
    match saida {
        Some(caminho) => std::fs::write(caminho, texto)
            .with_context(|| format!("não foi possível gravar {}", caminho.display())),
        None => Ok(std::io::stdout().write_all(texto.as_bytes())?),
    }
}
