use std::{
    thread,
    time::Duration,
};

use enigo::{Direction, Enigo, Key, Keyboard, Settings};

#[cfg(target_os = "windows")]
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};

use super::SelectionError;

const MODIFIER_POLL_INTERVAL: Duration = Duration::from_millis(10);
const MODIFIER_SETTLE_DELAY: Duration = Duration::from_millis(20);

pub fn wait_until_modifiers_released() {
    wait_for_modifier_release(any_modifier_pressed);
    thread::sleep(MODIFIER_SETTLE_DELAY);
}

pub fn send_copy_shortcut() -> Result<(), SelectionError> {
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

fn wait_for_modifier_release(mut any_modifier_pressed: impl FnMut() -> bool) {
    while any_modifier_pressed() {
        thread::sleep(MODIFIER_POLL_INTERVAL);
    }
}

#[cfg(target_os = "windows")]
fn any_modifier_pressed() -> bool {
    [VK_MENU, VK_CONTROL, VK_SHIFT, VK_LWIN, VK_RWIN]
        .into_iter()
        .any(|key| unsafe { GetAsyncKeyState(key.0 as i32) } < 0)
}

#[cfg(not(target_os = "windows"))]
fn any_modifier_pressed() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

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
