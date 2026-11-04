use std::cell::{Cell, OnceCell, RefCell};
use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Rc;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gdk, gio, glib};

use oleo_engine::canvas::CanvasSpec;

use crate::LOG_DOMAIN;
use crate::brush_panel::BrushPanel;
use crate::canvas_widget::OleoCanvas;
use crate::dialogs::confirm_discard;
use crate::lab_panel::LabPanel;
use crate::new_painting_dialog::NewPaintingDialog;
use crate::state::{DryOutcome, SharedState};
use crate::tools::{Tool, UiTool};

pub type PanelRefresher = Rc<dyn Fn()>;

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
        pub canvas: OnceCell<OleoCanvas>,
        pub panel_refreshers: RefCell<Vec<PanelRefresher>>,
        pub saves_in_flight: RefCell<HashSet<PathBuf>>,
        pub document_generation: Cell<u64>,
        pub close_after_saves: Cell<bool>,
        pub close_confirmed: Cell<bool>,
        pub asking_to_close: Cell<bool>,
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
            self.tool_group
                .set_active_name(Some(UiTool::default().name()));
        }
    }

    impl WidgetImpl for OleoWindow {}
    impl WindowImpl for OleoWindow {
        fn close_request(&self) -> glib::Propagation {
            if self.obj().may_close() {
                self.parent_close_request()
            } else {
                glib::Propagation::Stop
            }
        }
    }
    impl ApplicationWindowImpl for OleoWindow {}
    impl AdwApplicationWindowImpl for OleoWindow {}
}

