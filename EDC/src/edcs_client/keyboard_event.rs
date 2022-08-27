// Translate mouse/virtualkeyboardevent to Linux input keycodes

use glutin::event::VirtualKeyCode;
use input_linux_sys::*;

pub fn virtual_key_code_to_linux_input(vkc: VirtualKeyCode) -> i32 {
    match vkc {
        VirtualKeyCode::Key1 => KEY_1,
        VirtualKeyCode::Key2 => KEY_2,
        VirtualKeyCode::Key3 => KEY_3,
        VirtualKeyCode::Key4 => KEY_4,
        VirtualKeyCode::Key5 => KEY_5,
        VirtualKeyCode::Key6 => KEY_6,
        VirtualKeyCode::Key7 => KEY_7,
        VirtualKeyCode::Key8 => KEY_8,
        VirtualKeyCode::Key9 => KEY_9,
        VirtualKeyCode::Key0 => KEY_0,
        VirtualKeyCode::A => KEY_A,
        VirtualKeyCode::B => KEY_B,
        VirtualKeyCode::C => KEY_C,
        VirtualKeyCode::D => KEY_D,
        VirtualKeyCode::E => KEY_E,
        VirtualKeyCode::F => KEY_F,
        VirtualKeyCode::G => KEY_G,
        VirtualKeyCode::H => KEY_H,
        VirtualKeyCode::I => KEY_I,
        VirtualKeyCode::J => KEY_J,
        VirtualKeyCode::K => KEY_K,
        VirtualKeyCode::L => KEY_L,
        VirtualKeyCode::M => KEY_M,
        VirtualKeyCode::N => KEY_N,
        VirtualKeyCode::O => KEY_O,
        VirtualKeyCode::P => KEY_P,
        VirtualKeyCode::Q => KEY_Q,
        VirtualKeyCode::R => KEY_R,
        VirtualKeyCode::S => KEY_S,
        VirtualKeyCode::T => KEY_T,
        VirtualKeyCode::U => KEY_U,
        VirtualKeyCode::V => KEY_V,
        VirtualKeyCode::W => KEY_W,
        VirtualKeyCode::X => KEY_X,
        VirtualKeyCode::Y => KEY_Y,
        VirtualKeyCode::Z => KEY_Z,
        VirtualKeyCode::Escape => KEY_ESC,
        VirtualKeyCode::F1 => KEY_F1,
        VirtualKeyCode::F2 => KEY_F2,
        VirtualKeyCode::F3 => KEY_F3,
        VirtualKeyCode::F4 => KEY_F4,
        VirtualKeyCode::F5 => KEY_F5,
        VirtualKeyCode::F6 => KEY_F6,
        VirtualKeyCode::F7 => KEY_F7,
        VirtualKeyCode::F8 => KEY_F8,
        VirtualKeyCode::F9 => KEY_F9,
        VirtualKeyCode::F10 => KEY_F10,
        VirtualKeyCode::F11 => KEY_F11,
        VirtualKeyCode::F12 => KEY_F12,
        VirtualKeyCode::F13 => KEY_F13,
        VirtualKeyCode::F14 => KEY_F14,
        VirtualKeyCode::F15 => KEY_F15,
        VirtualKeyCode::F16 => KEY_F16,
        VirtualKeyCode::F17 => KEY_F17,
        VirtualKeyCode::F18 => KEY_F18,
        VirtualKeyCode::F19 => KEY_F19,
        VirtualKeyCode::F20 => KEY_F20,
        VirtualKeyCode::F21 => KEY_F21,
        VirtualKeyCode::F22 => KEY_F22,
        VirtualKeyCode::F23 => KEY_F23,
        VirtualKeyCode::F24 => KEY_F24,
        VirtualKeyCode::Snapshot => KEY_PRINT,
        VirtualKeyCode::Scroll => KEY_SCROLLLOCK,
        VirtualKeyCode::Pause => KEY_PAUSE,
        VirtualKeyCode::Insert => KEY_INSERT,
        VirtualKeyCode::Home => KEY_HOME,
        VirtualKeyCode::Delete => KEY_DELETE,
        VirtualKeyCode::End => KEY_END,
        VirtualKeyCode::PageDown => KEY_PAGEDOWN,
        VirtualKeyCode::PageUp => KEY_PAGEUP,
        VirtualKeyCode::Left => KEY_LEFT,
        VirtualKeyCode::Up => KEY_UP,
        VirtualKeyCode::Right => KEY_RIGHT,
        VirtualKeyCode::Down => KEY_DOWN,
        VirtualKeyCode::Back => KEY_BACKSPACE,
        VirtualKeyCode::Return => KEY_ENTER,
        VirtualKeyCode::Space => KEY_SPACE,
        VirtualKeyCode::Compose => KEY_COMPOSE,
        VirtualKeyCode::Caret => todo!(),
        VirtualKeyCode::Numlock => KEY_NUMLOCK,
        VirtualKeyCode::Numpad0 => KEY_NUMERIC_0,
        VirtualKeyCode::Numpad1 => KEY_NUMERIC_1,
        VirtualKeyCode::Numpad2 => KEY_NUMERIC_2,
        VirtualKeyCode::Numpad3 => KEY_NUMERIC_3,
        VirtualKeyCode::Numpad4 => KEY_NUMERIC_4,
        VirtualKeyCode::Numpad5 => KEY_NUMERIC_5,
        VirtualKeyCode::Numpad6 => KEY_NUMERIC_6,
        VirtualKeyCode::Numpad7 => KEY_NUMERIC_7,
        VirtualKeyCode::Numpad8 => KEY_NUMERIC_8,
        VirtualKeyCode::Numpad9 => KEY_NUMERIC_9,
        VirtualKeyCode::NumpadAdd => KEY_KPPLUS,
        VirtualKeyCode::NumpadDivide => KEY_KPSLASH,
        VirtualKeyCode::NumpadDecimal => KEY_KPDOT,
        VirtualKeyCode::NumpadComma => KEY_KPCOMMA,
        VirtualKeyCode::NumpadEnter => KEY_KPENTER,
        VirtualKeyCode::NumpadEquals => KEY_KPEQUAL,
        VirtualKeyCode::NumpadMultiply => KEY_NUMERIC_STAR,
        VirtualKeyCode::NumpadSubtract => KEY_KPMINUS,
        VirtualKeyCode::AbntC1 => todo!(),
        VirtualKeyCode::AbntC2 => todo!(),
        VirtualKeyCode::Apostrophe => KEY_APOSTROPHE,
        VirtualKeyCode::Apps => KEY_APPSELECT,
        VirtualKeyCode::Asterisk => KEY_KPASTERISK,
        VirtualKeyCode::At => KEY_EMAIL,
        VirtualKeyCode::Ax => KEY_KPASTERISK,
        VirtualKeyCode::Backslash => KEY_BACKSLASH,
        VirtualKeyCode::Calculator => KEY_CALC,
        VirtualKeyCode::Capital => KEY_CAPSLOCK,
        // This is going to be hard since this is done with colon + shift
        VirtualKeyCode::Colon => todo!(),
        VirtualKeyCode::Comma => KEY_COMMA,
        VirtualKeyCode::Convert => todo!(),
        VirtualKeyCode::Equals => KEY_EQUAL,
        VirtualKeyCode::Grave => KEY_GRAVE,
        VirtualKeyCode::Kana => KEY_KATAKANA,
        // This is not Kanji
        VirtualKeyCode::Kanji => todo!(),
        VirtualKeyCode::LAlt => KEY_LEFTALT,
        VirtualKeyCode::LBracket => KEY_LEFTBRACE,
        VirtualKeyCode::LControl => KEY_LEFTCTRL,
        VirtualKeyCode::LShift => KEY_LEFTSHIFT,
        VirtualKeyCode::LWin => KEY_LEFTMETA,
        VirtualKeyCode::Mail => KEY_MAIL,
        VirtualKeyCode::MediaSelect => KEY_MEDIA,
        VirtualKeyCode::MediaStop => KEY_STOP,
        VirtualKeyCode::Minus => KEY_MINUS,
        VirtualKeyCode::Mute => KEY_MUTE,
        VirtualKeyCode::MyComputer => KEY_COMPUTER,
        VirtualKeyCode::NavigateForward => KEY_NEXT,
        VirtualKeyCode::NavigateBackward => KEY_PREVIOUS,
        VirtualKeyCode::NextTrack => KEY_NEXTSONG,
        VirtualKeyCode::NoConvert => todo!(),
        VirtualKeyCode::OEM102 => todo!(),
        VirtualKeyCode::Period => KEY_DOT,
        VirtualKeyCode::PlayPause => KEY_PLAYPAUSE,
        VirtualKeyCode::Plus => KEY_KPPLUS,
        VirtualKeyCode::Power => KEY_POWER,
        VirtualKeyCode::PrevTrack => KEY_PREVIOUSSONG,
        VirtualKeyCode::RAlt => KEY_RIGHTALT,
        VirtualKeyCode::RBracket => KEY_RIGHTBRACE,
        VirtualKeyCode::RControl => KEY_RIGHTCTRL,
        VirtualKeyCode::RShift => KEY_RIGHTSHIFT,
        VirtualKeyCode::RWin => KEY_RIGHTMETA,
        VirtualKeyCode::Semicolon => KEY_SEMICOLON,
        VirtualKeyCode::Slash => KEY_SLASH,
        VirtualKeyCode::Sleep => KEY_SLEEP,
        VirtualKeyCode::Stop => KEY_STOP,
        VirtualKeyCode::Sysrq => KEY_SYSRQ,
        VirtualKeyCode::Tab => KEY_TAB,
        VirtualKeyCode::Underline => todo!(),
        VirtualKeyCode::Unlabeled => todo!(),
        VirtualKeyCode::VolumeDown => KEY_VOLUMEDOWN,
        VirtualKeyCode::VolumeUp => KEY_VOLUMEUP,
        VirtualKeyCode::Wake => KEY_WAKEUP,
        VirtualKeyCode::WebBack => KEY_BACK,
        VirtualKeyCode::WebFavorites => KEY_FAVORITES,
        VirtualKeyCode::WebForward => KEY_FORWARD,
        VirtualKeyCode::WebHome => KEY_HOMEPAGE,
        VirtualKeyCode::WebRefresh => KEY_REFRESH,
        VirtualKeyCode::WebSearch => KEY_SEARCH,
        VirtualKeyCode::WebStop => KEY_STOP,
        VirtualKeyCode::Yen => KEY_YEN,
        VirtualKeyCode::Copy => KEY_COPY,
        VirtualKeyCode::Paste => KEY_PASTE,
        VirtualKeyCode::Cut => KEY_CUT,
    }
}
