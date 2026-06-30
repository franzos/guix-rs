app-title = Guix GUI

tab-home = Inicio
tab-search = Buscar
tab-installed = Instalados
tab-updates = Actualizaciones
tab-channels = Canales
tab-system = Ajustes
tab-about = Acerca de

settings-language = Idioma
settings-language-system = Predeterminado del sistema

# -- shared --
common-refresh = Actualizar
common-cancel = Cancelar
common-close = Cerrar
common-dismiss = Descartar
common-remove = Eliminar
common-open-settings = Abrir ajustes

# -- app shell / overlay --
app-discover-failed = No se pudo detectar guix
app-discovering = Detectando guix...
app-brand = Guix
app-lightbox-close = Cerrar (Esc)
app-lightbox-no-image = sin imagen / error
app-running = En ejecución...
app-copy = Copiar
app-show-log = Mostrar registro
app-hide-log = Ocultar registro
# Status label shown in the progress overlay.
app-done = Listo.
app-failed-exit = Falló (salida { $code }).
app-ended-no-summary = Terminó sin resumen de salida.
app-op-title = { $label } (op #{ $id })
app-cancel-pkexec-tooltip = No se pueden cancelar operaciones privilegiadas — el kernel no permite enviar señales a procesos de root. Espera a que termine.
app-bootstrap-help =
    La reconfiguración falló: el Guix del sistema en ejecución no reconoce un módulo
    que importa tu configuración. Esto suele significar que un canal se actualizó
    después de tu última reconfiguración y el módulo nuevo aún no está integrado en
    el Guix del sistema.

    Inicialízalo una vez de forma manual:

        sudo guix system reconfigure -L { $load } { $cfg }

    Después de eso, este botón funcionará para las siguientes actualizaciones.
app-set-source-config-first = Primero define la ruta de configuración de origen en la pestaña Sistema.
app-op-start-failed = No se pudo iniciar la operación: { $error }
app-restore-panicked = La tarea de restauración entró en pánico: { $detail }
app-restore-failed = La restauración falló: { $detail }
app-discovery-client-failed = El cliente de descubrimiento falló: { $detail }

# -- op kinds --
op-install = Instalando
op-remove = Eliminando
op-upgrade = Actualizando paquetes de usuario
op-pull = Obteniendo catálogo de usuario
op-system-pull = Obteniendo catálogo del sistema
op-reconfigure = Reconfigurando el sistema

# -- stages --
stage-starting = Iniciando
stage-channel-update = Actualizando canales
stage-computing-deriv = Calculando derivación
stage-downloading = Descargando
stage-building = Compilando
stage-profile = Actualizando perfil
# Status label shown in the progress overlay.
stage-done = Listo
# Status label shown in the progress overlay.
stage-failed = Falló
stage-build-failed = La compilación falló: { $name }
stage-build-failed-log = La compilación falló: { $name } (registro: { $log })

# -- categories --
category-graphics = Gráficos
category-audio-video = Audio y vídeo
category-office = Oficina
category-development = Desarrollo
category-engineering = Ingeniería
category-internet = Internet
category-game = Juegos

# -- home --
home-title = Inicio
home-subtitle = Un punto de partida — aplicaciones conocidas disponibles en Guix. Abre una para instalarla, o usa Buscar para el catálogo completo.
home-installed-badge = instalado

# -- search --
search-title = Buscar
search-placeholder = Buscar paquetes...
search-loading-catalog = Cargando catálogo de paquetes...
search-searching = Buscando...
search-results = { $count ->
    [one] { $count } resultado
   *[other] { $count } resultados
}
search-truncated = Mostrando los primeros { $shown } de ≥{ $total } { $total ->
    [one] coincidencia
   *[other] coincidencias
}; refina tu búsqueda.
search-error-label = Error de búsqueda:
search-copy-details = Copiar detalles
search-select-prompt = Selecciona un paquete para ver sus detalles.
search-homepage = página web:
search-license = licencia: { $license }
search-outputs = salidas: { $outputs }
search-install = Instalar
search-remove = Eliminar
search-screenshots-flathub = Capturas vía Flathub ({ $id })
search-screenshots-debian = Capturas vía screenshots.debian.net
search-loading-media = Cargando iconos / capturas...
search-failed = La búsqueda falló.

# -- installed --
installed-title = Instalados
installed-count = { $count ->
    [one] { $count } paquete instalado
   *[other] { $count } paquetes instalados
}
installed-loading = Cargando...
installed-error = Error: { $error }

# -- updates --
updates-title = Actualizaciones
updates-your-packages-blurb = Gestiona tus paquetes a nivel de usuario.
updates-your-packages = Tus paquetes
updates-fetch-latest = Obtener catálogo más reciente
updates-update-my-packages = Actualizar mis paquetes
updates-system-blurb = Aplica la configuración de tu sistema. Requiere autenticación de administrador.
updates-system = Sistema
updates-source-config = Configuración de origen: { $path }
updates-source-config-unset = Configuración de origen: (sin definir — abre Ajustes para elegir)
updates-fetch-system = Obtener catálogo del sistema
updates-update-system = Actualizar el sistema
# DO NOT TRANSLATE — literal shell command shown as a tooltip
updates-update-system-tip = pkexec guix system reconfigure
updates-confirm-reconfigure = Confirmar reconfiguración del sistema
updates-reconfigure-blurb = Ejecutando como root vía pkexec. Verifica las rutas de abajo — cada una la cargará Guile con privilegios de root.
updates-config = Configuración:
updates-load-paths-none = Rutas de carga (-L): (ninguna)
updates-load-paths = { $count ->
    [one] Rutas de carga (-L), { $count } entrada:
   *[other] Rutas de carga (-L), { $count } entradas:
}
updates-confirm-reconfigure-btn = Confirmar reconfiguración
updates-loading-channels = Cargando canales...
updates-error-channels = Error al cargar los canales: { $error }
updates-last-pulled = Última obtención: hace { $age }.
updates-last-pulled-never = Última obtención: nunca.
updates-channels-none = Canales: (ninguno detectado).
updates-channels = Canales: { $list }.
updates-channel-no-commit = (sin commit)
updates-last-pulled-root = Última obtención (root): hace { $age }.
updates-last-pulled-root-never = Última obtención (root): nunca.
updates-last-reconfigured = Última reconfiguración: hace { $age }.
updates-last-reconfigured-never = Última reconfiguración: nunca (¿no es un host con Guix System?).

# -- updates: privileged help card --
updates-privileged-help-heading = Se requiere acción de administrador
updates-privileged-help-no-agent = No se detectó ningún agente de autenticación polkit. Puede que no aparezca una solicitud de contraseña. Si no aparece, inicie el agente polkit de su escritorio o ejecute el comando equivalente de abajo en una terminal.
updates-privileged-help-failed = Esta operación privilegiada no pudo completarse: { $error }. Ejecute en su lugar el comando equivalente de abajo en una terminal.
updates-privileged-help-failure-generic = fallo de autenticación o de un paso privilegiado
updates-privileged-help-cmd-label = Ejecute esto manualmente en una terminal:
# DO NOT TRANSLATE: literal shell command
updates-privileged-help-cmd-pull = sudo guix pull
# DO NOT TRANSLATE: literal shell command
updates-privileged-help-cmd-reconfigure = sudo guix system reconfigure -L { $load } { $cfg }

# -- about --
about-title = Acerca de
about-version = Versión { $version }
about-tagline = Interfaz de escritorio para el gestor de paquetes Guix.
about-authors = Autores
about-source = Código fuente y contribuciones
about-source-blurb = Se agradecen los informes de errores y los pull requests.
about-license = Licencia
about-license-line = Guix GUI se publica bajo la Licencia Pública General de GNU v3.0.
about-license-detail = Puedes redistribuirlo y modificarlo según los términos de esa licencia. Consulta el archivo LICENSE en el repositorio para el texto completo.
about-third-party = Datos de terceros
about-third-party-blurb = Los iconos y capturas de las aplicaciones se obtienen de servicios externos cuando activas los metadatos de terceros en Ajustes. Las marcas, iconos y capturas siguen siendo propiedad de sus respectivos proyectos.
about-channel-discovery = Descubrimiento de canales
about-channel-discovery-blurb = El submodo Descubrir de la pestaña Canales explora canales y paquetes de Guix indexados por toys.whereis.social. Es opcional; requiere conexión. El catálogo y sus contribuyentes siguen siendo propiedad de sus respectivos proyectos.
about-built-with = Construido con
about-built-with-detail = Las licencias de cada crate se enumeran en sus respectivos repositorios.

# -- progress overlay --
progress-last = Último: { $line }
progress-running = En ejecución ({ $count }):
progress-counts = { $built }/{ $started } compilados, { $dl_done }/{ $dl_started } descargados ({ $mb } MB)
progress-build-line = { "  - " }{ $name } [building]
progress-build-item = { "  - " }{ $status ->
    [done] { $name } [done]
    [failed] { $name } [FAILED]
   *[other] { $name } [building]
}
progress-finished = Terminado ({ $done } completados, { $failed } fallidos):
progress-and-more = ... y { $count } más
progress-active-downloads = Descargas activas ({ $count }):
progress-completed-downloads = Descargas completadas ({ $count }):
progress-starting = Iniciando...
progress-failed = Falló.
progress-stage-ellipsis = { $stage }...

# -- system / settings --
system-title = Ajustes
system-current-config = Configuración actual del sistema: { $path }
system-not-guix = No estás en Guix System: { $error }
system-checking-config = Comprobando la configuración actual del sistema...
system-no-config-banner = No se detectó ningún archivo de configuración del sistema en /etc/config.scm ni /etc/system.scm. Introduce abajo la ruta a tu configuración .scm.
system-validate = Validar
system-config-heading = Configuración del sistema
system-config-blurb = Ruta a tu configuración .scm editable del sistema.
system-config-placeholder = /home/you/dotfiles/config.scm
system-validation-empty = La ruta está vacía.
system-validation-missing = La ruta no existe: { $path }
system-validation-not-file = La ruta no es un archivo regular: { $path }
system-validation-ok = OK: { $path }
system-load-paths-heading = Rutas de carga adicionales
system-load-paths-blurb = Directorios adicionales donde buscar al resolver importaciones de Scheme.
system-load-paths-auto = Auto: { $path }
system-load-paths-auto-unset = Auto: (define arriba la configuración del sistema)
system-load-paths-placeholder = /path/to/extra/modules
system-add = + Añadir
system-section-system = SISTEMA
system-channels-heading = Canales
system-channels-blurb = Gestiona los canales a nivel de usuario en la pestaña dedicada.
system-channels-none = No hay canales configurados.
system-channels-configured = { $count ->
    [one] { $count } canal configurado.
   *[other] { $count } canales configurados.
}
system-channels-unknown = Canales configurados: —
system-open-channels = Abrir la pestaña Canales
system-channels-source-heading = Ruta de origen de los canales de usuario
system-channels-source-blurb = Anulación de ~/.config/guix/channels.scm. Necesaria cuando la ruta predeterminada la gestiona `guix home` (se resuelve en /gnu/store).
system-channels-source-placeholder = /home/you/dotfiles/channels.scm (vacío para el valor predeterminado)
system-use-default = Usar predeterminado
system-section-user-channels = CANALES DE USUARIO
system-metadata-heading = Iconos y capturas
system-metadata-blurb = Obtén iconos y capturas de catálogos de terceros para resultados de búsqueda seleccionados. Es opcional; requiere acceso a la red.
system-metadata-enable = Activar metadatos de terceros
system-metadata-flathub = Flathub (flathub.org)
system-metadata-debian = screenshots.debian.net
system-cache-heading = Caché
system-cache-blurb = Los iconos y capturas se guardan en disco hasta un año. Vacíala si un icono se ve mal en el origen.
system-cache-dir = Directorio de caché: { $path }
system-cache-dir-none = Directorio de caché: (no se encontró directorio de caché XDG — solo en memoria)
system-clear-cache = Vaciar caché
system-clearing-cache = Vaciando caché...
system-cache-cleared = Caché vaciada.
system-cache-clear-failed = No se pudo vaciar la caché: { $error }
system-discovery-heading = Descubrimiento
system-discovery-toggle = Explorar canales y paquetes de toys.whereis.social
system-discovery-blurb = Opcional. Requiere acceso a la red. Cuando está desactivado, el descubrimiento no aparece en ninguna parte de la app.
system-desktop-refresh = Actualizar el menú de aplicaciones tras instalar apps
system-desktop-refresh-desc = Reconstruye el menú del escritorio para que las aplicaciones recién instaladas aparezcan de inmediato (KDE, XFCE, MATE, LXQt). Desactívalo si prefieres actualizar manualmente.
system-section-metadata = METADATOS

# -- channels --
channels-title = Canales
channels-intro = Los canales son fuentes de paquetes para Guix. Añadir un canal te permite instalar el software que ofrece. Eliminar uno significa que sus paquetes dejan de recibir actualizaciones.
channels-section-user = CANALES DE USUARIO
channels-submode-installed = Instalados
channels-submode-discover = Descubrir
channels-default-path = ~/.config/guix/channels.scm (predeterminado)
channels-store-managed = gestionado por el store (solo lectura)
channels-writable = editable
channels-file = Archivo: { $path }
channels-confirm-restore = Confirmar restauración
channels-restore-last = Restaurar última copia de seguridad
channels-cant-edit-title = Este archivo no se puede editar aquí
channels-cant-edit-blurb = Tu channels.scm lo gestiona `guix home` (u otra herramienta) y no se puede editar directamente. Para usar guix-gui para cambios de canales, define un archivo editable en { $settings_tab } → { $channels_tab }.
channels-saving = Guardando...
channels-pull-then-install = Obtener y luego instalar { $pkg }
channels-pull-only = Solo obtener
channels-pull-now = Obtener ahora
channels-keep-changes = Conservar cambios
channels-pull-failed-shadow = La obtención falló — error de sombra de canal (#74396).
channels-pull-failed = La obtención falló.
channels-rollback-blurb = Tu channels.scm tiene los cambios nuevos pero Guix no pudo obtenerlos. ¿Restaurar el channels.scm anterior?
channels-rollback-none = No hay channels.scm anterior que restaurar.
channels-restore-previous = Restaurar anterior
channels-empty-title = No se encontró ningún channels.scm
channels-empty-blurb = Añade un canal abajo para crear uno. El archivo se ubica en ~/.config/guix/channels.scm de forma predeterminada.
channels-error = Error
channels-loading = Cargando channels.scm...
channels-count = { $count ->
    [one] { $count } canal
   *[other] { $count } canales
}
channels-none-in-file = No hay canales en este archivo.
channels-inherited-title = También incorporados por tus canales
channels-inherited-blurb = Estos provienen de los canales de arriba y son gestionados por ellos.
channels-branch = rama: { $branch }
channels-commit = commit: { $commit }
channels-introduction = introducción: { $fpr }
channels-no-fingerprint = (sin huella)
channels-introduction-none = introducción: (ninguna)
channels-remove-title = ¿Eliminar el canal `{ $name }`?
channels-remove-intro = { $count ->
    [one] { $count } paquete instalado proviene de este canal:
   *[other] { $count } paquetes instalados provienen de este canal:
}
channels-remove-explainer = Estos seguirán funcionando pero no recibirán actualizaciones tras eliminar el canal.
channels-remove-cmd-label = Para desinstalarlos junto con el canal:
channels-remove-anyway = Eliminar el canal de todos modos
channels-locked = bloqueado
channels-see-warning = ver advertencia abajo
channels-confirm-remove = Confirmar eliminación
channels-add-heading = Añadir un canal
channels-add-blurb = Todos los campos se almacenan tal cual; el commit de introducción y la huella son obligatorios.
channels-add-name = Nombre
channels-add-name-placeholder = e.g. nonguix
channels-add-url = URL
channels-add-url-placeholder = https://gitlab.com/nonguix/nonguix
channels-add-branch = Rama
channels-add-branch-placeholder = master (opcional)
channels-add-commit = Commit
channels-add-commit-placeholder = hash del commit (opcional)
channels-add-intro-commit = Commit de introducción
channels-add-intro-commit-placeholder = hash del commit de introducción
channels-add-intro-fpr = Huella de introducción
channels-add-intro-fpr-placeholder = OpenPGP fingerprint (e.g. 2A39 3FFF 68F4 EF7A 3D29 ...)
channels-add-btn = Añadir canal
channels-discover-placeholder = Buscar paquetes o canales...
channels-searching = Buscando...
channels-package-results = { $count ->
    [one] { $count } resultado de paquete
   *[other] { $count } resultados de paquetes
}
channels-from = de { $channel }
channels-packages = Paquetes
channels-channels-heading = Canales
channels-no-synopsis = (sin sinopsis)
channels-install = Instalar
channels-add-and-install = Añadir canal e instalar
channels-loading-discover = Cargando canales...
channels-no-introduced = No se devolvió ningún canal con introducción.
channels-available = { $count ->
    [one] { $count } canal disponible
   *[other] { $count } canales disponibles
}
channels-already-added = ya añadido
channels-add = Añadir
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
channels-set-writable-tooltip = Define un archivo editable en Ajustes
channels-confirm-add-title = Confirmar añadir canal
channels-confirm-add-blurb = Esto añadirá el canal a tu channels.scm y validará el archivo antes de guardar.
channels-provenance = Procedencia
channels-supplied-by = Proporcionado por { $source }
channels-trust-warning = Una vez añadido, cada `guix pull` ejecuta código Guile de este origen como tú. Verifica abajo el commit de introducción y la huella frente a los valores publicados por el propio canal antes de añadirlo.
channels-field-name = nombre
channels-field-url = url
channels-field-branch = rama
channels-field-commit = commit
channels-field-intro-commit = commit de introducción
channels-field-intro-fpr = huella de introducción

# -- channels status messages --
channels-restored = Restaurado desde la copia de seguridad.
channels-added-install-prompt = Canal añadido. ¿Obtener y luego instalar { $pkg }?
channels-updated = Canales actualizados. Obtén ahora para descargar el nuevo catálogo.
channels-no-file-loaded = No hay ningún channels.scm cargado; actualiza la pestaña primero.
channels-store-managed-error = El channels.scm en { $path } lo gestiona el store. Define una anulación de ruta de origen editable en Ajustes.
channels-no-backup = No hay ningún archivo de copia de seguridad.
channels-form-name-required = El nombre es obligatorio.
channels-form-url-required = La URL es obligatoria.
channels-form-intro-required = El commit de introducción y la huella son obligatorios.
channels-vanished-after-write = channels.scm desapareció tras la escritura — actualiza la pestaña.