glib::wrapper! {
    pub struct OleoWindow(ObjectSubclass<imp::OleoWindow>)
        @extends adw::ApplicationWindow, gtk::ApplicationWindow, gtk::Window, gtk::Widget,
        @implements gio::ActionGroup, gio::ActionMap, gtk::Accessible, gtk::Buildable,
            gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

const ZOOM_PERCENT: f32 = 100.0;
const DRIED_TOAST: &str = "Tela seca";
const UNDO_LABEL: &str = "Desfazer";
const COPY_PRESENTATION_TOAST: &str = "Apresentação por cópia (dmabuf indisponível)";
const NOTHING_TO_PICK_TOAST: &str = "Nada para pegar aqui";

pub const TOOL_KEYS: [(gdk::Key, UiTool); 7] = [
    (gdk::Key::h, UiTool::Move),
    (gdk::Key::_1, UiTool::Paint(Tool::Brush)),
    (gdk::Key::_2, UiTool::Paint(Tool::Knife)),
    (gdk::Key::_3, UiTool::Paint(Tool::Blender)),
    (gdk::Key::_4, UiTool::Paint(Tool::Rag)),
    (gdk::Key::_5, UiTool::Paint(Tool::Eyedropper)),
    (gdk::Key::r, UiTool::Paint(Tool::Rag)),
];

const KEYPAD_TOOL_KEYS: [(gdk::Key, UiTool); 5] = [
    (gdk::Key::KP_1, UiTool::Paint(Tool::Brush)),
    (gdk::Key::KP_2, UiTool::Paint(Tool::Knife)),
    (gdk::Key::KP_3, UiTool::Paint(Tool::Blender)),
    (gdk::Key::KP_4, UiTool::Paint(Tool::Rag)),
    (gdk::Key::KP_5, UiTool::Paint(Tool::Eyedropper)),
];

impl OleoWindow {
    pub fn new(app: &adw::Application, state: SharedState) -> Self {
        let window: OleoWindow = glib::Object::builder().property("application", app).build();
        window.attach_state(state);
        window
    }

    pub fn tool(&self) -> UiTool {
        self.imp().tool.get()
    }

    pub fn canvas(&self) -> OleoCanvas {
        self.imp()
            .canvas
            .get()
            .expect("window is created with its canvas")
            .clone()
    }

    fn attach_state(&self, state: SharedState) {
        state.borrow_mut().set_ui_tool(self.tool());
        let canvas = OleoCanvas::new(state);
        canvas.connect_view_changed(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |canvas| window.show_zoom(canvas.zoom())
        ));
        canvas.connect_colour_picked(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_, found_paint| {
                if found_paint {
                    window.refresh_panels();
                } else {
                    window.show_toast(NOTHING_TO_PICK_TOAST);
                }
            }
        ));
        canvas.connect_presented(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_| window.sync_history_actions()
        ));
        canvas.connect_presentation_fallback(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_| {
                window
                    .imp()
                    .toast_overlay
                    .add_toast(adw::Toast::new(COPY_PRESENTATION_TOAST));
            }
        ));
        self.imp().canvas_slot.append(&canvas);
        let state = canvas.state();
        if self.imp().canvas.set(canvas).is_err() {
            unreachable!("the canvas is attached once");
        }
        let brush_panel = BrushPanel::new(self, state.clone());
        self.imp().brush_panel_slot.append(brush_panel.widget());
        self.connect_refresh_panels(Rc::new(move || brush_panel.refresh()));
        let lab_panel = LabPanel::new(state);
        self.imp().lab_panel_slot.append(lab_panel.widget());
        let canvas = self.canvas();
        let pen_panel = Rc::downgrade(&lab_panel);
        canvas.connect_presented(move |_| {
            if let Some(panel) = pen_panel.upgrade() {
                panel.show_pen();
            }
        });
        let pen_panel = Rc::downgrade(&lab_panel);
        canvas.connect_pen_observed(move |_| {
            if let Some(panel) = pen_panel.upgrade() {
                panel.show_pen();
            }
        });
        self.connect_refresh_panels(Rc::new(move || lab_panel.refresh()));
        self.sync_history_actions();
    }

    pub fn connect_refresh_panels(&self, refresher: PanelRefresher) {
        self.imp().panel_refreshers.borrow_mut().push(refresher);
    }

    pub fn refresh_panels(&self) {
        let refreshers = self.imp().panel_refreshers.borrow().clone();
        for refresh in refreshers {
            refresh();
        }
    }

    pub fn show_sidebar_page(&self, name: &str) {
        let imp = self.imp();
        imp.split_view.set_show_sidebar(true);
        if imp.sidebar_stack.child_by_name(name).is_some() {
            imp.sidebar_stack.set_visible_child_name(name);
        } else {
            glib::g_warning!(LOG_DOMAIN, "no sidebar page named {name:?}");
        }
    }

    fn sync_history_actions(&self) {
        let state = self.state();
        let Ok(state) = state.try_borrow() else {
            return;
        };
        let (can_undo, can_redo) = (state.can_undo(), state.can_redo());
        drop(state);
        for (name, enabled) in [("undo", can_undo), ("redo", can_redo)] {
            if let Some(action) = self.lookup_action(name).and_downcast::<gio::SimpleAction>()
                && action.is_enabled() != enabled
            {
                action.set_enabled(enabled);
            }
        }
    }

    fn after_history_change(&self) {
        self.canvas().queue_present();
        self.sync_history_actions();
    }

    fn dry(&self) {
        let outcome = self.state().borrow_mut().dry();
        let DryOutcome::Dried { undoable } = outcome else {
            return;
        };
        self.after_history_change();
        let toast = adw::Toast::new(DRIED_TOAST);
        if undoable {
            toast.set_button_label(Some(UNDO_LABEL));
            toast.set_action_name(Some("win.undo"));
        }
        self.imp().toast_overlay.add_toast(toast);
    }

    fn show_zoom(&self, zoom: f32) {
        let text = format!("{:.0}%", zoom * ZOOM_PERCENT);
        let label = &self.imp().zoom_label;
        if label.label() != text {
            label.set_label(&text);
        }
    }

    pub fn state(&self) -> SharedState {
        self.canvas().state()
    }

    pub fn show_toast(&self, title: &str) {
        self.imp().toast_overlay.add_toast(adw::Toast::new(title));
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
        let new = gio::ActionEntry::builder("new")
            .activate(|window: &Self, _, _| window.open_new_painting_dialog())
            .build();
        let clean_brush = gio::ActionEntry::builder("clean-brush")
            .activate(|window: &Self, _, _| {
                window.state().borrow_mut().clean_brush();
                window.refresh_panels();
            })
            .build();
        let undo = gio::ActionEntry::builder("undo")
            .activate(|window: &Self, _, _| {
                window.state().borrow_mut().undo();
                window.after_history_change();
            })
            .build();
        let redo = gio::ActionEntry::builder("redo")
            .activate(|window: &Self, _, _| {
                window.state().borrow_mut().redo();
                window.after_history_change();
            })
            .build();
        let dry = gio::ActionEntry::builder("dry")
            .activate(|window: &Self, _, _| window.dry())
            .build();
        let zoom_in = gio::ActionEntry::builder("zoom-in")
            .activate(|window: &Self, _, _| window.canvas().zoom_in())
            .build();
        let zoom_out = gio::ActionEntry::builder("zoom-out")
            .activate(|window: &Self, _, _| window.canvas().zoom_out())
            .build();
        let zoom_fit = gio::ActionEntry::builder("zoom-fit")
            .activate(|window: &Self, _, _| window.canvas().zoom_fit())
            .build();
        let open = gio::ActionEntry::builder("open")
            .activate(|window: &Self, _, _| window.open())
            .build();
        let save = gio::ActionEntry::builder("save")
            .activate(|window: &Self, _, _| window.save())
            .build();
        let save_as = gio::ActionEntry::builder("save-as")
            .activate(|window: &Self, _, _| window.save_as())
            .build();
        let export = gio::ActionEntry::builder("export")
            .activate(|window: &Self, _, _| window.export())
            .build();
        self.add_action_entries([
            tool,
            toggle_sidebar,
            new,
            clean_brush,
            undo,
            redo,
            dry,
            zoom_in,
            zoom_out,
            zoom_fit,
            open,
            save,
            save_as,
            export,
        ]);
    }

    fn open_new_painting_dialog(&self) {
        if self.visible_dialog().is_some() {
            return;
        }
        let dialog = {
            let state = self.state();
            let state = state.borrow();
            NewPaintingDialog::new(&state.session.db, &state.session.canvas.spec)
        };
        dialog.connect_create(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |dialog, spec| {
                if !window.state().borrow().dirty_since_save {
                    dialog.close();
                    window.start_painting(spec);
                    return;
                }
                confirm_discard(
                    dialog,
                    glib::clone!(
                        #[weak]
                        window,
                        #[weak]
                        dialog,
                        move || {
                            dialog.close();
                            window.start_painting(spec.clone());
                        }
                    ),
                );
            }
        ));
        dialog.present(Some(self));
    }

    fn start_painting(&self, spec: CanvasSpec) {
        self.state().borrow_mut().start_painting(spec);
        self.after_document_replaced();
    }

    pub fn after_document_replaced(&self) {
        let generation = &self.imp().document_generation;
        generation.set(generation.get() + 1);
        let canvas = self.canvas();
        canvas.zoom_fit();
        canvas.queue_present();
        self.sync_history_actions();
        self.refresh_panels();
        self.show_document_title();
    }

    fn select_tool(&self, action: &gio::SimpleAction, name: &str) {
        let Some(tool) = UiTool::from_name(name) else {
            glib::g_warning!(LOG_DOMAIN, "unknown tool {name:?}");
            return;
        };
        action.set_state(&name.to_variant());
        self.imp().tool.set(tool);
        if let Some(canvas) = self.imp().canvas.get() {
            canvas.state().borrow_mut().set_ui_tool(tool);
            self.refresh_panels();
        }
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
        for (key, tool) in TOOL_KEYS.into_iter().chain(KEYPAD_TOOL_KEYS) {
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
