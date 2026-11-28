//! Labels shared by the native navigation and Start screen.

use super::App;

#[derive(Clone, Copy)]
pub(super) struct Locale {
    spanish: bool,
}

impl App {
    pub(super) fn locale(&self) -> Locale {
        Locale { spanish: self.settings.string("language", "en") == "es" }
    }
}

impl Locale {
    /// "Open Repo in {editor}", with the editor's own name kept as it is.
    pub(super) fn open_in_editor(self, editor: &str) -> String {
        if self.spanish { format!("Abrir repositorio en {editor}") } else { format!("Open Repo in {editor}") }
    }

    pub(super) fn text(self, label: &str) -> &str {
        if !self.spanish {
            return label;
        }
        match label {
            "File" => "Archivo",
            "Edit" => "Editar",
            "View" => "Ver",
            "Help" => "Ayuda",
            "New Tab" => "Nueva pestaña",
            "Close Tab" => "Cerrar pestaña",
            "Reopen Closed Tab" => "Reabrir pestaña cerrada",
            "Clone Repo..." => "Clonar repositorio…",
            "Init Repo..." => "Inicializar repositorio…",
            "Open Repo..." => "Abrir repositorio…",
            "Open Repo in New Window…" => "Abrir repositorio en otra ventana…",
            "Open Repo Management" => "Gestionar repositorios",
            "Open External Terminal" => "Abrir terminal externa",
            "Open in File Manager" => "Abrir en el gestor de archivos",
            "File History / Blame..." => "Historial y autoría del archivo…",
            "Preferences..." => "Preferencias…",
            "Check for Updates" => "Buscar actualizaciones",
            "Sign Out" => "Cerrar sesión",
            "Exit" => "Salir",
            "Undo" => "Deshacer",
            "Redo" => "Rehacer",
            "Show Left Panel" => "Mostrar panel izquierdo",
            "Show Commit Details Panel" => "Mostrar detalles del commit",
            "Commit Details Below Graph" => "Detalles del commit debajo del gráfico",
            "Move to bottom" => "Mover abajo",
            "Move to right" => "Mover a la derecha",
            "Show Toolbar Labels" => "Mostrar etiquetas de la barra",
            "Show Menus in Title Bar" => "Mostrar menús en la barra de título",
            "Show Terminal Panel" => "Mostrar terminal",
            "Increase Zoom" => "Acercar",
            "Decrease Zoom" => "Alejar",
            "Reset Zoom" => "Restablecer zoom",
            "Toggle Full Screen" => "Alternar pantalla completa",
            "Activity Log" => "Registro de actividad",
            "Command Palette" => "Paleta de comandos",
            "Keyboard Shortcuts" => "Atajos de teclado",
            "Feature Plan" => "Plan de desarrollo",
            "Copy diagnostics" => "Copiar diagnóstico",
            "View Activity Logs" => "Ver registro de actividad",
            "View Release Notes" => "Ver notas de la versión",
            "Join Discord" => "Unirse a Discord",
            "About GitCraft" => "Acerca de GitCraft",
            "Repositories" => "Repositorios",
            "Open" => "Abrir",
            "Clone" => "Clonar",
            "Create" => "Crear",
            "Search repositories" => "Buscar repositorios",
            "Favorites" => "Favoritos",
            "Recent" => "Recientes",
            "Repository Management" => "Gestionar repositorios",
            "No recent repositories yet." => "Todavía no hay repositorios recientes.",
            "Preparing the sample project…" => "Preparando el proyecto de ejemplo…",
            "Try the sample project" => "Probar el proyecto de ejemplo",
            "Connect More Integrations" => "Conectar más integraciones",
            "Connect Integrations" => "Conectar integraciones",
            "Connect a hosting or issue tracking account to see its pull requests and issues in the left panel and Launchpad." => "Conecta una cuenta de alojamiento o seguimiento para ver sus solicitudes de cambio e incidencias en el panel izquierdo y en Launchpad.",
            "Workspaces" => "Espacios de trabajo",
            "Manage workspaces" => "Gestionar espacios de trabajo",
            "Create a workspace" => "Crear un espacio de trabajo",
            "Resources" => "Recursos",
            "Preparing…" => "Preparando…",
            "Intro Tutorials" => "Tutoriales iniciales",
            "Release Notes" => "Notas de la versión",
            "Documentation" => "Documentación",
            "Pull" => "Traer",
            "Fetch" => "Obtener",
            "Fetch All" => "Obtener de todos los remotos",
            "Pull options" => "Opciones de pull",
            "Pull (fast-forward if possible)" => "Pull (avance rápido si es posible)",
            "Pull (fast-forward only)" => "Pull (solo avance rápido)",
            "Pull (rebase)" => "Pull (rebase)",
            "Pull is not available when in a detached HEAD state" => "Pull no está disponible con HEAD separado",
            "Select the default pull/fetch operation to execute when clicking this button" => "Selecciona la operación pull/fetch predeterminada que se ejecutará al pulsar este botón",
            "Set as default" => "Establecer como predeterminada",
            "Push" => "Enviar",
            "Branch" => "Rama",
            "Stash" => "Guardar",
            "Pop" => "Recuperar",
            "Terminal" => "Terminal",
            "Stash options" => "Opciones de almacenamiento temporal",
            _ => label,
        }
    }
}
