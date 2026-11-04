use adw::subclass::prelude::*;
use gtk::{gio, glib};

mod imp {
    use super::*;

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/marcelo/Oleo/window.ui")]
    pub struct OleoWindow {}

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

    impl ObjectImpl for OleoWindow {}
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

impl OleoWindow {
    pub fn new(app: &adw::Application) -> Self {
        glib::Object::builder().property("application", app).build()
    }
}
