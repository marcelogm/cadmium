use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::glib;

use crate::dialogs::{ask_to_quit_while_writing, unsaved_changes_discard_confirmed};
use crate::window::CadmiumWindow;
use crate::writes::CloseStep;

const WRITE_HANG_SECONDS: u32 = 10;

impl CadmiumWindow {
    pub fn work_would_be_lost(&self) -> bool {
        let unsaved_edits = self.state().borrow().has_unsaved_edits();
        self.imp()
            .closing
            .borrow()
            .work_would_be_lost(unsaved_edits)
    }

    pub fn may_close(&self) -> bool {
        let unsaved_edits = || self.state().borrow().has_unsaved_edits();
        let step = self.imp().closing.borrow_mut().request_close(unsaved_edits);
        match step {
            CloseStep::Close => true,
            CloseStep::WaitForWrites => {
                self.offer_to_quit_if_writes_hang();
                false
            }
            CloseStep::Ask => {
                glib::spawn_future_local(confirm_close(self.clone()));
                false
            }
            CloseStep::AlreadyAsking => false,
        }
    }

    fn offer_to_quit_if_writes_hang(&self) {
        let imp = self.imp();
        if imp.write_wait_timeout.borrow().is_some() || imp.write_wait_alert.borrow().is_some() {
            return;
        }
        let window = self.downgrade();
        let offer = move || offer_to_quit_now(&window);
        let timeout = glib::timeout_add_seconds_local_once(WRITE_HANG_SECONDS, offer);
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
    window.imp().closing.borrow_mut().discard_answered(discard);
    if discard {
        window.close();
    }
}

fn offer_to_quit_now(window: &glib::WeakRef<CadmiumWindow>) {
    let Some(window) = window.upgrade() else {
        return;
    };
    let imp = window.imp();
    imp.write_wait_timeout.take();
    if imp.closing.borrow().still_waiting_for_writes() {
        window.ask_to_quit_while_writing();
    }
}

fn abandon_writes_and_close(window: &glib::WeakRef<CadmiumWindow>) {
    if let Some(window) = window.upgrade() {
        window.imp().closing.borrow_mut().abandon_writes();
        window.close();
    }
}

fn forget_write_wait_alert(window: &glib::WeakRef<CadmiumWindow>) {
    if let Some(window) = window.upgrade() {
        window.imp().write_wait_alert.take();
    }
}
