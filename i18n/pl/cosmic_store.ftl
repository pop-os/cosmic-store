app-name = Sklep COSMIC
comment = Sklep z aplikacjami pulpitu COSMIC
keywords = Aplikacje;Soft;Oprogramowanie;Sklep;
back = Powrót
cancel = Anuluj
check-for-updates = Sprawdź aktualizacje
checking-for-updates = Sprawdzam aktualizacje…
close = Zamknij
install = Zainstaluj
no-installed-applications = Brak zainstalowanych aplikacji.
no-updates = Wszystkie zainstalowane aplikacje są aktualne.
no-results = Brak wyników dla „{ $search }”.
notification-in-progress = Instalacje i aktualizacje w toku.
open = Otwórz
see-all = Zobacz wszystkie
uninstall = Odinstaluj
update = Zaktualizuj
update-all = Zaktualizuj wszystkie
place-on-desktop = Umieść na pulpicie
place-applet = Umieść aplet
place-applet-desc = Wybierz gdzie umieścić aplet przed ostatecznym umiejscowieniem.
panel = Panel
dock = Dok
place-and-refine = Umieść i uwydatnij
# Codec dialog
codec-title = Zainstalować dodatkowe pakiety?
codec-header = „{ $application }” wymaga dodatkowych pakietów zapewniających „{ $description }”.
codec-footer =
    Użycie tych pakietów może być obłożone obostrzeniami w niektórych państwach.
    Musisz stwierdzić, że jedno z poniższych jest prawdziwe:
     • Te obostrzenia nie obowiązują w twoim kraju zamieszkania
     • Masz pozwolenie na użycie tego oprogramowania (na przykład licencję patentową)
     • Używasz tego oprogramowania tylko do celów badawczych
codec-error = Wystąpiły błędy podczas instalacji pakietów.
codec-installed = Pakiety zostały zainstalowane.
# Progress footer
details = Detale
dismiss = Odrzuć wiadomość
operations-running = { $running } bieżące działania ({ $percent }%)...
operations-running-finished = { $running } bieżące działania ({ $percent }%), { $finished } ukończone…
# Repository add error dialog
repository-add-error-title = „Nieudane dodanie repozytorium”
# Repository remove dialog
repository-remove-title = Usunąć „{ $name }” repozytorium?
repository-remove-body =
    Usuwając to repozytorium { $dependency ->
        [none] usuniesz
       *[other] usuniesz „{ $dependency }” i usuniesz
    } następujące aplikacje i elementy. Będą one musiały być zainstalowane ponownie jeśli to repozytorium zostanie ponownie dodane.
add = Dodaj
adding = Dodawanie…
remove = Usuń
removing = Usuwanie…
# Uninstall Dialog
uninstall-app = Odinstalować { $name }?
uninstall-app-warning = Odinstalowanie { $name } usunie dane programu.
# Nav Pages
explore = Odkrywaj
create = Utwórz
work = Praca
develop = Programistyczne
learn = Nauka
game = Gry
relax = Relaks
socialize = Społeczne
utilities = Użytkowe
applets = Aplety
installed-apps = Zainstalowane aplikacje
updates = Aktualizacje

## Applets page

enable-flathub-cosmic = Musisz włączyć Flathub oraz COSMIC Flatpak by widzieć dostępne aplety.
manage-repositories = Zarządzanie repozytoriami
# Explore Pages
editors-choice = Wybór redakcji
popular-apps = Popularne aplikacje
made-for-cosmic = Stworzone dla COSMIC
new-apps = Nowe aplikacje
recently-updated = Ostatnio zaktualizowane
development-tools = Narzędzia programistyczne
scientific-tools = Narzędzia naukowe
productivity-apps = Aplikacje zwiększające produktywność
graphics-and-photography-tools = Narzędzia graficzne i fotograficzne
social-networking-apps = Aplikacje mediów społecznościowych
games = Gry
music-and-video-apps = Aplikacje muzyczne i wideo
apps-for-learning = Aplikacje do nauki
# Details Page
source-installed = { $source } (zainstalowane)
developer = Wydawca
app-developers = Wydawca { $app }
monthly-downloads = Miesięczne pobrania z Flathub
licenses = Licencje
proprietary = Własnościowa

## App URLs

bug-tracker = Śledzenie błędów
contact = Kontakt
donation = Darowizny
faq = Często zadawane pytania
help = Pomoc
homepage = Strona domowa
translate = Tłumaczenie

# Context Pages


## Operations

cancelled = Anulowano
operations = Działania
no-operations = Brak działań w historii.
pending = Oczekujące
failed = Nieudane
complete = Ukończone

## Settings

settings = Ustawienia

## Release notes

latest-version = Najnowsza wersja
no-description = Brak opisu.

## Repositories

recommended-flatpak-sources = Polecane źródła Flatpak
custom-flatpak-sources = Własne źródła Flatpak
import-flatpakrepo = Importuj plik .flatpakrepo by dodać własne źródła
no-custom-flatpak-sources = Brak własnych źródeł Flatpak
import = Importuj
no-flatpak = Flatpak nie jest wspierany
software-repositories = Repozytoria oprogramowania

### Appearance

appearance = Wygląd
theme = Motyw
match-desktop = Dopasuj do Pulpitu
dark = Ciemny
light = Jasny
addons = Rozszerzenia
view-more = Zobacz więcej
version = Wersja { $version }
delete-app-data = Definitywnie usuń dane aplikacji
uninstall-app-flatpak-warning = Odinstalowanie { $name } nie usunie danych i dokumentów.
system-package-updates = Aktualizacje pakietów
system-packages-summary =
    { $count ->
        [one] { $count } pakiet
        [few] { $count } pakiety
       *[other] { $count } pakietów
    }
system-packages = Pakiety Systemowe
flatpak-runtimes = Środowiska Uruchomieniowe Flatpak
