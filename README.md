# Cadmium

Cadmium é um simulador de pintura a óleo para o GNOME. A tinta fica molhada na tela e se
comporta como óleo: o pincel deposita, arrasta e mistura a tinta que já está lá, a espátula
assenta placas grossas e abre sulcos, e as cores se misturam como pigmentos, não como luz.
Cada pigmento é descrito por um espectro de absorção e espalhamento (Kubelka-Munk), e toda a
simulação roda na GPU.

## Requisitos

- GNOME com GTK 4.22 ou mais recente e libadwaita 1.9 ou mais recente
- GPU com Vulkan (testado em AMD com o driver RADV do Mesa)
- Rust 1.92 ou mais recente para compilar
- Uma mesa digitalizadora com caneta (Wacom ou similar) é o recomendado; o mouse funciona,
  mas sem pressão nem inclinação

## Compilar e executar

```sh
cargo run --release -p cadmium-app
```

Use sempre o build `--release`: sem otimização a simulação fica lenta demais para pintar.

Uma forma prática de ter as dependências no Fedora é uma toolbox:

```sh
toolbox create cadmium
toolbox run -c cadmium sudo dnf install -y gtk4-devel libadwaita-devel rust cargo gcc \
    mesa-vulkan-drivers vulkan-loader
toolbox run -c cadmium cargo run --release -p cadmium-app
```

Se a apresentação direta pela GPU (dmabuf) não estiver disponível, o Cadmium passa a copiar
cada quadro pela memória e avisa com "Apresentação por cópia". Para forçar esse modo:
`CADMIUM_PRESENT=readback cargo run --release -p cadmium-app`.

## Instalação

