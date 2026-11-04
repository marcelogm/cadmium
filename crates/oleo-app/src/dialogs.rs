use adw::prelude::*;
use gtk::glib;

use crate::application::ACCELERATORS;
use crate::tools::UiTool;
use crate::window::TOOL_KEYS;

const APPLICATION_ICON: &str = "applications-graphics-symbolic";
const PIGMENT_DATA_CREDIT: &str = "Espectros provisórios, ajustados à mão a partir da aparência conhecida de cada pigmento; não são medições";

pub const ACTION_SHORTCUTS: [(&str, &[(&str, &str)]); 3] = [
    (
        "Pintura",
        &[
            ("Desfazer", "win.undo"),
            ("Refazer", "win.redo"),
            ("Secar", "win.dry"),
        ],
    ),
    (
        "Arquivo",
        &[
            ("Nova pintura", "win.new"),
            ("Abrir", "win.open"),
            ("Salvar", "win.save"),
            ("Salvar como", "win.save-as"),
            ("Exportar PNG", "win.export"),
            ("Sair", "app.quit"),
        ],
    ),
    (
        "Vista",
        &[
            ("Aproximar", "win.zoom-in"),
            ("Afastar", "win.zoom-out"),
            ("Ajustar à janela", "win.zoom-fit"),
            ("Mostrar ou ocultar barra lateral", "win.toggle-sidebar"),
            ("Atalhos de teclado", "app.shortcuts"),
        ],
    ),
];

pub fn action_accelerator(action: &str) -> Option<String> {
    ACCELERATORS
        .iter()
        .find(|(name, _)| *name == action)
        .map(|(_, accels)| accels.join(" "))
}

pub fn tool_accelerator(tool: UiTool) -> String {
    TOOL_KEYS
        .iter()
        .filter(|(_, bound)| *bound == tool)
        .filter_map(|(key, _)| key.name())
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn about_dialog() -> adw::AboutDialog {
    let about = adw::AboutDialog::builder()
        .application_name("Óleo")
        .application_icon(APPLICATION_ICON)
        .version(env!("CARGO_PKG_VERSION"))
        .comments("Simulador de pintura a óleo com pigmentos espectrais")
        .developer_name("Marcelo")
        .license_type(gtk::License::Unknown)
        .build();
    about.add_credit_section(Some("Dados de pigmentos"), &[PIGMENT_DATA_CREDIT]);
    about
}

pub fn shortcuts_dialog() -> adw::ShortcutsDialog {
    let dialog = adw::ShortcutsDialog::new();
    let tools = adw::ShortcutsSection::new(Some("Ferramentas"));
    for tool in UiTool::ALL {
        tools.add(adw::ShortcutsItem::new(
            tool.label(),
            &tool_accelerator(tool),
        ));
    }
    dialog.add(tools);
    for (title, entries) in ACTION_SHORTCUTS {
        let section = adw::ShortcutsSection::new(Some(title));
        for (label, action) in entries {
            if let Some(accelerator) = action_accelerator(action) {
                section.add(adw::ShortcutsItem::new(label, &accelerator));
            }
        }
        dialog.add(section);
    }
    dialog
}

pub fn confirm_discard(parent: &impl IsA<gtk::Widget>, on_discard: impl Fn() + 'static) {
    let alert = adw::AlertDialog::builder()
        .heading("Descartar a pintura atual?")
        .body("As alterações que não foram salvas serão perdidas.")
        .default_response("cancel")
        .close_response("cancel")
        .build();
    alert.add_responses(&[("cancel", "Cancelar"), ("discard", "Descartar")]);
    alert.set_response_appearance("discard", adw::ResponseAppearance::Destructive);
    alert.choose(
        Some(parent),
        None::<&gtk::gio::Cancellable>,
        move |response: glib::GString| {
            if response == "discard" {
                on_discard();
            }
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_accelerator_is_listed_exactly_once() {
        for (action, _) in ACCELERATORS {
            let listed = ACTION_SHORTCUTS
                .iter()
                .flat_map(|(_, entries)| entries.iter())
                .filter(|(_, listed)| *listed == action)
                .count();
            assert_eq!(listed, 1, "{action}");
        }
    }

    #[test]
    fn every_listed_action_has_an_accelerator() {
        for (_, entries) in ACTION_SHORTCUTS {
            for (label, action) in entries {
                assert!(action_accelerator(action).is_some(), "{label}: {action}");
            }
        }
    }

    #[test]
    fn every_tool_key_is_listed() {
        for tool in UiTool::ALL {
            assert!(!tool_accelerator(tool).is_empty(), "{tool:?}");
        }
    }

    #[test]
    fn rag_lists_both_of_its_keys() {
        assert_eq!(
            tool_accelerator(UiTool::Paint(crate::tools::Tool::Rag)),
            "4 r"
        );
    }

    #[test]
    fn zoom_in_lists_both_of_its_accelerators() {
        assert_eq!(
            action_accelerator("win.zoom-in").as_deref(),
            Some("<Control>plus <Control>equal")
        );
    }
}
