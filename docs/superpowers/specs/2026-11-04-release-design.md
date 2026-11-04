# Release: Flatpak, AppImage e Windows em x86_64 e arm64

## Objetivo

Cada tag `v*` publica um GitHub Release com o Cadmium empacotado para Linux e Windows, nas
duas arquiteturas, e uma tabela no texto do release listando todas as opções.

## Identidade do app

- Ícone: a rede hexagonal da wurtzita (CdS) vista pelo eixo c, em amarelo cádmio médio
  (`#f6b600`, metade direita `#d98a00`), contorno interno claro (`#fff4a8`), seis átomos
  escuros (`#3d3846`) nos vértices e o átomo de cádmio prata (`#b8c2cc`) no centro.
  - `io.github.marcelogm.Cadmium.svg` (colorido, 128 px de grade)
  - `io.github.marcelogm.Cadmium-symbolic.svg` (16 px, monocromático)
  - `cadmium.ico` (16–256 px) embutido no `.exe` do Windows
- `io.github.marcelogm.Cadmium.desktop` e `io.github.marcelogm.Cadmium.metainfo.xml`
  (validados com `desktop-file-validate` e `appstreamcli validate`).
- Os arquivos ficam em `crates/cadmium-app/data/`, fora do gresource.

## Workflow `release.yml`

Substitui o `windows.yml`.

- Gatilhos: push em `main` (só artifacts), tag `v*` (artifacts e Release) e
  `workflow_dispatch`.
- Na tag, um job inicial confere que `vX.Y.Z` é igual a `version` do `Cargo.toml`.
- Runners nativos: `ubuntu-24.04`, `ubuntu-24.04-arm`, `windows-latest`, `windows-11-arm`.
- Job final (só na tag): baixa todos os artifacts, gera `SHA256SUMS` e cria o Release com
  a tabela de downloads.

| Pacote | Arquivo | Como é feito |
|---|---|---|
| Flatpak | `Cadmium-<arch>.flatpak` | `flatpak-builder` no container `ghcr.io/flathub-infra/flatpak-github-actions:gnome-51`, runtime `org.gnome.Platform//51`, `rust-stable`, fontes do cargo offline (`cargo-sources.json` gerado pelo `flatpak-cargo-generator`) |
| AppImage | `Cadmium-<arch>.AppImage` | build no container Fedora 44; `quick-sharun` empacota o binário com todas as bibliotecas, inclusive glibc e os drivers Vulkan do Mesa |
| Windows zip | `Cadmium-windows-<arch>.zip` | MSYS2 UCRT64 (x86_64) e CLANGARM64 (arm64), mesma montagem do zip atual |
| Windows instalador | `Cadmium-windows-<arch>-setup.exe` | Inno Setup a partir da pasta do zip; atalho no menu Iniciar, desinstalador |

Flatpak: `--socket=wayland`, `--socket=fallback-x11`, `--share=ipc`, `--device=dri`; os
arquivos passam pelo portal (o app já usa `gtk::FileDialog`).

## Testes no pipeline

- Windows: os testes do engine sem GPU, como hoje, e `cadmium.exe --help` com só as DLLs
  empacotadas.
- AppImage: `--help` rodando o AppImage extraído.
- Flatpak: o build roda dentro do `flatpak-builder`; sem teste de execução.

## Riscos conhecidos

- AppImage com driver NVIDIA proprietário: o pacote leva o Mesa (AMD/Intel); a NVIDIA vem do
  sistema e pode não carregar. Documentar se falhar.
- Windows arm64: a CI só confirma que o app abre; a pintura não é testada em ARM.

## README

A seção "Windows" vira "Instalação", com cada pacote e como instalar.
