app-name = Loja de Aplicativos
back = Voltar
cancel = Cancelar
check-for-updates = Verificar por atualizações
checking-for-updates = Verificando por atualizações...
close = Fechar
install = Instalar
no-installed-applications = Nenhum aplicativo instalado.
no-updates = Todos os aplicativos instalados estão atualizados.
no-results = Nenhum resultado para "{ $search }".
notification-in-progress = Instalações e atualizações estão em andamento.
open = Abrir
see-all = Ver tudo
uninstall = Desinstalar
update = Atualizar
update-all = Atualizar tudo
place-on-desktop = Adicionar na área de trabalho
place-applet = Adicionar miniaplicativo
place-applet-desc = Escolha onde adicionar o miniaplicativo antes de ajustar sua posição.
panel = Painel
dock = Dock
place-and-refine = Adicionar e ajustar
# Codec dialog
codec-title = Instalar pacotes adicionais?
codec-header = "{ $application }" requer pacotes adicionais fornecendo "{ $description }".
codec-footer =
    O uso desses pacotes adicionais pode ser restrito em alguns países.
    Você deve verificar se uma das seguintes condições é verdadeira:
     • Essas restrições não se aplicam ao seu país de residência legal
     • Você tem permissão para usar este software (por exemplo, uma licença de patente)
     • Você está usando este software apenas para fins de pesquisa
codec-error = Ocorreram erros durante a instalação do pacote.
codec-installed = Os pacotes foram instalados com sucesso.
# Progress footer
details = Detalhes
dismiss = Dispensar mensagem
operations-running = { $running } operações em andamento ({ $percent }%)...
operations-running-finished = { $running } operações em andamento ({ $percent }%), { $finished } finalizadas...
# Repository add error dialog
repository-add-error-title = "Falha ao adicionar repositório"
# Repository remove dialog
repository-remove-title = Remover o repositório "{ $name }"?
repository-remove-body =
    Remover este repositório irá { $dependency ->
        [none] excluir
       *[other] remover "{ $dependency }" e excluir
    } os seguintes aplicativos e itens. Eles precisarão ser reinstalados se o repositório for adicionado novamente.
add = Adicionar
adding = Adicionando...
remove = Remover
removing = Removendo...
# Uninstall Dialog
uninstall-app = Desinstalar { $name }?
uninstall-app-warning = A desinstalação de { $name } apagará todos os dados do aplicativo.
# Nav Pages
explore = Explorar
create = Criação
work = Trabalho
develop = Desenvolvimento
learn = Educação
game = Jogos
relax = Áudio e Vídeo
socialize = Rede e Internet
utilities = Utilitários
applets = Miniaplicativos
installed-apps = Instalados
updates = Atualizações

## Applets page

enable-flathub-cosmic = Por favor, habilite o Flathub e o COSMIC Flatpak para ver os miniaplicativos disponíveis.
manage-repositories = Gerenciar repositórios
# Explore Pages
editors-choice = Escolha dos Editores
popular-apps = Aplicativos Populares
made-for-cosmic = Feitos para o COSMIC
new-apps = Novos Aplicativos
recently-updated = Atualizados Recentemente
development-tools = Ferramentas de Desenvolvimento
scientific-tools = Ferramentas Científicas
productivity-apps = Aplicativos de Produtividade
graphics-and-photography-tools = Ferramentas Gráficas e de Fotografia
social-networking-apps = Aplicativos de Rede e Internet
games = Jogos
music-and-video-apps = Aplicativos de Música e Vídeo
apps-for-learning = Aplicativos Educacionais
# Details Page
source-installed = { $source } (instalado)
developer = Desenvolvedor
app-developers = Desenvolvedores de { $app }
monthly-downloads = Downloads Mensais
licenses = Licenças
proprietary = Proprietário

## App URLs

bug-tracker = Relatar um problema
contact = Contato
donation = Fazer uma doação
faq = Perguntas frequentes
help = Ajuda
homepage = Site do projeto
translate = Contribuir com tradução

# Context Pages


## Operations

cancelled = Canceladas
operations = Operações
no-operations = Nenhuma operação no histórico.
pending = Pendentes
failed = Com falha
complete = Concluídas

## Settings

settings = Configurações

## Release notes

latest-version = Última versão
no-description = Nenhuma descrição disponível.

## Repositories

recommended-flatpak-sources = Fontes Recomendadas de Flatpak
custom-flatpak-sources = Fontes Flatpak Customizadas
import-flatpakrepo = Importar um arquivo .flatpakrepo para adicionar uma fonte customizada
no-custom-flatpak-sources = Nenhuma fonte customizada de Flatpak
import = Importar
no-flatpak = Sem suporte flatpak
software-repositories = Repositórios de Software

### Appearance

appearance = Aparência
theme = Tema
match-desktop = Estilo do sistema
dark = Estilo escuro
light = Estilo claro
addons = Complementos
view-more = Ver mais
delete-app-data = Apagar permanentemente os dados do aplicativo
uninstall-app-flatpak-warning = A desinstalação de { $name } manterá seus documentos e dados.
version = Versão { $version }
system-package-updates = Atualizações de pacotes
system-packages-summary =
    { $count ->
        [one] { $count } pacote
       *[other] { $count } pacotes
    }
system-packages = Pacotes do Sistema
flatpak-runtimes = Runtimes Flatpak
