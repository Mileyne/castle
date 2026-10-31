//! Apple TV: pas de dossier d'enregistrement durable (Documents est vidé par le système), seules les préférences de l'app
//! (`NSUserDefaults`) restent d'un lancement à l'autre. Les parties et les réglages y sont donc rangés, à la place des fichiers.

use std::ffi::{CStr, CString, c_char};

use castle_shared::save::{self, Backend};
use objc2::{class, msg_send, runtime::AnyObject};

/// `UIUserInterfaceIdiomTV`.
const IDIOM_TV: isize = 3;
const PREFIX: &str = "castle.";

struct Defaults;

/// Une chaîne Objective-C (NSString) pour un texte Rust.
unsafe fn ns_string(text: &str) -> *mut AnyObject {
    let text = CString::new(text.replace('\0', "")).unwrap_or_default();
    unsafe { msg_send![class!(NSString), stringWithUTF8String: text.as_ptr()] }
}

unsafe fn rust_string(object: *mut AnyObject) -> Option<String> {
    if object.is_null() {
        return None;
    }
    let chars: *const c_char = unsafe { msg_send![object, UTF8String] };
    (!chars.is_null()).then(|| unsafe { CStr::from_ptr(chars) }.to_string_lossy().into_owned())
}

unsafe fn defaults() -> *mut AnyObject {
    unsafe { msg_send![class!(NSUserDefaults), standardUserDefaults] }
}

impl Backend for Defaults {
    fn get(&self, key: &str) -> Option<String> {
        unsafe {
            let value: *mut AnyObject = msg_send![defaults(), stringForKey: ns_string(&format!("{PREFIX}{key}"))];
            rust_string(value)
        }
    }

    fn set(&self, key: &str, value: &str) {
        unsafe {
            let store = defaults();
            let _: () = msg_send![store, setObject: ns_string(value), forKey: ns_string(&format!("{PREFIX}{key}"))];
            // Écrit tout de suite: l'app peut être fermée à tout moment depuis la télécommande.
            let _: bool = msg_send![store, synchronize];
        }
    }

    fn remove(&self, key: &str) {
        unsafe {
            let store = defaults();
            let _: () = msg_send![store, removeObjectForKey: ns_string(&format!("{PREFIX}{key}"))];
            let _: bool = msg_send![store, synchronize];
        }
    }

    fn keys(&self) -> Vec<String> {
        unsafe {
            let all: *mut AnyObject = msg_send![defaults(), dictionaryRepresentation];
            let names: *mut AnyObject = msg_send![all, allKeys];
            let count: usize = msg_send![names, count];
            (0..count)
                .filter_map(|i| {
                    let name: *mut AnyObject = msg_send![names, objectAtIndex: i];
                    rust_string(name)
                })
                .filter_map(|name| name.strip_prefix(PREFIX).map(str::to_string))
                .collect()
        }
    }
}

/// Sur Apple TV (le client y est compilé comme pour iOS), range parties et réglages dans les préférences.
pub fn use_on_tv() {
    let idiom: isize = unsafe {
        let device: *mut AnyObject = msg_send![class!(UIDevice), currentDevice];
        msg_send![device, userInterfaceIdiom]
    };
    if idiom == IDIOM_TV {
        save::use_backend(Box::new(Defaults));
    }
}