Baixe o pacote na página de [Releases](https://github.com/marcelogm/cadmium/releases). Há
versões para x86_64 (Intel/AMD) e arm64. A versão mais recente da `main` fica nos *Artifacts*
do último build do workflow
[Release](https://github.com/marcelogm/cadmium/actions/workflows/release.yml) (é preciso estar
logado no GitHub).

| Sistema | Pacote | Como instalar |
|---|---|---|
| Linux | `Cadmium-<arch>.flatpak` | `flatpak install --user Cadmium-x86_64.flatpak` (o runtime GNOME 51 vem do Flathub) |
| Linux | `Cadmium-<arch>.AppImage` | `chmod +x Cadmium-x86_64.AppImage` e abra; roda em qualquer distribuição |
| Windows | `Cadmium-windows-<arch>-setup.exe` | Execute o instalador; o Cadmium aparece no menu Iniciar |
| Windows | `Cadmium-windows-<arch>.zip` | Descompacte a pasta inteira e abra `cadmium.exe` |

Todos precisam de uma placa de vídeo com Vulkan e com o driver atualizado. O AppImage já traz
os drivers Vulkan do Mesa (AMD e Intel); com NVIDIA, usa o driver proprietário instalado no
sistema. No Windows, a mesa digitalizadora funciona pelo Windows Ink.

Para publicar uma versão, atualize `version` no `Cargo.toml` e a lista `<releases>` do
metainfo, e crie a tag `v<versão>`. O workflow **Release** gera todos os pacotes e o Release.

## Pintando com a caneta

- **Pressão** controla o tamanho da pincelada e o quanto o pincel afunda na tinta molhada.
- **Inclinação** muda a forma da marca: a caneta deitada alonga a pegada do pincel, como
  quando se pinta com a lateral dele, e faz a espátula assentar tinta em vez de raspar.
- **Borracha** (a outra ponta da caneta) usa o **Pano**, que tira a tinta molhada.
- **Botão lateral de baixo** é o **Conta-gotas**: pega a tinta do ponto tocado e carrega o
  pincel com ela.
- **Botão lateral de cima** é **Mover**: arrasta a tela.

Os botões laterais não fazem nada enquanto a ponta está pintando.

Com o mouse, o botão do meio arrasta a tela e o botão direito é o conta-gotas. A roda do
mouse aproxima e afasta em torno do cursor; com Shift, a roda desloca a tela. No touchpad,
o gesto de pinça também aproxima e afasta.

## Ferramentas

| Ferramenta  | O que faz                                                                  |
|-------------|----------------------------------------------------------------------------|
| Mover       | Arrasta a tela                                                             |
| Pincel      | Deposita a tinta do pincel e arrasta a tinta molhada que encontra          |
| Espátula    | Assenta placas de tinta ou raspa e abre sulcos                             |
| Misturador  | Funde a tinta molhada sem acrescentar tinta                                |
| Pano        | Tira a tinta molhada                                                       |
| Conta-gotas | Carrega o pincel com a tinta do ponto tocado                               |

No painel **Pincel** ficam o tamanho, a **Carga** (quanta tinta o pincel leva), o **Médium**
(tinta mais fluida e transparente) e o **Formato** da ponta: Redondo, Chato ou Leque.

### Espátula

Com a caneta deitada (bem inclinada), a espátula assenta uma placa de tinta; em pé, ela raspa
e abre sulco na tinta molhada. Pressão leve deixa a placa mais grossa. O tamanho é o
comprimento da lâmina e a **Espessura da placa** define quanta tinta fica na tela.

## Carregando o pincel

Os pigmentos da pintura aparecem como montes de tinta no topo do painel **Pincel**:

- **Clique** limpa o pincel e o carrega com aquele pigmento; clicar de novo no mesmo monte
  mergulha mais.
- **Ctrl+clique** mistura um pouco daquele pigmento ao que já está no pincel.
- **Shift+clique** limpa o pincel e o carrega de novo, mesmo que seja o mesmo pigmento.

A amostra **No pincel** mostra à esquerda a mistura da paleta e à direita a cor que está de
fato na ponta, que vai se sujando com a tinta da tela. A barra embaixo indica quanta tinta
resta.

- **Recarregar a cada traço** repõe a reserva com a mistura da paleta antes de cada traço.
- **Limpar a cada traço** começa cada traço com o pincel limpo e recarregado.
- **Limpar pincel** tira toda a tinta do pincel.

A espátula se carrega do mesmo jeito.

## Nova pintura

**Nova pintura** (Ctrl+N) escolhe o formato e o tamanho da tela (até 4096 × 4096), a base
(gesso branco ou imprimatura siena), a textura da tela (linho, algodão ou lisa) e até oito
pigmentos entre os doze disponíveis. O menu ao lado de **Pigmentos** traz paletas prontas,
como a paleta limitada de Zorn.

## Secar e desfazer

**Secar** (Ctrl+D) seca toda a tinta molhada de uma vez. A tinta seca não se mexe mais, e o
que for pintado depois fica por cima dela.

**Desfazer** (Ctrl+Z) e **Refazer** (Ctrl+Shift+Z) valem para traços e para a secagem.

## Salvar, abrir e exportar

As pinturas são salvas em arquivos `.cadmium`, que guardam a tela inteira, inclusive a
tinta ainda molhada, os relevos e os pigmentos escolhidos. Ao abrir, dá para continuar
pintando de onde parou.

**Exportar PNG** (Ctrl+E) gera uma imagem da pintura no tamanho da tela.

## Lab

O painel **Lab** serve para olhar por dentro da simulação:

- **Visualizar** mostra a cor, a altura da tinta, a umidade ou a quantidade de um pigmento.
- Os controles de **Transferência**, **Dinâmica**, **Ferramentas**, **Pegada** e **Render**
  ajustam o comportamento da tinta; **Restaurar padrões** volta aos valores originais.
- **Caneta** mostra a pressão, a inclinação e a taxa de amostras que chegam da caneta.
- **Pigmentos** mostra cada pigmento em massa, em tinta 1:1 com branco e em veladura sobre
  branco e preto.

## Atalhos de teclado

| Atalho              | Ação                              |
|---------------------|-----------------------------------|
| H                   | Mover                             |
| 1                   | Pincel                            |
| 2                   | Espátula                          |
| 3                   | Misturador                        |
| 4 ou R              | Pano                              |
| 5                   | Conta-gotas                       |
| Ctrl+Z              | Desfazer                          |
| Ctrl+Shift+Z        | Refazer                           |
| Ctrl+D              | Secar                             |
| Ctrl+N              | Nova pintura                      |
| Ctrl+O              | Abrir                             |
| Ctrl+S              | Salvar                            |
| Ctrl+Shift+S        | Salvar como                       |
| Ctrl+E              | Exportar PNG                      |
| Ctrl+Q              | Sair                              |
| Ctrl++ ou Ctrl+=    | Aproximar                         |
| Ctrl+-              | Afastar                           |
| Ctrl+0              | Ajustar à janela                  |
| F9                  | Mostrar ou ocultar barra lateral  |
| Ctrl+?              | Atalhos de teclado                |

As teclas de 1 a 5 do teclado numérico também escolhem as ferramentas.

## Dados dos pigmentos

Os espectros dos pigmentos são provisórios: foram ajustados à mão a partir da aparência
conhecida de cada pigmento e não são medições. As cores e as misturas são plausíveis, mas
não reproduzem com exatidão as tintas reais.

## Limitações conhecidas

- A rotação da caneta em torno do próprio eixo não é usada.
- O pincel é simulado por carimbos ao longo do traço, não por cerdas individuais; um motor de
  cerdas está planejado.
