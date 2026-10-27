//! A saída deve ser idêntica à gerada pelo script Python original
//! (`ajusta_arquivo_asm.py`) para a mesma listagem.

use ajusta_asm_core::{Opcoes, converter};

#[test]
fn igual_ao_script_python() {
    let origem = include_str!("fixtures/origem.lst");
    let esperado = include_str!("fixtures/destino.asm");

    let resultado = converter(origem, &Opcoes::default());

    assert!(resultado.avisos.is_empty(), "{:?}", resultado.avisos);
    if resultado.texto != esperado {
        let divergencia = resultado
            .texto
            .lines()
            .zip(esperado.lines())
            .enumerate()
            .find(|(_, (a, b))| a != b);
        panic!("saída diverge do esperado: {divergencia:?}");
    }
}
