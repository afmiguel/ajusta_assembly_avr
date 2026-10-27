use std::io::{Read, Write};
use std::path::PathBuf;

use ajusta_asm_core::{Opcoes, converter};
use anyhow::Context;
use clap::Parser;

/// Converte a listagem do `avr-objdump -D` em assembly para o Atmel Studio.
#[derive(Parser)]
#[command(version)]
struct Args {
    /// Listagem gerada pelo avr-objdump (padrão: entrada padrão).
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

    let opcoes = Opcoes {
        compactar_preenchimento: !args.manter_preenchimento,
        comentar_endereco: args.comentar_endereco,
    };
    let resultado = converter(&listagem, &opcoes);

    for aviso in &resultado.avisos {
        eprintln!("aviso: linha {}: {}", aviso.linha, aviso.mensagem);
    }

    match &args.saida {
        Some(caminho) => std::fs::write(caminho, &resultado.texto)
            .with_context(|| format!("não foi possível gravar {}", caminho.display()))?,
        None => std::io::stdout().write_all(resultado.texto.as_bytes())?,
    }
    Ok(())
}
