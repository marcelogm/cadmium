use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::glib;

use crate::dialogs::{ask_to_quit_while_writing, unsaved_changes_discard_confirmed};
use crate::window::CadmiumWindow;
use crate::writes::work_would_be_lost;

const WRITE_HANG_SECONDS: u32 = 10;

impl CadmiumWindow {
    pub fn work_would_be_lost(&self) -> bool {
        let unsaved_edits = self.state().borrow().has_unsaved_edits();
        work_would_be_lost(unsaved_edits, &self.imp().writes_in_flight.borrow())
    }

    pub fn may_close(&self) -> bool {
        let imp = self.imp();
        if imp.abandon_writes.get() {
            return true;
        }
        if !imp.writes_in_flight.borrow().is_empty() {
            imp.close_after_writes.set(true);
            self.offer_to_quit_if_writes_hang();
            return false;
        }
        if imp.close_confirmed.get() || !self.work_would_be_lost() {
            return true;
        }
        if !imp.asking_to_close.replace(true) {
            glib::spawn_future_local(confirm_close(self.clone()));
        }
        false
    }

    fn offer_to_quit_if_writes_hang(&self) {
        let imp = self.imp();
        if imp.write_wait_timeout.borrow().is_some() || imp.write_wait_alert.borrow().is_some() {
            return;
        }
        let window = self.downgrade();
        let timeout = glib::timeout_add_seconds_local_once(WRITE_HANG_SECONDS, move || {
            offer_to_quit_now(&window)
        });
        imp.write_wait_timeout.replace(Some(timeout));
    }

    fn ask_to_quit_while_writing(&self) {
        let window = self.downgrade();
        let alert = ask_to_quit_while_writing(self, move || abandon_writes_and_close(&window));
        let window = self.downgrade();
        alert.connect_closed(move |_| forget_write_wait_alert(&window));
        self.imp().write_wait_alert.replace(Some(alert));
    }
}

async fn confirm_close(window: CadmiumWindow) {
    let discard = unsaved_changes_discard_confirmed(&window).await;
    window.imp().asking_to_close.set(false);
    if discard {
        window.imp().close_confirmed.set(true);
        window.close();
    }
}

fn offer_to_quit_now(window: &glib::WeakRef<CadmiumWindow>) {
    let Some(window) = window.upgrade() else {
        return;
    };
    let imp = window.imp();
    imp.write_wait_timeout.take();
    if imp.close_after_writes.get() && !imp.writes_in_flight.borrow().is_empty() {
        window.ask_to_quit_while_writing();
    }
}

fn abandon_writes_and_close(window: &glib::WeakRef<CadmiumWindow>) {
    if let Some(window) = window.upgrade() {
        window.imp().abandon_writes.set(true);
        window.close();
    }
}

fn forget_write_wait_alert(window: &glib::WeakRef<CadmiumWindow>) {
    if let Some(window) = window.upgrade() {
        window.imp().write_wait_alert.take();
    }
}
