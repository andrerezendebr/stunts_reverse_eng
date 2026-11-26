# Stunts em Rust

Projeto inicial de reimplementação de Stunts para DOS em Rust, com apoio do Ghidra para estudar as rotinas originais.

O projeto ainda não executa o jogo. A ferramenta atual lista arquivos e inspeciona cabeçalhos DOS MZ e strings ASCII.

## Executar

Instale o toolchain Rust e coloque sua própria cópia do jogo na pasta `stunts_dos/`, na raiz do repositório.

```powershell
cd stunts-rs
cargo run -- inventory ..\stunts_dos
cargo run -- inspect ..\stunts_dos\LOAD.EXE
cargo run -- inspect ..\stunts_dos\MCGA.HDR
```

## Organização

- `stunts-rs/`: código Rust das ferramentas de análise e da futura reimplementação.
- `stunts_dos/`: cópia local do jogo, excluída do Git.
- `reverse/`: projeto Ghidra e registros locais, excluídos do Git.

## Etapas previstas

1. Confirmar a versão e estudar o carregamento e a descompressão dos módulos.
2. Reconstruir uma imagem estática para análise no Ghidra.
3. Implementar leitores dos recursos com formatos confirmados.
4. Portar matemática e física preservando o comportamento dos inteiros do DOS.
5. Comparar estados por frame usando replays antes de integrar vídeo, entrada e áudio.

## Referências

- [ReStunts](https://github.com/4d-stunts/restunts)
- [ReStunts2](https://github.com/dstien/restunts2)

Os executáveis e recursos originais do jogo não são distribuídos neste repositório.
