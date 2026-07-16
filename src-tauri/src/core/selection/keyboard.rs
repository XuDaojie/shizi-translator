#[cfg(target_os = "windows")]
use std::mem::size_of;
#[cfg(not(target_os = "windows"))]
use std::{thread, time::Duration};

#[cfg(not(target_os = "windows"))]
use enigo::{Direction, Enigo, Key, Keyboard, Settings};

#[cfg(target_os = "windows")]
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS,
    KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_C, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_RCONTROL,
    VK_RMENU, VK_RSHIFT, VK_RWIN,
};

use super::SelectionError;

#[cfg(not(target_os = "windows"))]
const MODIFIER_POLL_INTERVAL: Duration = Duration::from_millis(10);
#[cfg(not(target_os = "windows"))]
const MODIFIER_SETTLE_DELAY: Duration = Duration::from_millis(20);

pub fn send_copy_shortcut() -> Result<(), SelectionError> {
    #[cfg(target_os = "windows")]
    {
        send_copy_shortcut_windows()
    }

    #[cfg(not(target_os = "windows"))]
    {
        send_copy_shortcut_portable()
    }
}

#[cfg(not(target_os = "windows"))]
fn send_copy_shortcut_portable() -> Result<(), SelectionError> {
    wait_until_modifiers_released();
    let mut enigo =
        Enigo::new(&Settings::default()).map_err(|_| SelectionError::CopyShortcutFailed)?;
    enigo
        .key(Key::Control, Direction::Press)
        .map_err(|_| SelectionError::CopyShortcutFailed)?;
    let result = enigo.key(Key::Unicode('c'), Direction::Click);
    let release_result = enigo.key(Key::Control, Direction::Release);

    result
        .and(release_result)
        .map_err(|_| SelectionError::CopyShortcutFailed)
}

#[cfg(not(target_os = "windows"))]
fn wait_until_modifiers_released() {
    wait_for_modifier_release(any_modifier_pressed);
    thread::sleep(MODIFIER_SETTLE_DELAY);
}

#[cfg(not(target_os = "windows"))]
fn wait_for_modifier_release(mut any_modifier_pressed: impl FnMut() -> bool) {
    while any_modifier_pressed() {
        thread::sleep(MODIFIER_POLL_INTERVAL);
    }
}

#[cfg(target_os = "windows")]
const MODIFIER_KEYS: [VIRTUAL_KEY; 8] = [
    VK_LCONTROL,
    VK_RCONTROL,
    VK_LMENU,
    VK_RMENU,
    VK_LSHIFT,
    VK_RSHIFT,
    VK_LWIN,
    VK_RWIN,
];

#[cfg(target_os = "windows")]
fn pressed_modifiers() -> Vec<VIRTUAL_KEY> {
    MODIFIER_KEYS
        .into_iter()
        .filter(|key| unsafe { GetAsyncKeyState(key.0 as i32) } < 0)
        .collect()
}

#[cfg(not(target_os = "windows"))]
fn any_modifier_pressed() -> bool {
    false
}

#[cfg(target_os = "windows")]
fn keyboard_input(key: VIRTUAL_KEY, key_up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: key,
                dwFlags: if key_up {
                    KEYEVENTF_KEYUP
                } else {
                    KEYBD_EVENT_FLAGS::default()
                },
                ..Default::default()
            },
        },
    }
}

#[cfg(target_os = "windows")]
fn copy_inputs(pressed_modifiers: &[VIRTUAL_KEY]) -> Vec<INPUT> {
    let mut inputs = Vec::with_capacity(pressed_modifiers.len() * 2 + 4);
    inputs.extend(
        pressed_modifiers
            .iter()
            .map(|key| keyboard_input(*key, true)),
    );
    inputs.extend([
        keyboard_input(VK_LCONTROL, false),
        keyboard_input(VK_C, false),
        keyboard_input(VK_C, true),
        keyboard_input(VK_LCONTROL, true),
    ]);
    inputs.extend(
        pressed_modifiers
            .iter()
            .map(|key| keyboard_input(*key, false)),
    );
    inputs
}

#[cfg(target_os = "windows")]
fn send_copy_shortcut_windows() -> Result<(), SelectionError> {
    let pressed_modifiers = pressed_modifiers();
    let inputs = copy_inputs(&pressed_modifiers);
    let sent = unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) } as usize;

    if sent == inputs.len() {
        return Ok(());
    }

    let mut recovery = vec![
        keyboard_input(VK_C, true),
        keyboard_input(VK_LCONTROL, true),
    ];
    recovery.extend(
        pressed_modifiers
            .iter()
            .map(|key| keyboard_input(*key, false)),
    );
    unsafe {
        SendInput(&recovery, size_of::<INPUT>() as i32);
    }
    Err(SelectionError::CopyShortcutFailed)
}

#[cfg(test)]
mod tests {
    #[cfg(not(target_os = "windows"))]
    use std::cell::Cell;

    use super::*;

    #[cfg(target_os = "windows")]
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        KEYEVENTF_KEYUP, VK_C, VK_LCONTROL, VK_LMENU,
    };

    #[cfg(target_os = "windows")]
    #[test]
    fn copy_inputs_temporarily_release_and_restore_pressed_alt() {
        let inputs = copy_inputs(&[VK_LMENU]);
        let events: Vec<_> = inputs
            .into_iter()
            .map(|input| {
                let keyboard = unsafe { input.Anonymous.ki };
                (keyboard.wVk, keyboard.dwFlags == KEYEVENTF_KEYUP)
            })
            .collect();

        assert_eq!(
            events,
            vec![
                (VK_LMENU, true),
                (VK_LCONTROL, false),
                (VK_C, false),
                (VK_C, true),
                (VK_LCONTROL, true),
                (VK_LMENU, false),
            ]
        );
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn waits_until_modifiers_are_released() {
        let polls = Cell::new(0);

        wait_for_modifier_release(|| {
            let current = polls.get();
            polls.set(current + 1);
            current < 2
        });

        assert_eq!(polls.get(), 3);
    }
}
