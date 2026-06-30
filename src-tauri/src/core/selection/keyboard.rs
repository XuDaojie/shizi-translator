use enigo::{Direction, Enigo, Key, Keyboard, Settings};

use super::SelectionError;

pub fn send_copy_shortcut() -> Result<(), SelectionError> {
    let mut enigo = Enigo::new(&Settings::default()).map_err(|_| SelectionError::CopyShortcutFailed)?;
    let _ = enigo.key(Key::Alt, Direction::Release);
    enigo
        .key(Key::Control, Direction::Press)
        .map_err(|_| SelectionError::CopyShortcutFailed)?;
    let result = enigo.key(Key::Unicode('c'), Direction::Click);
    let release_result = enigo.key(Key::Control, Direction::Release);

    result
        .and(release_result)
        .map_err(|_| SelectionError::CopyShortcutFailed)
}
