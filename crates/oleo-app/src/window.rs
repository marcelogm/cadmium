use std::cell::Cell;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gdk, gio, glib};

use crate::tools::{Tool, UiTool};

const LOG_DOMAIN: &str = "oleo";

mod imp {
    use super::*;

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/marcelo/Oleo/window.ui")]
    pub struct OleoWindow {
        #[template_child]
        pub split_view: TemplateChild<adw::OverlaySplitView>,
        #[template_child]
        pub content_header: TemplateChild<adw::HeaderBar>,
        #[template_child]
        pub tool_group: TemplateChild<adw::ToggleGroup>,
        #[template_child]
        pub toast_overlay: TemplateChild<adw::ToastOverlay>,
        #[template_child]
        pub canvas_overlay: TemplateChild<gtk::Overlay>,
        #[template_child]
        pub canvas_slot: TemplateChild<gtk::Box>,
        #[template_child]
        pub zoom_osd: TemplateChild<gtk::Box>,
        #[template_child]
        pub zoom_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub primary_menu: TemplateChild<gtk::MenuButton>,
        #[template_child]
        pub sidebar_stack: TemplateChild<adw::ViewStack>,
        #[template_child]
        pub brush_panel_slot: TemplateChild<gtk::Box>,
        #[template_child]
        pub lab_panel_slot: TemplateChild<gtk::Box>,
        pub tool: Cell<UiTool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for OleoWindow {
        const NAME: &'static str = "OleoWindow";
        type Type = super::OleoWindow;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for OleoWindow {
        fn constructed(&self) {
            self.parent_constructed();
            let window = self.obj();
            window.setup_actions();
            window.setup_tool_shortcuts();
            window.follow_tool_group();
            window.keep_window_buttons_reachable();
        }
    }

    impl WidgetImpl for OleoWindow {}
    impl WindowImpl for OleoWindow {}
    impl ApplicationWindowImpl for OleoWindow {}
    impl AdwApplicationWindowImpl for OleoWindow {}
}

glib::wrapper! {
    pub struct OleoWindow(ObjectSubclass<imp::OleoWindow>)
        @extends adw::ApplicationWindow, gtk::ApplicationWindow, gtk::Window, gtk::Widget,
        @implements gio::ActionGroup, gio::ActionMap, gtk::Accessible, gtk::Buildable,
            gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

const PENDING_ACTIONS: [&str; 12] = [
    "undo",
    "redo",
    "dry",
    "new",
    "open",
    "save",
    "save-as",
    "export",
    "clean-brush",
    "zoom-in",
    "zoom-out",
    "zoom-fit",
];

const TOOL_KEYS: [(gdk::Key, UiTool); 7] = [
    (gdk::Key::h, UiTool::Move),
    (gdk::Key::_1, UiTool::Paint(Tool::Brush)),
    (gdk::Key::_2, UiTool::Paint(Tool::Knife)),
    (gdk::Key::_3, UiTool::Paint(Tool::Blender)),
    (gdk::Key::_4, UiTool::Paint(Tool::Rag)),
    (gdk::Key::_5, UiTool::Paint(Tool::Eyedropper)),
    (gdk::Key::r, UiTool::Paint(Tool::Rag)),
];

impl OleoWindow {
    pub fn new(app: &adw::Application) -> Self {
        glib::Object::builder().property("application", app).build()
    }

    fn setup_actions(&self) {
        let tool = gio::ActionEntry::builder("tool")
            .parameter_type(Some(glib::VariantTy::STRING))
            .state(UiTool::default().name().to_variant())
            .change_state(|window: &Self, action, value| {
                if let Some(name) = value.and_then(|value| value.str()) {
                    window.select_tool(action, name);
                }
            })
            .build();
        let toggle_sidebar = gio::ActionEntry::builder("toggle-sidebar")
            .activate(|window: &Self, _, _| {
                let split_view = &window.imp().split_view;
                split_view.set_show_sidebar(!split_view.shows_sidebar());
            })
            .build();
        let pending = PENDING_ACTIONS.into_iter().map(|name| {
            gio::ActionEntry::builder(name)
                .activate(move |_: &Self, _, _| {
                    glib::g_debug!(LOG_DOMAIN, "win.{name} is not wired yet");
                })
                .build()
        });
        self.add_action_entries([tool, toggle_sidebar].into_iter().chain(pending));
    }

    fn select_tool(&self, action: &gio::SimpleAction, name: &str) {
        let Some(tool) = UiTool::from_name(name) else {
            glib::g_warning!(LOG_DOMAIN, "unknown tool {name:?}");
            return;
        };
        action.set_state(&name.to_variant());
        self.imp().tool.set(tool);
        let tool_group = &self.imp().tool_group;
        if tool_group.active_name().as_deref() != Some(name) {
            tool_group.set_active_name(Some(name));
        }
    }

    fn follow_tool_group(&self) {
        self.imp()
            .tool_group
            .connect_active_name_notify(glib::clone!(
                #[weak(rename_to = window)]
                self,
                move |tool_group| {
                    let Some(name) = tool_group.active_name() else {
                        return;
                    };
                    if window.imp().tool.get().name() != name {
                        window.change_action_state("tool", &name.to_variant());
                    }
                }
            ));
    }

    fn setup_tool_shortcuts(&self) {
        let controller = gtk::ShortcutController::new();
        controller.set_scope(gtk::ShortcutScope::Global);
        controller.set_propagation_phase(gtk::PropagationPhase::Bubble);
        for (key, tool) in TOOL_KEYS {
            let shortcut = gtk::Shortcut::builder()
                .trigger(&gtk::KeyvalTrigger::new(key, gdk::ModifierType::empty()))
                .action(&gtk::NamedAction::new("win.tool"))
                .arguments(&tool.name().to_variant())
                .build();
            controller.add_shortcut(shortcut);
        }
        self.add_controller(controller);
    }

    fn keep_window_buttons_reachable(&self) {
        let imp = self.imp();
        imp.split_view
            .bind_property(
                "show-sidebar",
                &*imp.content_header,
                "show-end-title-buttons",
            )
            .invert_boolean()
            .sync_create()
            .build();
    }
}
