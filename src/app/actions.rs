//! Window actions (menu items and keyboard shortcuts)

use relm4::actions::{AccelsPlus, RelmAction, RelmActionGroup};
use relm4::adw;
use relm4::adw::prelude::*;
use relm4::prelude::*;

use super::{Action, App, Msg};

// The action types used by the `menu!` macro in `mod.rs` must be visible to the parent module
relm4::new_action_group!(pub(super) WinActions, "win");
relm4::new_stateless_action!(pub(super) NewAction, WinActions, "new");
relm4::new_stateless_action!(pub(super) OpenAction, WinActions, "open");
relm4::new_stateless_action!(pub(super) SaveAction, WinActions, "save");
relm4::new_stateless_action!(pub(super) SaveAsAction, WinActions, "save-as");
relm4::new_stateless_action!(pub(super) PreferencesAction, WinActions, "preferences");
relm4::new_stateless_action!(pub(super) AboutAction, WinActions, "about");

// Register window actions and keyboard shortcuts
pub(super) fn register_window_actions(
    root: &adw::ApplicationWindow,
    sender: &ComponentSender<App>,
) {
    let mut actions = RelmActionGroup::<WinActions>::new();

    macro_rules! action {
        ($ty:ty, $msg:expr) => {{
            let sender = sender.clone();
            actions.add_action(RelmAction::<$ty>::new_stateless(move |_| {
                sender.input($msg)
            }));
        }};
    }

    action!(NewAction, Msg::Request(Action::New));
    action!(OpenAction, Msg::Request(Action::Open));
    action!(SaveAction, Msg::Save(None));
    action!(SaveAsAction, Msg::SaveAs(None));
    action!(PreferencesAction, Msg::ShowPreferences);
    action!(AboutAction, Msg::ShowAbout);
    actions.register_for_widget(root);

    let app = relm4::main_application();
    app.set_accelerators_for_action::<NewAction>(&["<Primary>n"]);
    app.set_accelerators_for_action::<OpenAction>(&["<Primary>o"]);
    app.set_accelerators_for_action::<SaveAction>(&["<Primary>s"]);
    app.set_accelerators_for_action::<SaveAsAction>(&["<Primary><Shift>s"]);
    app.set_accelerators_for_action::<PreferencesAction>(&["<Primary>comma"]);
    app.set_accels_for_action("window.close", &["<Primary>w", "<Primary>q"]);
}
