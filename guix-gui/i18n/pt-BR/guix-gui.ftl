app-title = Guix GUI

tab-home = Início
tab-search = Buscar
tab-installed = Instalados
tab-updates = Atualizações
tab-channels = Canais
tab-system = Configurações
tab-about = Sobre

settings-language = Idioma
settings-language-system = Padrão do sistema

# -- shared --
common-refresh = Atualizar
common-cancel = Cancelar
common-close = Fechar
common-dismiss = Dispensar
common-remove = Remover
common-open-settings = Abrir Configurações

# -- app shell / overlay --
app-discover-failed = Falha ao localizar o guix
app-discovering = Localizando o guix...
app-brand = Guix
app-lightbox-close = Fechar (Esc)
app-lightbox-no-image = sem imagem / falhou
app-running = Executando...
app-copy = Copiar
app-show-log = Mostrar log
app-hide-log = Ocultar log
# Status label shown in the progress overlay.
app-done = Concluído.
app-failed-exit = Falhou (saída { $code }).
app-ended-no-summary = Terminou sem resumo de saída.
app-op-title = { $label } (op #{ $id })
app-cancel-pkexec-tooltip = Não dá para cancelar operações privilegiadas — o kernel não permite sinalizar processos de propriedade do root. Espere terminar.
app-bootstrap-help =
    A reconfiguração falhou: o Guix do sistema em execução não reconhece um módulo
    que sua configuração importa. Isso normalmente significa que um canal foi
    atualizado depois da sua última reconfiguração e o novo módulo ainda não está
    embutido no Guix do sistema.

    Faça o bootstrap manualmente uma vez:

        sudo guix system reconfigure -L { $load } { $cfg }

    Depois disso, este botão vai funcionar nas próximas atualizações.
app-set-source-config-first = Defina primeiro o caminho da configuração de origem na aba Sistema.
app-op-start-failed = Falha ao iniciar a op: { $error }
app-restore-panicked = A tarefa de restauração entrou em pânico: { $detail }
app-restore-failed = Falha na restauração: { $detail }
app-discovery-client-failed = O cliente de descoberta falhou: { $detail }

# -- op kinds --
op-install = Instalando
op-remove = Removendo
op-upgrade = Atualizando pacotes do usuário
op-pull = Buscando o catálogo do usuário
op-system-pull = Buscando o catálogo do sistema
op-reconfigure = Reconfigurando o sistema

# -- stages --
stage-starting = Iniciando
stage-channel-update = Atualizando canais
stage-computing-deriv = Calculando a derivação
stage-downloading = Baixando
stage-building = Compilando
stage-profile = Atualizando o perfil
# Status label shown in the progress overlay.
stage-done = Concluído
# Status label shown in the progress overlay.
stage-failed = Falhou
stage-build-failed = Falha na compilação: { $name }
stage-build-failed-log = Falha na compilação: { $name } (log: { $log })

# -- categories --
category-graphics = Gráficos
category-audio-video = Áudio e Vídeo
category-office = Escritório
category-development = Desenvolvimento
category-engineering = Engenharia
category-internet = Internet
category-game = Jogos

# -- home --
home-title = Início
home-subtitle = Um ponto de partida — aplicativos conhecidos disponíveis no Guix. Abra um para instalar, ou use a Busca para o catálogo completo.
home-installed-badge = instalado

# -- search --
search-title = Buscar
search-placeholder = Buscar pacotes...
search-loading-catalog = Carregando o catálogo de pacotes...
search-searching = Buscando...
search-results = { $count ->
    [one] { $count } resultado
   *[other] { $count } resultados
}
search-truncated = Mostrando os primeiros { $shown } de ≥{ $total } { $total ->
    [one] correspondência
   *[other] correspondências
}; refine sua busca.
search-error-label = Erro na busca:
search-copy-details = Copiar detalhes
search-select-prompt = Selecione um pacote para ver os detalhes.
search-homepage = página inicial:
search-license = licença: { $license }
search-outputs = saídas: { $outputs }
search-install = Instalar
search-remove = Remover
search-screenshots-flathub = Capturas de tela via Flathub ({ $id })
search-screenshots-debian = Capturas de tela via screenshots.debian.net
search-loading-media = Carregando ícones / capturas de tela...
search-failed = A busca falhou.

# -- installed --
installed-title = Instalados
installed-count = { $count ->
    [one] { $count } pacote instalado
   *[other] { $count } pacotes instalados
}
installed-loading = Carregando...
installed-error = Erro: { $error }

# -- updates --
updates-title = Atualizações
updates-your-packages-blurb = Gerencie seus pacotes de nível de usuário.
updates-your-packages = Seus pacotes
updates-fetch-latest = Buscar o catálogo mais recente
updates-update-my-packages = Atualizar meus pacotes
updates-system-blurb = Aplique a configuração do seu sistema. Requer autenticação de administrador.
updates-system = Sistema
updates-source-config = Configuração de origem: { $path }
updates-source-config-unset = Configuração de origem: (não definida — abra as Configurações para escolher)
updates-fetch-system = Buscar o catálogo do sistema
updates-update-system = Atualizar o sistema
# DO NOT TRANSLATE — literal shell command shown as a tooltip
updates-update-system-tip = pkexec guix system reconfigure
updates-confirm-reconfigure = Confirmar a reconfiguração do sistema
updates-reconfigure-blurb = Executando como root via pkexec. Verifique os caminhos abaixo — cada um será carregado pelo Guile com privilégios de root.
updates-config = Configuração:
updates-load-paths-none = Caminhos de carga (-L): (nenhum)
updates-load-paths = { $count ->
    [one] Caminhos de carga (-L), { $count } entrada:
   *[other] Caminhos de carga (-L), { $count } entradas:
}
updates-confirm-reconfigure-btn = Confirmar reconfiguração
updates-loading-channels = Carregando canais...
updates-error-channels = Erro ao carregar canais: { $error }
updates-last-pulled = Último pull: { $age }.
updates-last-pulled-never = Último pull: nunca.
updates-channels-none = Canais: (nenhum descoberto).
updates-channels = Canais: { $list }.
updates-channel-no-commit = (sem commit)
updates-last-pulled-root = Último pull (root): { $age }.
updates-last-pulled-root-never = Último pull (root): nunca.
updates-last-reconfigured = Última reconfiguração: { $age }.
updates-last-reconfigured-never = Última reconfiguração: nunca (não é um host Guix System?).

# -- updates: privileged help card --
updates-privileged-help-heading = Ação de administrador necessária
updates-privileged-help-no-agent = Nenhum agente de autenticação polkit foi detectado. Uma solicitação de senha pode não aparecer. Se nenhuma aparecer, inicie o agente polkit do seu ambiente de trabalho ou execute o comando equivalente abaixo em um terminal.
updates-privileged-help-failed = Esta operação privilegiada não pôde ser concluída: { $error }. Em vez disso, execute o comando equivalente abaixo em um terminal.
updates-privileged-help-failure-generic = falha na autenticação ou em uma etapa privilegiada
updates-privileged-help-cmd-label = Execute isto manualmente em um terminal:
# DO NOT TRANSLATE: literal shell command
updates-privileged-help-cmd-pull = sudo guix pull
# DO NOT TRANSLATE: literal shell command
updates-privileged-help-cmd-reconfigure = sudo guix system reconfigure -L { $load } { $cfg }

# -- about --
about-title = Sobre
about-version = Versão { $version }
about-tagline = Interface desktop para o gerenciador de pacotes Guix.
about-authors = Autores
about-source = Código-fonte e contribuições
about-source-blurb = Relatos de bugs e pull requests são bem-vindos.
about-license = Licença
about-license-line = O Guix GUI é distribuído sob a Licença Pública Geral GNU v3.0.
about-license-detail = Você pode redistribuí-lo e modificá-lo nos termos dessa licença. Veja o arquivo LICENSE no repositório para o texto completo.
about-third-party = Dados de terceiros
about-third-party-blurb = Ícones e capturas de tela de aplicativos são buscados em serviços externos quando você ativa os metadados de terceiros nas Configurações. Marcas, ícones e capturas de tela permanecem propriedade dos respectivos projetos.
about-channel-discovery = Descoberta de canais
about-channel-discovery-blurb = O submodo Descobrir da aba Canais navega pelos canais e pacotes do Guix indexados por toys.whereis.social. Opcional; requer rede. O catálogo e seus colaboradores permanecem propriedade dos respectivos projetos.
about-built-with = Construído com
about-built-with-detail = As licenças de cada crate estão listadas nos respectivos repositórios.

# -- progress overlay --
progress-last = Último: { $line }
progress-running = Executando ({ $count }):
progress-counts = { $built }/{ $started } compilados, { $dl_done }/{ $dl_started } baixados ({ $mb } MB)
progress-build-line = { "  - " }{ $name } [building]
progress-build-item = { "  - " }{ $status ->
    [done] { $name } [done]
    [failed] { $name } [FAILED]
   *[other] { $name } [building]
}
progress-finished = Finalizado ({ $done } concluídos, { $failed } com falha):
progress-and-more = ... e mais { $count }
progress-active-downloads = Downloads ativos ({ $count }):
progress-completed-downloads = Downloads concluídos ({ $count }):
progress-starting = Iniciando...
progress-failed = Falhou.
progress-stage-ellipsis = { $stage }...

# -- system / settings --
system-title = Configurações
system-current-config = Configuração atual do sistema: { $path }
system-not-guix = Não está em um Guix System: { $error }
system-checking-config = Verificando a configuração atual do sistema...
system-no-config-banner = Nenhum arquivo de configuração do sistema detectado em /etc/config.scm ou /etc/system.scm. Informe abaixo o caminho da sua configuração .scm.
system-validate = Validar
system-config-heading = Configuração do sistema
system-config-blurb = Caminho para a sua configuração .scm editável do sistema.
system-config-placeholder = /home/you/dotfiles/config.scm
system-validation-empty = O caminho está vazio.
system-validation-missing = O caminho não existe: { $path }
system-validation-not-file = O caminho não é um arquivo comum: { $path }
system-validation-ok = OK: { $path }
system-load-paths-heading = Caminhos de carga extras
system-load-paths-blurb = Diretórios adicionais para procurar ao resolver imports do Scheme.
system-load-paths-auto = Automático: { $path }
system-load-paths-auto-unset = Automático: (defina a configuração do sistema acima)
system-load-paths-placeholder = /path/to/extra/modules
system-add = + Adicionar
system-section-system = SISTEMA
system-channels-heading = Canais
system-channels-blurb = Gerencie os canais de nível de usuário na aba dedicada.
system-channels-none = Nenhum canal configurado.
system-channels-configured = { $count ->
    [one] { $count } canal configurado.
   *[other] { $count } canais configurados.
}
system-channels-unknown = Canais configurados: —
system-open-channels = Abrir a aba Canais
system-channels-source-heading = Caminho de origem dos canais do usuário
system-channels-source-blurb = Substitui ~/.config/guix/channels.scm. Necessário quando o caminho padrão é gerenciado pelo `guix home` (resolve para /gnu/store).
system-channels-source-placeholder = /home/you/dotfiles/channels.scm (deixe vazio para o padrão)
system-use-default = Usar o padrão
system-section-user-channels = CANAIS DO USUÁRIO
system-metadata-heading = Ícones e capturas de tela
system-metadata-blurb = Busque ícones e capturas de tela em catálogos de terceiros para os resultados de busca selecionados. Opcional; requer acesso à rede.
system-metadata-enable = Ativar metadados de terceiros
system-metadata-flathub = Flathub (flathub.org)
system-metadata-debian = screenshots.debian.net
system-cache-heading = Cache
system-cache-blurb = Ícones e capturas de tela ficam salvos em disco por até um ano. Limpe o cache se um ícone parecer errado na origem.
system-cache-dir = Diretório de cache: { $path }
system-cache-dir-none = Diretório de cache: (nenhum diretório de cache XDG encontrado — usando apenas memória)
system-clear-cache = Limpar cache
system-clearing-cache = Limpando o cache...
system-cache-cleared = Cache limpo.
system-cache-clear-failed = Falha ao limpar o cache: { $error }
system-discovery-heading = Descoberta
system-discovery-toggle = Navegar por canais e pacotes de toys.whereis.social
system-discovery-blurb = Opcional. Requer acesso à rede. Quando desativado, a descoberta não aparece em lugar nenhum do app.
system-desktop-refresh = Atualizar o menu de aplicativos após instalar apps
system-desktop-refresh-desc = Reconstrói o menu da área de trabalho para que os aplicativos recém-instalados apareçam imediatamente (KDE, XFCE, MATE, LXQt). Desative se preferir atualizar manualmente.
system-section-metadata = METADADOS

# -- channels --
channels-title = Canais
channels-intro = Canais são fontes de pacotes para o Guix. Adicionar um canal permite instalar o software que ele oferece. Remover um significa que os pacotes dele param de receber atualizações.
channels-section-user = CANAIS DO USUÁRIO
channels-submode-installed = Instalados
channels-submode-discover = Descobrir
channels-default-path = ~/.config/guix/channels.scm (padrão)
channels-store-managed = gerenciado pelo store (somente leitura)
channels-writable = gravável
channels-file = Arquivo: { $path }
channels-confirm-restore = Confirmar restauração
channels-restore-last = Restaurar o último backup
channels-cant-edit-title = Este arquivo não pode ser editado aqui
channels-cant-edit-blurb = Seu channels.scm é gerenciado pelo `guix home` (ou outra ferramenta) e não pode ser editado diretamente. Para usar o guix-gui nas alterações de canais, defina um arquivo gravável em { $settings_tab } → { $channels_tab }.
channels-saving = Salvando...
channels-pull-then-install = Fazer pull e instalar { $pkg }
channels-pull-only = Só pull
channels-pull-now = Fazer pull agora
channels-keep-changes = Manter alterações
channels-pull-failed-shadow = O pull falhou — bug de sombreamento de canal (#74396).
channels-pull-failed = O pull falhou.
channels-rollback-blurb = Seu channels.scm tem as novas alterações, mas o Guix não conseguiu buscá-las. Restaurar o channels.scm anterior?
channels-rollback-none = Nenhum channels.scm anterior para restaurar.
channels-restore-previous = Restaurar o anterior
channels-empty-title = Nenhum channels.scm encontrado
channels-empty-blurb = Adicione um canal abaixo para criar um. O arquivo fica em ~/.config/guix/channels.scm por padrão.
channels-error = Erro
channels-loading = Carregando channels.scm...
channels-count = { $count ->
    [one] { $count } canal
   *[other] { $count } canais
}
channels-none-in-file = Nenhum canal neste arquivo.
channels-inherited-title = Também trazidos pelos seus canais
channels-inherited-blurb = Estes vêm dos canais acima e são gerenciados por eles.
channels-branch = branch: { $branch }
channels-commit = commit: { $commit }
channels-introduction = introdução: { $fpr }
channels-no-fingerprint = (sem fingerprint)
channels-introduction-none = introdução: (nenhuma)
channels-remove-title = Remover o canal `{ $name }`?
channels-remove-intro = { $count ->
    [one] { $count } pacote instalado vem deste canal:
   *[other] { $count } pacotes instalados vêm deste canal:
}
channels-remove-explainer = Eles continuarão funcionando, mas não receberão atualizações depois que o canal for removido.
channels-remove-cmd-label = Para desinstalá-los junto com o canal:
channels-remove-anyway = Remover o canal mesmo assim
channels-locked = bloqueado
channels-see-warning = veja o aviso abaixo
channels-confirm-remove = Confirmar remoção
channels-add-heading = Adicionar um canal
channels-add-blurb = Todos os campos são armazenados literalmente; o commit de introdução + fingerprint são obrigatórios.
channels-add-name = Nome
channels-add-name-placeholder = ex.: nonguix
channels-add-url = URL
channels-add-url-placeholder = https://gitlab.com/nonguix/nonguix
channels-add-branch = Branch
channels-add-branch-placeholder = master (opcional)
channels-add-commit = Commit
channels-add-commit-placeholder = hash do commit (opcional)
channels-add-intro-commit = Commit de introdução
channels-add-intro-commit-placeholder = hash do commit de introdução
channels-add-intro-fpr = Fingerprint de introdução
channels-add-intro-fpr-placeholder = fingerprint OpenPGP (ex.: 2A39 3FFF 68F4 EF7A 3D29 ...)
channels-add-btn = Adicionar canal
channels-discover-placeholder = Buscar pacotes ou canais...
channels-searching = Buscando...
channels-package-results = { $count ->
    [one] { $count } resultado de pacote
   *[other] { $count } resultados de pacote
}
channels-from = de { $channel }
channels-packages = Pacotes
channels-channels-heading = Canais
channels-no-synopsis = (sem sinopse)
channels-install = Instalar
channels-add-and-install = Adicionar canal e instalar
channels-loading-discover = Carregando canais...
channels-no-introduced = Nenhum canal introduzido foi retornado.
channels-available = { $count ->
    [one] { $count } canal disponível
   *[other] { $count } canais disponíveis
}
channels-already-added = já adicionado
channels-add = Adicionar
channels-pkgs = { $count ->
    [one] { $count } pkg
   *[other] { $count } pkgs
}
channels-svcs = { $count ->
    [one] { $count } svc
   *[other] { $count } svcs
}
channels-intro-dash = intro: —
channels-intro-short = intro: { $fpr }...
channels-set-writable-tooltip = Defina um arquivo gravável nas Configurações
channels-confirm-add-title = Confirmar adição de canal
channels-confirm-add-blurb = Isso vai acrescentar o canal ao seu channels.scm e validar o arquivo antes de salvar.
channels-provenance = Procedência
channels-supplied-by = Fornecido por { $source }
channels-trust-warning = Depois de adicionado, cada `guix pull` executa código Guile dessa origem como você. Verifique o commit de introdução e o fingerprint abaixo comparando com os valores publicados pelo próprio canal antes de adicionar.
channels-field-name = nome
channels-field-url = url
channels-field-branch = branch
channels-field-commit = commit
channels-field-intro-commit = commit de introdução
channels-field-intro-fpr = fingerprint de introdução

# -- channels status messages --
channels-restored = Restaurado do backup.
channels-added-install-prompt = Canal adicionado. Fazer pull e instalar { $pkg }?
channels-updated = Canais atualizados. Faça pull agora para buscar o novo catálogo.
channels-no-file-loaded = Nenhum channels.scm carregado; atualize a aba primeiro.
channels-store-managed-error = O channels.scm em { $path } é gerenciado pelo store. Defina nas Configurações um caminho de origem gravável que o substitua.
channels-no-backup = Nenhum arquivo de backup presente.
channels-form-name-required = O nome é obrigatório.
channels-form-url-required = A URL é obrigatória.
channels-form-intro-required = O commit de introdução e o fingerprint são obrigatórios.
channels-vanished-after-write = O channels.scm sumiu depois da gravação — atualize a aba.
