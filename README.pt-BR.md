# Skilvella

[日本語](README.ja.md) | [English](README.md) | [简体中文](README.zh-CN.md) | [繁體中文](README.zh-TW.md) | [한국어](README.ko.md) | [Español](README.es.md) | [Français](README.fr.md) | [Deutsch](README.de.md) | **Português (Brasil)**

Um aplicativo local que organiza grandes coleções de arquivos `SKILL.md` no navegador e carrega via MCP somente as habilidades de que você precisa. Não importa quantas habilidades você tenha, ele expõe apenas três ferramentas MCP.

- Busca por palavras-chave em nomes, descrições, conteúdo e etiquetas, incluindo correspondência de substrings em japonês.
- Registro de pastas, criação de habilidades, edição de conteúdo, etiquetas e categorias, operações em lote e alternância da publicação via MCP por habilidade.
- Histórico salvo antes de cada edição com restauração, detecção de conflitos com edições externas e verificações de nomes duplicados, erros de sintaxe e arquivos ausentes.
- Windows / macOS / Linux. Use o navegador no mesmo PC.

## Primeiros passos

No Windows, execute [SkilvellaApp-win-Setup.exe](https://skilvella.kagayoi.com/SkilvellaApp-win-Setup.exe) e abra "Skilvella" pelo menu Iniciar. A versão com instalador (Setup) se registra durante a instalação para iniciar quando o usuário atual entra no Windows. Ao migrar de uma versão autônoma (ZIP) existente, primeiro encerre a versão antiga pelo ícone na bandeja de tarefas (se a versão antiga ainda estiver em execução, a porta padrão (7465) não estará disponível, e a versão com instalador (Setup) exibe uma mensagem informando isso e não inicia). O registro de inicialização automática da versão ZIP é trocado automaticamente para a versão com instalador (Setup) durante a instalação, e o atalho "Skilvella" do menu Iniciar passa a apontar para a versão com instalador (Setup) também. Se você havia desativado a inicialização automática, ela continua desativada. A desinstalação também remove o registro de inicialização automática que aponta para a versão com instalador (Setup). A pasta da versão antiga não é excluída automaticamente, então remova-a manualmente se não precisar mais dela. Se você usa uma versão antiga com instalador (Setup) sem mecanismo de atualização automática (1.1.4 a 1.1.6), reinstale uma vez com o novo Setup. A pasta de dados padrão `%LOCALAPPDATA%\Skilvella` é compartilhada e permanece após a desinstalação. A versão com instalador é atualizada automaticamente ([Atualizações do aplicativo](#app-updates)).

Para usar a versão ZIP ou a versão para macOS, baixe o pacote correspondente ([Windows x64](https://skilvella.kagayoi.com/portable/Skilvella-windows-x86_64.zip) ou [Mac Apple Silicon](https://skilvella.kagayoi.com/portable/Skilvella-macos-aarch64.tar.gz), hospedados em skilvella.kagayoi.com) e extraia-o. As versões ZIP e macOS até a 1.1.8 procuravam atualizações no GitHub, o que não é mais usado; se você usa uma delas, baixe o novo pacote manualmente uma única vez.

- Windows x64: extraia `Skilvella-windows-x86_64.zip` para a pasta que você continuará usando e abra `skilvella.exe`. Ele é executado na bandeja de tarefas sem abrir um terminal, e a primeira inicialização normal adiciona "Skilvella" ao menu Iniciar do usuário atual.
- Mac Apple Silicon: extraia `Skilvella-macos-aarch64.tar.gz` e execute `./skilvella` em um terminal.
- Antes de extrair, você pode verificar o SHA-256 do pacote com o [`SHA256SUMS.txt`](https://skilvella.kagayoi.com/portable/SHA256SUMS.txt).

Para Linux ou para compilar a partir do código-fonte, consulte o [guia de desenvolvimento](docs/DEVELOPMENT.md).

O Skilvella mostra seu ícone na bandeja de tarefas no Windows e na barra de menus no Mac. Clique com o botão esquerdo para abrir o gerenciador no navegador padrão. Clique com o botão direito (ou Control-clique no Mac) para escolher "Abrir gerenciador", "Buscar atualizações", "Reiniciar serviço" ou "Sair". No Windows, se você não vir o ícone, verifique também a área de ícones ocultos da bandeja.

"Reiniciar serviço" reinicia o servidor HTTP/MCP e o monitoramento de arquivos no mesmo processo e na mesma URL. As habilidades e configurações salvas são mantidas. Solicitações duplicadas durante a reinicialização são combinadas, e a reinicialização aguarda a conclusão das operações HTTP em andamento. As conexões MCP são encerradas, então reconecte pelo seu cliente. Recarregue qualquer página do gerenciador aberta antes de usá-la. "Sair" encerra o servidor e o ícone.

Você também pode abrir o gerenciador diretamente em [http://127.0.0.1:7465/](http://127.0.0.1:7465/). O endpoint MCP é `http://127.0.0.1:7465/mcp`. Em um ambiente sem tela, passe `--no-tray`. No Linux, ele não mostra ícone e é executado como um servidor HTTP/MCP.

Na estrutura autônoma do Windows, iniciar sem argumentos tenta registrar o aplicativo para iniciar quando o usuário atual entra no Windows. Se já estiver registrado, abrir o executável de outro local não altera o caminho registrado. Você pode ativar ou desativar a inicialização automática em "Configurações" → "Aplicativos" → "Inicialização" do Windows. Para não registrar nesta inicialização, passe `--no-autostart`. Para a inicialização automática sob gerenciamento do Toolport, configure-a no lado do Toolport.

Não são necessários Rust, Node.js, Python nem um servidor de banco de dados externo em tempo de execução. Quando iniciado a partir de um terminal, você também pode encerrar com `Ctrl+C` no console anexado.

## Gerenciando a biblioteca

Na primeira inicialização, o Skilvella detecta a pasta gerenciada automaticamente. Ele verifica, nesta ordem, `~/.agents/skills`, `$CODEX_HOME/skills` e `~/.codex/skills` existentes e, se nenhuma for encontrada, cria `<data-folder-name>-skills` ao lado da pasta de dados. Um candidato que contenha apenas entradas de invocação do Codex geradas pelo Skilvella é ignorado, e as próprias entradas não são contadas como habilidades gerenciadas. Isso vale somente quando a pasta de dados tem um registro de propriedade dessas entradas. Se você recriar a pasta de dados ou apontar para outra com `--data-dir`, as entradas geradas anteriormente que permanecerem serão registradas como habilidades comuns. Se havia origens registradas antes, essa configuração tem prioridade e é mantida. Se um problema ao ler o registro de propriedade das entradas ou os candidatos impedir a conclusão da detecção automática, o destino de registro automático salvo é mantido. Confira o status da detecção na lista de pastas à esquerda da biblioteca.

A biblioteca é uma tela única: a lista de pastas gerenciadas à esquerda e a lista de habilidades à direita. Pressione "+ Adicionar pasta" para abrir o seletor de pastas padrão do sistema; a pasta escolhida é registrada como está e verificada. Os arquivos não são movidos. Você não precisa informar um nome, e o nome da pasta é usado como nome de exibição (você pode alterá-lo depois com "Alterar nome de exibição"). Em ambientes em que o diálogo não pode ser aberto (como sessões SSH), insira um caminho absoluto em "Digitar um caminho".

Ao selecionar uma pasta à esquerda, são exibidos seu local, a contagem e o aviso de caminho de busca padrão, junto com "Alterar nome de exibição" e "Parar de gerenciar". Para a pasta padrão, "Alterar pasta de referência" permite trocar a pasta referenciada, e "Procurar…" permite escolhê-la pelo diálogo. A configuração persiste entre reinicializações, e "Parar de gerenciar" não exclui arquivos. Se você escolher uma pasta que já é gerenciada, ou que tenha relação de pai/filho com uma pasta gerenciada, a adição é interrompida e o nome da pasta sobreposta é exibido. Se uma pasta for movida ou excluída fora do Skilvella, aparece um aviso na barra lateral e na página da pasta, e a sincronização para até que seja resolvido. Se você a moveu, abra "Alterar o local da pasta" e informe o novo local (ou escolha com "Procurar…"). Apenas o registro é atualizado; nenhum arquivo é movido, e os IDs das habilidades, o estado de publicação, o uso e o histórico são mantidos. Se não precisar mais da pasta, escolha "Parar de gerenciar". Se o novo local já for gerenciado como outra pasta, pare de gerenciar essa pasta primeiro.

As novas habilidades encontradas por adição, detecção automática ou edições externas são registradas com **publicação MCP ativada** se puderem ser lidas. Habilidades com erros de leitura ou frontmatter inválido permanecem não publicadas e não são publicadas automaticamente nem depois de corrigidas. O estado de publicação das habilidades existentes não muda em uma nova verificação nem na inicialização (habilidades que você despublicou nunca são publicadas automaticamente). No Windows e no macOS, uma pasta cujo nome difere apenas na caixa das letras ASCII é tratada como a mesma habilidade, e seu estado de publicação não muda. Se você não quiser que uma habilidade seja publicada, escolha "Não publicada" na coluna "Status" da lista depois de adicionar a pasta. Escolher "Publicada" ou "Não publicada" na coluna "Status" da lista salva imediatamente. Você também pode alternar na página individual ou com as operações em lote da lista. Ao mudar para publicada, os arquivos são verificados novamente, e habilidades ausentes, com erros de leitura ou com frontmatter inválido não são publicadas; a seleção é revertida e o motivo é exibido. Etiquetas e categorias não mudam.

A biblioteca lista desde o início as habilidades de todas as pastas e permite pesquisá-las no próprio local. Uma pasta que tenha um diretório ou arquivo `.git` diretamente dentro dela é exibida aninhada sob a pasta como uma origem (repositório), e todo o restante é agrupado em "Habilidades pessoais". Se uma pasta sem nenhum repositório de origem diretamente dentro dela for, ela própria, um repositório Git, a pasta inteira passa a ser uma única origem. A lista normal mostra as habilidades que existem no momento. Registros antigos, como os de antes de uma movimentação, são mantidos e podem ser revisados no status "Não encontrada" ou "Todos" e em "Limpar registros de habilidades ausentes" à esquerda. Na lista "Não encontrada", "Excluir registros ausentes" exclui os dados de registro, as configurações e o histórico de todas as páginas que correspondem aos critérios de busca. Isso não pode ser desfeito. Não altera os arquivos reais e interrompe a exclusão de todas as entradas se a origem não puder ser acessada ou se um caminho existir. As categorias podem ser editadas nos metadados de cada habilidade.

O Skilvella procura os arquivos `SKILL.md` nas pastas registradas **recursivamente, sem limite de profundidade**. Ele continua verificando subdiretórios mais profundos mesmo depois de encontrar uma habilidade. Habilidades pai e filha são gerenciadas com IDs e configurações de publicação separados, e os arquivos de uma filha não são incluídos no escopo de publicação MCP da pai. Links simbólicos, junções do Windows, `.git`, `node_modules`, `target` e `.work` são excluídos. Cancelar o registro de uma pasta não exclui os arquivos originais. Se você adicionar de volta uma pasta que havia removido do gerenciamento, seus dados de gerenciamento anteriores (configurações de publicação, etiquetas, histórico) são restaurados. Pastas com o mesmo nome não entram em conflito entre si.

Salvar alterações de conteúdo modifica o arquivo real no local registrado. A prévia desativa HTML, carregamento de imagens e ações de links, e não executa scripts. As alterações feitas em editores externos são monitoradas automaticamente. Se não forem refletidas, use "Verificar novamente".

Quando houver erros de leitura, abra a lista de erros entre pastas em "Revisar e corrigir erros de leitura" na biblioteca ou em "Erros de leitura" à esquerda. Abra o nome ou o caminho da habilidade para verificar a causa e, no caso de erros de sintaxe, corrija o conteúdo e salve. Se um arquivo não puder ser lido, confira seu local, as permissões e a codificação de caracteres e verifique novamente. O aviso desaparece quando o erro é resolvido.

<a id="app-updates"></a>

## Atualizações do aplicativo

**Versão com instalador (Setup)**: as atualizações automáticas vêm ativadas por padrão. Ela procura uma nova versão na inicialização e a cada 24 horas enquanto continua em execução, e baixa a versão se houver uma disponível. A atualização é aplicada automaticamente na próxima inicialização, reiniciando com as mesmas configurações. Ela não se conecta dentro de 24 horas da verificação anterior. Se uma verificação ou download falhar, continua executando a versão atual. Você pode desativar isso em "Atualizar automaticamente ao iniciar" em "Atualizações do aplicativo", em "Configurações" (a configuração é salva em `app-update.json` na pasta de dados, como `%LOCALAPPDATA%\Skilvella`, e não altera a biblioteca de habilidades). "Buscar atualizações" verifica independentemente do intervalo de 24 horas, e "Instalar atualização" baixa a atualização e reinicia imediatamente. As informações de atualização são obtidas de `skilvella.kagayoi.com`.

O que segue vale para a versão autônoma (ZIP) e para o Mac.

Você pode verificar a versão estável mais recente no skilvella.kagayoi.com na seção de atualizações em "Configurações" ou em "Buscar atualizações" no menu da bandeja. Escolher "Instalar atualização" baixa por HTTPS o pacote para Windows x64 / Mac Apple Silicon e o `SHA256SUMS.txt`, verifica o SHA-256 e então o aplica. Isso exige o `curl` que acompanha o sistema operacional e permissão de gravação no local do executável. A atualização dentro do aplicativo não é compatível com Linux nem com outras CPUs.

A atualização aguarda a conclusão das operações HTTP em andamento antes de encerrar e depois reinicia com os mesmos argumentos de inicialização e a mesma pasta de trabalho. Reconecte seus clientes MCP. Se a verificação do pacote falhar, a versão atual é mantida. Se o novo processo não puder ser iniciado depois de aplicar a atualização, o aplicativo tenta restaurar o executável antigo ou o destino de referência do Toolport. A página de configurações que iniciou a atualização é recarregada automaticamente assim que a nova versão responde. Se você executar uma operação de atualização em uma página de configurações antiga que já estava aberta antes de o aplicativo reiniciar, será solicitado que você recarregue a página.

Uma pasta `.skilvella-update-*` é criada no mesmo local do executável anterior à atualização e mantém o `update.log`. Na estrutura normal, ela também armazena `previous-binary` para recuperação. O Skilvella exclui essa pasta automaticamente na inicialização quando ela tem mais de 24 horas, nenhuma atualização está em andamento e ela contém apenas arquivos criados pelo atualizador. Você não precisa excluí-la; se o fizer, aguarde o auxiliar de atualização encerrar.

Sob o gerenciamento `Skilvella-Latest.ps1` do Toolport, a versão existente é mantida, a pasta da nova versão e as informações de verificação são adicionadas, e o destino de referência do iniciador é trocado. O auxiliar de atualização é executado até que o aplicativo reiniciado seja encerrado, então encerre o Skilvella antes de excluir a pasta de atualização.

Quando o Skilvella é iniciado por um monitor de processos externo, talvez seja necessário reiniciá-lo a partir dessa ferramenta. Se o bloqueio de inicialização do Toolport não puder ser obtido, a atualização não é aplicada. Se ele não iniciar, execute o iniciador novamente. Um local gerenciado pelo Toolport que contenha caracteres não ASCII, como japonês, não é compatível com a atualização dentro do aplicativo. A estrutura autônoma normal não está sujeita a essa limitação.

<a id="mcp-connection"></a>

## Conexão MCP

Registre a seguinte URL em um cliente compatível com Streamable HTTP.

```text
http://127.0.0.1:7465/mcp
```

No Toolport, registre-a como um servidor HTTP e informe a mesma URL. Mantenha o Skilvella em execução.

| Ferramenta | Finalidade |
| --- | --- |
| `search_skills` | Pesquisar habilidades publicadas e obter resumos e IDs |
| `load_skill` | Obter o conteúdo de `SKILL.md` e a lista de arquivos de referência informando um ID |
| `read_skill_file` | Obter texto UTF-8 dentro de uma habilidade, por linha |

Para `load_skill`, continue passando os argumentos de continuação até que `next` seja nulo, concatene o conteúdo e só então siga as instruções. A lista normalmente tem 100 entradas por página e até 8 KiB de caminhos no total, mas, se um único caminho exceder 8 KiB, esse caminho é retornado sozinho, e a resposta fica grande. Obtenha as habilidades e as referências de que elas precisam uma de cada vez. Para as referências, use `read_skill_file` para obter `next_text_offset` do intervalo de linhas necessário até o fim e use `next_start_line` somente quando mais linhas forem necessárias. As páginas de bytes do mesmo intervalo de linhas são concatenadas como estão, e quebras de linha são inseridas entre intervalos de linhas na concatenação. Ao continuar com `next_text_offset`, mantenha `start_line` e `max_lines`; para o próximo intervalo de linhas, defina `start_line` como `next_start_line` e redefina `text_offset` para 0. As quebras de linha são normalizadas como antes, e a quebra de linha final é removida. Nas continuações, informe o `revision` retornado como `expected_revision`. Se o arquivo mudou nesse meio tempo, obtenha-o novamente desde o início. `id` é o ID de skill retornado por `search_skills`; um nome de skill publicado exato também é aceito se for único, e um nome ambíguo retorna os IDs candidatos. A primeira chamada a `read_skill_file` precisa apenas de `id` e `path`; se o resultado for truncado, chame de novo com o objeto `next`, que já inclui `expected_revision`.

O MCP é somente leitura. Ele não pode retornar o conteúdo de habilidades não publicadas, com frontmatter inválido ou ausentes. Também não há recurso para executar arquivos de referência. O conteúdo é limitado a 2 MiB e o texto de referência a 1 MiB.

A publicação MCP por si só não altera o fato de que os agentes descobrem e carregam, por padrão, as mesmas habilidades. Para evitar o carregamento duplicado, coloque as habilidades fora dos caminhos de busca padrão do agente e registre esse local com "+ Adicionar pasta" da biblioteca. O Skilvella nunca move arquivos. Se for preciso mover arquivos existentes, faça isso você mesmo e depois informe o novo local em "Alterar o local da pasta" dessa pasta. Os arquivos de configuração do Codex não são editados. Reinicie o Codex depois de alterar os caminhos de busca padrão. O conteúdo já carregado em uma conversa não é removido. Verifique separadamente as configurações de busca personalizadas e outros clientes.

## Seleção automática e sugestões de comandos

Primeiro registre e verifique o Skilvella com um cliente que use a [Conexão MCP](#mcp-connection). Em "Configurações", em "Sincronização dos pontos de invocação", marque os destinos de sincronização (Codex, Claude Code, Grok, Cursor, GitHub Copilot, Devin; é possível selecionar vários) e salve, e o próprio Skilvella sincroniza automaticamente os pontos de invocação de cada cliente. Clientes cuja pasta de configuração não é encontrada neste computador (como `~/.cursor`) ficam recolhidos em "Outros clientes" no mesmo formulário; clientes ativados ou com erro de sincronização são sempre exibidos. A tabela de status mostra apenas os clientes ativados e os que têm erro de sincronização. Se você havia ativado a "integração com o Codex" em uma versão mais antiga, ela é mantida com o Codex selecionado. Isso funciona da mesma forma no Windows, no macOS e no Linux e não usa o Agendador de Tarefas nem nenhum outro processo residente separado. A sincronização é executada na inicialização, após operações de gerenciamento e durante verificações periódicas, e a configuração é mantida após reiniciar o serviço.

| Destino de sincronização | Local do ponto de invocação | Como é exposto à busca | Controle da seleção automática |
| --- | --- | --- | --- |
| Codex | `~/.agents/skills/skilvella/<name>` | Arquivos reais (o Codex busca em pastas aninhadas) | `agents/openai.yaml` |
| Claude Code | `~/.claude/skilvella/skills/<name>` | Um link para `~/.claude/skills/<name>` (uma junção no Windows, um link simbólico no macOS e no Linux) | `disable-model-invocation` |
| Grok | `~/.grok/skills/skilvella/<name>` | Arquivos reais (o Grok busca em pastas aninhadas) | `disable-model-invocation` |
| Cursor | `~/.cursor/skills/skilvella/<name>` | Arquivos reais (o Cursor busca em pastas aninhadas, mas pode não seguir links) | `disable-model-invocation` |
| GitHub Copilot | `~/.copilot/skilvella/skills/<name>` | Um link para `~/.copilot/skills/<name>` | `disable-model-invocation` |
| Devin | `<config do Devin>/skilvella/skills/<name>` | Um link para `<config do Devin>/skills/<name>` | `triggers: [user]` |

`<config do Devin>` é `%APPDATA%\devin` no Windows e `~/.config/devin` no macOS e no Linux (`XDG_CONFIG_HOME` não é usado). O GitHub Copilot sempre usa `~/.copilot` e não segue `COPILOT_HOME`.

O Claude Code busca apenas um nível diretamente abaixo de `~/.claude/skills`, então os pontos de invocação são colocados fora do caminho de busca e um link é criado para cada habilidade. Se uma habilidade ou link existente com o mesmo nome já estiver em `~/.claude/skills`, ele não é sobrescrito, e o nome é exibido como um erro de sincronização. Um link que você criou manualmente de `~/.claude/skills/<name>` para `~/.agents/skills/skilvella/<name>` seguindo a orientação de uma versão antiga é redirecionado para o ponto de invocação do Claude Code e assumido para gerenciamento, mas somente para nomes que são destinos de sincronização. Links de nomes que não são destinos de sincronização são deixados intactos (se você desativar o Codex, o destino a que se referem desaparece). Desativar a sincronização do Claude Code remove apenas os links que o Skilvella criou ou assumiu. Se você registrou o próprio `~/.claude/skills` como pasta gerenciada, a sincronização não é executada.

O GitHub Copilot (a CLI do Copilot só encontra `~/.copilot/skills/<name>/SKILL.md`) e o Devin usam o mesmo método de links e as mesmas regras: entradas existentes com o mesmo nome não são sobrescritas, apenas os links criados pelo Skilvella são removidos e não há sincronização se a pasta de links coincidir com uma pasta gerenciada. Somente o Claude Code assume links para pontos do Codex criados manualmente. Ainda não foi confirmado se a CLI do Copilot respeita `disable-model-invocation` nem a profundidade de busca do Devin.

O Cursor também lê `~/.agents/skills` e `~/.claude/skills`, o GitHub Copilot lê `~/.agents/skills` e o Devin lê `~/.agents/skills` e `~/.copilot/skills`, então o mesmo ponto pode aparecer repetido quando vários clientes estão ativados.

O Grok também lê os caminhos de busca do Codex (`~/.agents/skills`) e do Claude Code (`~/.claude/skills`) e, para habilidades com o mesmo nome, dá prioridade na ordem `~/.grok/skills`, `~/.agents/skills`, `~/.claude/skills`. Os pontos de invocação do Codex interrompem a seleção automática com `agents/openai.yaml`, então, no Grok, até as habilidades em "Somente sugestões de comandos" são selecionadas automaticamente. Se você usa o Grok, selecione o Grok como destino de sincronização.

Escolha o modo na página de detalhes de cada habilidade ou com "Uso" na lista. Na lista, você pode conferir o modo atual e alterá-lo no próprio local. O MCP não retorna o conteúdo de habilidades não publicadas e, após uma sincronização bem-sucedida, também remove os pontos de sugestão delas.

Se você pressionar o botão Voltar do navegador ou recarregar enquanto um salvamento está em andamento na lista, o Skilvella verifica o resultado do salvamento antes de exibir as configurações mais recentes. Uma solicitação que não foi aceita porque a conexão foi interrompida é cancelada, e o resultado da verificação é exibido na tela.

| Modo | Nome e descrição passados para a seleção automática | Sugestões de comandos |
| --- | --- | --- |
| Somente busca (padrão) | Não passados | Não exibidas |
| Somente sugestões de comandos | Não passados | Exibidas |
| Seleção automática + sugestões de comandos | Passados | Exibidas |

Para habilidades que exibem sugestões de comandos, o comando de invocação de cada destino de sincronização (`$name` para o Codex, `/name` para os outros clientes) é exibido abaixo do nome da habilidade na lista e em "Uso" na página de detalhes. Se o nome contiver algo além de letras minúsculas, dígitos e `-`, ele se torna `skill-<primeiros 8 caracteres do id>` e, se existir uma habilidade publicada com o mesmo nome, torna-se `<name>-<primeiros 8 caracteres do id>`, então use o comando que for exibido. Quando não houver comando, a página de detalhes mostra o motivo (somente busca, nenhum destino de sincronização selecionado, não publicada ou erro de leitura). O comando de uma habilidade que você muda de não publicada para publicada na lista aparece após recarregar a página.

Na lista, marque habilidades, escolha "Uso" nas operações em lote e pressione "Aplicar à seleção" para alterar o modo de várias habilidades de uma vez. "Sem alteração" mantém o modo atual. Você pode alterar o estado de publicação e as etiquetas ao mesmo tempo.

Abrir "Configuração em massa por origem" em "Todas as habilidades" permite selecionar várias origens e alterar juntos o estado de publicação e o uso com "Aplicar às origens selecionadas". O escopo são as habilidades de todas as páginas registradas no momento da execução e não se estende às habilidades adicionadas depois. A contagem da pré-verificação se baseia no índice da verificação e exclui entradas ausentes e com erro. O resultado mostra o número de atualizadas e o número de excluídas. Ao publicar, os arquivos são verificados novamente e, se não puderem ser revalidados por estarem ausentes, ilegíveis, inválidos ou algo semelhante, o salvamento das configurações de todas as entradas é cancelado. As configurações são salvas em uma única transação, e as habilidades originais não são alteradas.

O Codex não tem uma configuração separada para "ativar a seleção automática, mas ocultá-la das sugestões", então o Skilvella oferece os três modos correspondentes. Mesmo com a seleção automática, ele não mantém sempre uma cópia completa do conteúdo original; o conteúdo é obtido do MCP quando necessário. As habilidades não exibidas nas sugestões podem ser buscadas com `$skilvella-search`, que é apenas para invocação explícita. Você também pode usar diretamente o `search_skills` do MCP.

O modo de uso é comum a todos os destinos de sincronização. Os locais dos pontos de invocação são os da tabela acima. Quando você despublica uma habilidade ou altera seu modo, os artefatos que deixam de ser necessários são movidos para fora dos caminhos de busca (`skilvella-staging` / `skilvella-retired` diretamente em `~/.agents`, `~/.claude`, `~/.grok`, `~/.cursor`, `~/.copilot` e `<config do Devin>`). Se arquivos desconhecidos ou edições manuais forem detectados, eles não são sobrescritos, e um erro de sincronização é exibido na página de configurações. Se arquivos temporários incompletos permanecerem por falta de espaço em disco ou encerramento forçado, a sincronização é interrompida preservando o índice íntegro existente. Confira o conteúdo do caminho exibido e tente novamente. Desativar a integração remove dos caminhos de busca os pontos de invocação de que ela é dona. Enquanto a sincronização estiver parada, sugestões e configurações de seleção automática anteriores podem permanecer. Os arquivos originais, os estados de publicação e o histórico de edição não são alterados. Se a exibição de sugestões do Codex não for atualizada, reabra o menu de sugestões ou reinicie o Codex. O Claude Code reflete alterações em `~/.claude/skills` mesmo em uma sessão em execução, mas, se você criou o próprio `~/.claude/skills` depois do início da sessão, execute `/reload-skills`. O conteúdo já carregado em uma conversa existente não pode ser removido.

<a id="display-settings"></a>

## Configurações de exibição

Na biblioteca, selecione uma pasta à esquerda e abra "Alterar nome de exibição" para mudar o nome da pasta mostrado na tela. A entrada deve ter até 100 caracteres, incluindo espaços no início e no fim, e caracteres de controle como quebras de linha não são permitidos. Os espaços no início e no fim são removidos ao salvar, e um valor vazio volta ao identificador original. O nome de exibição é salvo no aplicativo, e o nome real da pasta, os IDs das habilidades, as configurações de publicação e o histórico de edição não mudam. Nomes de exibição duplicados são permitidos, mas um nome que você alterou é exibido junto com seu identificador original.

Em "Configurações" do gerenciador, você pode escolher o idioma como automático ou um de nove idiomas (japonês, inglês, chinês simplificado, chinês tradicional, coreano, espanhol, francês, alemão, português do Brasil). O padrão "Automático" usa a configuração de idioma do navegador. O idioma é salvo quando você o seleciona e, se o JavaScript estiver desativado, pressione "Salvar". A seleção é salva por porta de conexão no mesmo navegador, e as outras abas já abertas a aplicam a partir do próximo carregamento de página. Uma configuração de idioma de uma versão antiga é mantida quando você abre o gerenciador no mesmo endereço no mesmo navegador. Mantê-la exige JavaScript. Nomes de habilidades, descrições, conteúdo, caminhos e detalhes técnicos de erros mantêm o texto original. Os menus da bandeja e da barra de menus, a saída da linha de comando como `--help`, as mensagens de inicialização e os erros, e as caixas de mensagem da versão com instalador (Setup) também são exibidos nos mesmos nove idiomas. Esses idiomas são determinados na ordem: variável de ambiente `SKILVELLA_LANGUAGE` (por exemplo `ja`, `pt-BR`) → `LC_ALL` → `LC_MESSAGES` → `LANG` → idioma de exibição do sistema operacional e, se não for possível determiná-lo, usa-se o inglês. Os nomes e as descrições das ferramentas MCP estão em inglês.

Você pode escolher o tema entre Automático, Claro e Escuro quando o JavaScript está ativado. A seleção é salva no navegador, e Automático segue a configuração de esquema de cores do navegador.

## Dados e backups

| SO | Local de dados padrão |
| --- | --- |
| Windows | `%LOCALAPPDATA%\Skilvella` |
| macOS | `~/Library/Application Support/Skilvella` |
| Linux | `$XDG_DATA_HOME/skilvella` ou `~/.local/share/skilvella` |

A fonte da verdade para `SKILL.md` e os arquivos de referência são as pastas registradas. Além do índice de busca, o `library.sqlite3` da pasta de dados também armazena etiquetas, categorias, configurações de publicação, locais registrados, nomes de exibição de pastas e histórico de edição. **Se você excluir o banco de dados inteiro, esses dados de gerenciamento serão perdidos.**

Para fazer backup, encerre primeiro o Skilvella e depois copie tanto a pasta de dados quanto as pastas registradas. O banco de dados sozinho não consegue restaurar os arquivos de habilidades atuais. Em outro PC, os caminhos absolutos das origens registradas são diferentes, então compartilhar o banco de dados como está para uso simultâneo não é compatível.

## Opções e solução de problemas

```sh
skilvella --data-dir /absolute/path/to/data --port 7465 --no-watch
skilvella --help
```

- Se a porta estiver em uso, encerre o processo existente ou altere-a com `--port`. O gerenciador e o MCP usam a mesma porta.
- Se o gerenciador não voltar depois de atualizar a partir de uma versão antiga do Windows, abra uma vez o `skilvella.exe` atualizado na estrutura normal, ou o iniciador uma vez sob o gerenciamento do Toolport.
- Se uma movimentação de pasta interrompida de uma versão antiga não puder ser recuperada na inicialização, a inicialização é abortada. Os dados originais e o registro de migração são mantidos.
- O local dos dados e os locais de registro das habilidades não podem se sobrepor. Especifique pastas separadas em um disco local.
- Edições feitas em outra aba também são detectadas como conflitos ao salvar. Copie o rascunho mantido, recarregue e mescle-o ao conteúdo mais recente.
- A busca usa AND separado por espaços e correspondência de substrings. Buscar habilidades em inglês pelo significado a partir de uma consulta em japonês, modelos de embedding e sincronização na nuvem não estão incluídos.
- Mesmo que a pasta de origem fique offline, os dados de registro e o histórico são mantidos. Verifique novamente depois que ela for reconectada.

As conexões são limitadas a `127.0.0.1`. O uso pretendido é onde você confia nos usuários e processos do mesmo PC.

A licença é [MIT](LICENSE).

Se você quiser contribuir com o desenvolvimento, consulte o [guia de desenvolvimento](docs/DEVELOPMENT.md), o [design](DESIGN.md) e as [diretrizes para contribuidores](AGENTS.md).
