//! Manettes sur iPhone, iPad et Apple TV. `gilrs` (utilisé par Bevy ailleurs) ne connaît pas iOS ni tvOS: on lit donc les
//! manettes par le framework Apple GameController, et on les présente à Bevy comme des événements de manette ordinaires.
//! Le reste du jeu (`touch.rs`) lit ainsi les mêmes `Gamepad` que sur ordinateur.

use std::collections::HashMap;

use bevy::{
    input::gamepad::{
        GamepadConnection, GamepadConnectionEvent, RawGamepadAxisChangedEvent, RawGamepadButtonChangedEvent,
        RawGamepadEvent,
    },
    prelude::*,
};
use objc2::{class, msg_send, rc::Retained, runtime::AnyObject};

#[link(name = "GameController", kind = "framework")]
unsafe extern "C" {}

pub struct GameControllerPlugin;

impl Plugin for GameControllerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreUpdate, poll_controllers.before(bevy::input::InputSystems));
    }
}

/// Une manette connue: son entité Bevy et les dernières valeurs envoyées (pour n'émettre que les changements).
struct Known {
    entity: Entity,
    buttons: [f32; BUTTONS.len()],
    axes: [f32; AXES.len()],
    seen: bool,
}

/// Boutons lus: chemin des sélecteurs depuis `GCExtendedGamepad` → bouton Bevy.
const BUTTONS: [(&[&str], GamepadButton); 12] = [
    (&["buttonA"], GamepadButton::South),
    (&["buttonB"], GamepadButton::East),
    (&["buttonX"], GamepadButton::West),
    (&["buttonY"], GamepadButton::North),
    (&["leftShoulder"], GamepadButton::LeftTrigger),
    (&["rightShoulder"], GamepadButton::RightTrigger),
    (&["buttonMenu"], GamepadButton::Start),
    (&["buttonOptions"], GamepadButton::Select),
    (&["dpad", "up"], GamepadButton::DPadUp),
    (&["dpad", "down"], GamepadButton::DPadDown),
    (&["dpad", "left"], GamepadButton::DPadLeft),
    (&["dpad", "right"], GamepadButton::DPadRight),
];

/// Axes lus: chemin des sélecteurs → axe Bevy (Y vers le haut, comme chez Apple).
const AXES: [(&[&str], GamepadAxis); 4] = [
    (&["leftThumbstick", "xAxis"], GamepadAxis::LeftStickX),
    (&["leftThumbstick", "yAxis"], GamepadAxis::LeftStickY),
    (&["rightThumbstick", "xAxis"], GamepadAxis::RightStickX),
    (&["rightThumbstick", "yAxis"], GamepadAxis::RightStickY),
];

/// Suit un chemin de propriétés Objective-C (`dpad` puis `up`...) et lit la `value` de la dernière.
fn read(pad: &AnyObject, path: &[&str]) -> f32 {
    let mut object: Option<Retained<AnyObject>> = None;
    for name in path {
        let selector = objc2::runtime::Sel::register(&std::ffi::CString::new(*name).unwrap());
        let from: &AnyObject = object.as_deref().unwrap_or(pad);
        let next: Option<Retained<AnyObject>> = unsafe { msg_send![from, performSelector: selector] };
        match next {
            Some(next) => object = Some(next),
            None => return 0.0,
        }
    }
    match object {
        Some(input) => unsafe { msg_send![&*input, value] },
        None => 0.0,
    }
}

/// Le nom que la manette donne d'elle-même (« 8BitDo SN30 Pro », « Xbox Wireless Controller »...).
fn vendor_name(controller: &AnyObject) -> String {
    let name: Option<Retained<AnyObject>> = unsafe { msg_send![controller, vendorName] };
    let Some(name) = name else {
        return "GameController".to_string();
    };
    let chars: *const std::ffi::c_char = unsafe { msg_send![&*name, UTF8String] };
    if chars.is_null() {
        return "GameController".to_string();
    }
    unsafe { std::ffi::CStr::from_ptr(chars) }.to_string_lossy().into_owned()
}

fn poll_controllers(
    mut known: Local<HashMap<usize, Known>>,
    mut commands: Commands,
    mut events: MessageWriter<RawGamepadEvent>,
    mut connection_events: MessageWriter<GamepadConnectionEvent>,
    mut button_events: MessageWriter<RawGamepadButtonChangedEvent>,
    mut axis_events: MessageWriter<RawGamepadAxisChangedEvent>,
) {
    let controllers: Retained<AnyObject> = unsafe { msg_send![class!(GCController), controllers] };
    let count: usize = unsafe { msg_send![&*controllers, count] };

    for known in known.values_mut() {
        known.seen = false;
    }
    for index in 0..count {
        let controller: Retained<AnyObject> = unsafe { msg_send![&*controllers, objectAtIndex: index] };
        let key = Retained::as_ptr(&controller) as usize;
        let pad: Option<Retained<AnyObject>> = unsafe { msg_send![&*controller, extendedGamepad] };
        let Some(pad) = pad else {
            continue;
        };

        let entry = known.entry(key).or_insert_with(|| {
            let entity = commands.spawn_empty().id();
            // Le nom de la manette dit si ses boutons sont disposés à la façon de Nintendo (voir `settings::PadLayout`).
            let vendor = vendor_name(&controller);
            crate::settings::note_controller(&vendor);
            let event = GamepadConnectionEvent::new(
                entity,
                GamepadConnection::Connected { name: vendor, vendor_id: None, product_id: None },
            );
            events.write(event.clone().into());
            connection_events.write(event);
            Known { entity, buttons: [0.0; BUTTONS.len()], axes: [0.0; AXES.len()], seen: true }
        });
        entry.seen = true;

        for (i, (path, button)) in BUTTONS.iter().enumerate() {
            let value = read(&pad, path);
            if value != entry.buttons[i] {
                entry.buttons[i] = value;
                events.write(RawGamepadButtonChangedEvent::new(entry.entity, *button, value).into());
                button_events.write(RawGamepadButtonChangedEvent::new(entry.entity, *button, value));
            }
        }
        for (i, (path, axis)) in AXES.iter().enumerate() {
            let value = read(&pad, path);
            if value != entry.axes[i] {
                entry.axes[i] = value;
                events.write(RawGamepadAxisChangedEvent::new(entry.entity, *axis, value).into());
                axis_events.write(RawGamepadAxisChangedEvent::new(entry.entity, *axis, value));
            }
        }
    }

    known.retain(|_, k| {
        if !k.seen {
            let event = GamepadConnectionEvent::new(k.entity, GamepadConnection::Disconnected);
            events.write(event.clone().into());
            connection_events.write(event);
        }
        k.seen
    });
}
