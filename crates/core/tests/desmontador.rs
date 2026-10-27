//! Compatibilidade do desmontador com o `avr-objdump -D -m avr5`.

use std::path::PathBuf;
use std::process::Command;

use ajusta_asm_core::{Opcoes, converter, desmontar};

const ORIGEM: &str = include_str!("fixtures/origem.lst");
const DESTINO: &str = include_str!("fixtures/destino.asm");

/// Gera um Intel HEX (registros de 16 bytes) a partir dos bytes.
fn intel_hex(dados: &[u8]) -> String {
    fn registro(endereco: u16, tipo: u8, dados: &[u8]) -> String {
        let mut bytes = vec![dados.len() as u8];
        bytes.extend(endereco.to_be_bytes());
        bytes.push(tipo);
        bytes.extend(dados);
        let soma = bytes.iter().fold(0u8, |s, b| s.wrapping_add(*b));
        bytes.push(soma.wrapping_neg());
        let hex: String = bytes.iter().map(|b| format!("{b:02X}")).collect();
        format!(":{hex}\n")
    }

    let mut saida = String::new();
    for (i, bloco) in dados.chunks(16).enumerate() {
        let endereco = (i * 16) as u32;
        if endereco.is_multiple_of(0x1_0000) && endereco > 0 {
            saida += &registro(0, 4, &((endereco >> 16) as u16).to_be_bytes());
        }
        saida += &registro(endereco as u16, 0, bloco);
    }
    saida + &registro(0, 1, &[])
}

/// Bytes da memória de programa, lidos da coluna de bytes da listagem.
fn bytes_da_listagem(listagem: &str) -> Vec<u8> {
    let mut dados = Vec::new();
    for linha in listagem.lines() {
        let Some((endereco, resto)) = linha.split_once(":\t") else {
            continue;
        };
        if u32::from_str_radix(endereco.trim(), 16).is_err() {
            continue;
        }
        let bytes = resto.split('\t').next().unwrap_or_default();
        dados.extend(
            bytes
                .split_whitespace()
                .map(|b| u8::from_str_radix(b, 16).unwrap()),
        );
    }
    dados
}

#[test]
fn listagem_igual_a_do_atmel_studio() {
    let hex = intel_hex(&bytes_da_listagem(ORIGEM));
    let gerada = desmontar(&hex, "prog_c2.hex").unwrap();

    // A primeira linha da fixture é o comando executado.
    let esperada = ORIGEM.split_once('\n').unwrap().1;
    for (numero, (g, e)) in gerada.lines().zip(esperada.lines()).enumerate() {
        assert_eq!(g, e, "linha {}", numero + 2);
    }
    assert_eq!(gerada.lines().count(), esperada.lines().count());
}

#[test]
fn hex_gera_o_mesmo_asm_que_o_script_python() {
    let hex = intel_hex(&bytes_da_listagem(ORIGEM));
    let resultado = converter(&hex, &Opcoes::default());
    assert!(resultado.avisos.is_empty(), "{:?}", resultado.avisos);
    assert!(resultado.texto == DESTINO, "saída diverge do esperado");
}

/// Compara, word a word, com o avr-objdump instalado. Sem ele, o teste é
/// pulado (no macOS: `brew install avr-binutils`; no Ubuntu: `binutils-avr`).
#[test]
fn todas_as_words_iguais_ao_avr_objdump() {
    if Command::new("avr-objdump")
        .arg("--version")
        .output()
        .is_err()
    {
        eprintln!("avr-objdump não encontrado; teste pulado");
        return;
    }

    // Cada word é seguida de 0x1234 (cpse r3, r20), que também serve de
    // segunda word para jmp/call/lds/sts.
    let dados: Vec<u8> = (0..=u16::MAX)
        .flat_map(|w| [w.to_le_bytes(), 0x1234u16.to_le_bytes()])
        .flatten()
        .collect();
    let caminho: PathBuf = std::env::temp_dir().join(format!("todas-{}.hex", std::process::id()));
    std::fs::write(&caminho, intel_hex(&dados)).unwrap();

    let saida = Command::new("avr-objdump")
        .args(["-D", "-z", "-m", "avr5"])
        .arg(&caminho)
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&caminho);
    assert!(saida.status.success());
    let esperada = String::from_utf8(saida.stdout).unwrap();

    let nome = caminho.file_name().unwrap().to_str().unwrap();
    let gerada = desmontar(&intel_hex(&dados), nome).unwrap();
    // O objdump imprime o caminho completo no cabeçalho; compara o resto.
    let instrucoes = |texto: &str| -> Vec<String> {
        texto
            .lines()
            .filter(|l| l.contains(":\t"))
            .map(str::to_owned)
            .collect()
    };
    let (gerada, esperada) = (instrucoes(&gerada), instrucoes(&esperada));
    for (g, e) in gerada.iter().zip(&esperada) {
        assert_eq!(g, e);
    }
    assert_eq!(gerada.len(), esperada.len());
}
