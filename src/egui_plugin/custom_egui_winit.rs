//! This file is in large parts copied from egui_winit

use bevy::prelude::*;
use std::time::Instant;

/// Handles the integration between egui and winit.
#[derive(Resource)]
pub struct EguiWinitState {
    start_time: std::time::Instant,
    egui_input: egui::RawInput,
    pointer_pos_in_points: Option<egui::Pos2>,
    current_cursor_icon: egui::CursorIcon,
    /// What egui uses.
    pixels_per_point: f32,
}

impl EguiWinitState {
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            egui_input: egui::RawInput {
                has_focus: false,
                ..Default::default()
            },
            pointer_pos_in_points: None,
            current_cursor_icon: egui::CursorIcon::Default,
            pixels_per_point: 1.0,
        }
    }

    /// Prepare for a new frame by extracting the accumulated input,
    /// as well as setting [the time](egui::RawInput::time) and [screen rectangle](egui::RawInput::screen_rect).
    pub fn take_egui_input(&mut self, window: &winit::window::Window) -> egui::RawInput {
        let pixels_per_point = self.pixels_per_point;

        self.egui_input.time = Some(self.start_time.elapsed().as_secs_f64());

        // On Windows, a minimized window will have 0 width and height.
        // See: https://github.com/rust-windowing/winit/issues/208
        // This solves an issue where egui window positions would be changed when minimizing on Windows.
        let screen_size_in_pixels = screen_size_in_pixels(window);
        let screen_size_in_points = screen_size_in_pixels / pixels_per_point;
        self.egui_input.screen_rect =
            if screen_size_in_points.x > 0.0 && screen_size_in_points.y > 0.0 {
                Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    screen_size_in_points,
                ))
            } else {
                None
            };

        self.egui_input.take()
    }

    /// Call this when there is a new event.
    ///
    /// The result can be found in [`Self::egui_input`] and be extracted with [`Self::take_egui_input`].
    ///
    /// Returns `true` if egui wants exclusive use of this event
    /// (e.g. a mouse click on an egui window, or entering text into a text field).
    /// For instance, if you use egui for a game, you want to first call this
    /// and only when this returns `false` pass on the events to your game.
    ///
    /// Note that egui uses `tab` to move focus between elements, so this will always return `true` for tabs.
    pub fn on_event(
        &mut self,
        egui_ctx: &egui::Context,
        event: &winit::event::WindowEvent<'_>,
    ) -> bool {
        use winit::event::WindowEvent;
        // TODO use bevy events directly
        match event {
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                let pixels_per_point = *scale_factor as f32;
                self.egui_input.pixels_per_point = Some(pixels_per_point);
                self.pixels_per_point = pixels_per_point;
                false
            }
            WindowEvent::MouseInput { state, button, .. } => {
                self.on_mouse_button_input(*state, *button);
                egui_ctx.wants_pointer_input()
            }
            WindowEvent::MouseWheel { delta, .. } => {
                self.on_mouse_wheel(*delta);
                egui_ctx.wants_pointer_input()
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.on_cursor_moved(*position);
                egui_ctx.is_using_pointer()
            }
            WindowEvent::CursorLeft { .. } => {
                self.pointer_pos_in_points = None;
                self.egui_input.events.push(egui::Event::PointerGone);
                false
            }
            WindowEvent::ReceivedCharacter(ch) => {
                // On Mac we get here when the user presses Cmd-C (copy), ctrl-W, etc.
                // We need to ignore these characters that are side-effects of commands.
                let is_mac_cmd = cfg!(target_os = "macos")
                    && (self.egui_input.modifiers.ctrl || self.egui_input.modifiers.mac_cmd);

                if is_printable_char(*ch) && !is_mac_cmd {
                    self.egui_input
                        .events
                        .push(egui::Event::Text(ch.to_string()));
                    egui_ctx.wants_keyboard_input()
                } else {
                    false
                }
            }
            WindowEvent::KeyboardInput { input, .. } => {
                self.on_keyboard_input(input);
                egui_ctx.wants_keyboard_input()
                    || input.virtual_keycode == Some(winit::event::VirtualKeyCode::Tab)
            }
            WindowEvent::Focused(has_focus) => {
                self.egui_input.has_focus = *has_focus;
                // We will not be given a KeyboardInput event when the modifiers are released while
                // the window does not have focus. Unset all modifier state to be safe.
                self.egui_input.modifiers = egui::Modifiers::default();
                false
            }
            WindowEvent::HoveredFile(path) => {
                self.egui_input.hovered_files.push(egui::HoveredFile {
                    path: Some(path.clone()),
                    ..Default::default()
                });
                false
            }
            WindowEvent::HoveredFileCancelled => {
                self.egui_input.hovered_files.clear();
                false
            }
            WindowEvent::DroppedFile(path) => {
                self.egui_input.hovered_files.clear();
                self.egui_input.dropped_files.push(egui::DroppedFile {
                    path: Some(path.clone()),
                    ..Default::default()
                });
                false
            }
            WindowEvent::ModifiersChanged(state) => {
                self.egui_input.modifiers.alt = state.alt();
                self.egui_input.modifiers.ctrl = state.ctrl();
                self.egui_input.modifiers.shift = state.shift();
                self.egui_input.modifiers.mac_cmd = cfg!(target_os = "macos") && state.logo();
                self.egui_input.modifiers.command = if cfg!(target_os = "macos") {
                    state.logo()
                } else {
                    state.ctrl()
                };
                false
            }
            _ => {
                // dbg!(event);
                false
            }
        }
    }

    /// Call with the output given by `egui`.
    ///
    /// This will, if needed:
    /// * update the cursor
    /// * copy text to the clipboard
    /// * open any clicked urls
    /// * update the IME
    /// *
    pub fn handle_platform_output(
        &mut self,
        window: &winit::window::Window,
        egui_ctx: &egui::Context,
        platform_output: egui::PlatformOutput,
    ) {
        let egui::PlatformOutput {
            cursor_icon,
            open_url,
            events: _,                    // handled above
            mutable_text_under_cursor: _, // only used in eframe web
            text_cursor_pos,
            ..
        } = platform_output;
        self.pixels_per_point = egui_ctx.pixels_per_point(); // someone can have changed it to scale the UI

        self.set_cursor_icon(window, cursor_icon);

        if let Some(_open_url) = open_url {
            log::warn!("Cannot open url");
        }

        if let Some(egui::Pos2 { x, y }) = text_cursor_pos {
            window.set_ime_position(winit::dpi::LogicalPosition { x, y });
        }
    }

    fn set_cursor_icon(&mut self, window: &winit::window::Window, cursor_icon: egui::CursorIcon) {
        // prevent flickering near frame boundary when Windows OS tries to control cursor icon for window resizing
        #[cfg(windows)]
        if self.current_cursor_icon == cursor_icon {
            return;
        }
        self.current_cursor_icon = cursor_icon;

        if let Some(cursor_icon) = translate_cursor(cursor_icon) {
            window.set_cursor_visible(true);

            let is_pointer_in_window = self.pointer_pos_in_points.is_some();
            if is_pointer_in_window {
                window.set_cursor_icon(cursor_icon);
            }
        } else {
            window.set_cursor_visible(false);
        }
    }

    fn on_cursor_moved(&mut self, pos_in_pixels: winit::dpi::PhysicalPosition<f64>) {
        let pos_in_points = egui::pos2(
            pos_in_pixels.x as f32 / self.pixels_per_point,
            pos_in_pixels.y as f32 / self.pixels_per_point,
        );
        self.pointer_pos_in_points = Some(pos_in_points);

        self.egui_input
            .events
            .push(egui::Event::PointerMoved(pos_in_points));
    }

    fn on_mouse_wheel(&mut self, delta: winit::event::MouseScrollDelta) {
        let delta = match delta {
            winit::event::MouseScrollDelta::LineDelta(x, y) => {
                let points_per_scroll_line = 50.0; // Scroll speed decided by consensus: https://github.com/emilk/egui/issues/461
                egui::vec2(x, y) * points_per_scroll_line
            }
            winit::event::MouseScrollDelta::PixelDelta(delta) => {
                egui::vec2(delta.x as f32, delta.y as f32) / self.pixels_per_point
            }
        };

        if self.egui_input.modifiers.ctrl || self.egui_input.modifiers.command {
            // Treat as zoom instead:
            let factor = (delta.y / 200.0).exp();
            self.egui_input.events.push(egui::Event::Zoom(factor));
        } else if self.egui_input.modifiers.shift {
            // Treat as horizontal scrolling.
            // Note: one Mac we already get horizontal scroll events when shift is down.
            self.egui_input
                .events
                .push(egui::Event::Scroll(egui::vec2(delta.x + delta.y, 0.0)));
        } else {
            self.egui_input.events.push(egui::Event::Scroll(delta));
        }
    }

    fn on_keyboard_input(&mut self, input: &winit::event::KeyboardInput) {
        if let Some(keycode) = input.virtual_keycode {
            let pressed = input.state == winit::event::ElementState::Pressed;

            if pressed {
                // VirtualKeyCode::Paste etc in winit are broken/untrustworthy,
                // so we detect these things manually:
                if is_cut_command(self.egui_input.modifiers, keycode) {
                    self.egui_input.events.push(egui::Event::Cut);
                } else if is_copy_command(self.egui_input.modifiers, keycode) {
                    self.egui_input.events.push(egui::Event::Copy);
                }
            }

            if let Some(key) = translate_virtual_key_code(keycode) {
                self.egui_input.events.push(egui::Event::Key {
                    key,
                    pressed,
                    modifiers: self.egui_input.modifiers,
                });
            }
        }
    }

    fn on_mouse_button_input(
        &mut self,
        state: winit::event::ElementState,
        button: winit::event::MouseButton,
    ) {
        if let Some(pos) = self.pointer_pos_in_points {
            if let Some(button) = translate_mouse_button(button) {
                let pressed = state == winit::event::ElementState::Pressed;

                self.egui_input.events.push(egui::Event::PointerButton {
                    pos,
                    button,
                    pressed,
                    modifiers: self.egui_input.modifiers,
                });
            }
        }
    }
}

pub fn screen_size_in_pixels(window: &winit::window::Window) -> egui::Vec2 {
    let size = window.inner_size();
    egui::vec2(size.width as f32, size.height as f32)
}

/// Winit sends special keys (backspace, delete, F1, …) as characters.
/// Ignore those.
/// We also ignore '\r', '\n', '\t'.
/// Newlines are handled by the `Key::Enter` event.
fn is_printable_char(chr: char) -> bool {
    let is_in_private_use_area = ('\u{e000}'..='\u{f8ff}').contains(&chr)
        || ('\u{f0000}'..='\u{ffffd}').contains(&chr)
        || ('\u{100000}'..='\u{10fffd}').contains(&chr);

    !is_in_private_use_area && !chr.is_ascii_control()
}

fn is_cut_command(modifiers: egui::Modifiers, keycode: winit::event::VirtualKeyCode) -> bool {
    (modifiers.command && keycode == winit::event::VirtualKeyCode::X)
        || (cfg!(target_os = "windows")
            && modifiers.shift
            && keycode == winit::event::VirtualKeyCode::Delete)
}

fn is_copy_command(modifiers: egui::Modifiers, keycode: winit::event::VirtualKeyCode) -> bool {
    (modifiers.command && keycode == winit::event::VirtualKeyCode::C)
        || (cfg!(target_os = "windows")
            && modifiers.ctrl
            && keycode == winit::event::VirtualKeyCode::Insert)
}

fn translate_mouse_button(button: winit::event::MouseButton) -> Option<egui::PointerButton> {
    match button {
        winit::event::MouseButton::Left => Some(egui::PointerButton::Primary),
        winit::event::MouseButton::Right => Some(egui::PointerButton::Secondary),
        winit::event::MouseButton::Middle => Some(egui::PointerButton::Middle),
        winit::event::MouseButton::Other(1) => Some(egui::PointerButton::Extra1),
        winit::event::MouseButton::Other(2) => Some(egui::PointerButton::Extra2),
        winit::event::MouseButton::Other(_) => None,
    }
}

fn translate_virtual_key_code(key: winit::event::VirtualKeyCode) -> Option<egui::Key> {
    use egui::Key;
    use winit::event::VirtualKeyCode;

    Some(match key {
        VirtualKeyCode::Down => Key::ArrowDown,
        VirtualKeyCode::Left => Key::ArrowLeft,
        VirtualKeyCode::Right => Key::ArrowRight,
        VirtualKeyCode::Up => Key::ArrowUp,

        VirtualKeyCode::Escape => Key::Escape,
        VirtualKeyCode::Tab => Key::Tab,
        VirtualKeyCode::Back => Key::Backspace,
        VirtualKeyCode::Return => Key::Enter,
        VirtualKeyCode::Space => Key::Space,

        VirtualKeyCode::Insert => Key::Insert,
        VirtualKeyCode::Delete => Key::Delete,
        VirtualKeyCode::Home => Key::Home,
        VirtualKeyCode::End => Key::End,
        VirtualKeyCode::PageUp => Key::PageUp,
        VirtualKeyCode::PageDown => Key::PageDown,

        VirtualKeyCode::Key0 | VirtualKeyCode::Numpad0 => Key::Num0,
        VirtualKeyCode::Key1 | VirtualKeyCode::Numpad1 => Key::Num1,
        VirtualKeyCode::Key2 | VirtualKeyCode::Numpad2 => Key::Num2,
        VirtualKeyCode::Key3 | VirtualKeyCode::Numpad3 => Key::Num3,
        VirtualKeyCode::Key4 | VirtualKeyCode::Numpad4 => Key::Num4,
        VirtualKeyCode::Key5 | VirtualKeyCode::Numpad5 => Key::Num5,
        VirtualKeyCode::Key6 | VirtualKeyCode::Numpad6 => Key::Num6,
        VirtualKeyCode::Key7 | VirtualKeyCode::Numpad7 => Key::Num7,
        VirtualKeyCode::Key8 | VirtualKeyCode::Numpad8 => Key::Num8,
        VirtualKeyCode::Key9 | VirtualKeyCode::Numpad9 => Key::Num9,

        VirtualKeyCode::A => Key::A,
        VirtualKeyCode::B => Key::B,
        VirtualKeyCode::C => Key::C,
        VirtualKeyCode::D => Key::D,
        VirtualKeyCode::E => Key::E,
        VirtualKeyCode::F => Key::F,
        VirtualKeyCode::G => Key::G,
        VirtualKeyCode::H => Key::H,
        VirtualKeyCode::I => Key::I,
        VirtualKeyCode::J => Key::J,
        VirtualKeyCode::K => Key::K,
        VirtualKeyCode::L => Key::L,
        VirtualKeyCode::M => Key::M,
        VirtualKeyCode::N => Key::N,
        VirtualKeyCode::O => Key::O,
        VirtualKeyCode::P => Key::P,
        VirtualKeyCode::Q => Key::Q,
        VirtualKeyCode::R => Key::R,
        VirtualKeyCode::S => Key::S,
        VirtualKeyCode::T => Key::T,
        VirtualKeyCode::U => Key::U,
        VirtualKeyCode::V => Key::V,
        VirtualKeyCode::W => Key::W,
        VirtualKeyCode::X => Key::X,
        VirtualKeyCode::Y => Key::Y,
        VirtualKeyCode::Z => Key::Z,

        VirtualKeyCode::F1 => Key::F1,
        VirtualKeyCode::F2 => Key::F2,
        VirtualKeyCode::F3 => Key::F3,
        VirtualKeyCode::F4 => Key::F4,
        VirtualKeyCode::F5 => Key::F5,
        VirtualKeyCode::F6 => Key::F6,
        VirtualKeyCode::F7 => Key::F7,
        VirtualKeyCode::F8 => Key::F8,
        VirtualKeyCode::F9 => Key::F9,
        VirtualKeyCode::F10 => Key::F10,
        VirtualKeyCode::F11 => Key::F11,
        VirtualKeyCode::F12 => Key::F12,
        VirtualKeyCode::F13 => Key::F13,
        VirtualKeyCode::F14 => Key::F14,
        VirtualKeyCode::F15 => Key::F15,
        VirtualKeyCode::F16 => Key::F16,
        VirtualKeyCode::F17 => Key::F17,
        VirtualKeyCode::F18 => Key::F18,
        VirtualKeyCode::F19 => Key::F19,
        VirtualKeyCode::F20 => Key::F20,

        _ => {
            return None;
        }
    })
}

fn translate_cursor(cursor_icon: egui::CursorIcon) -> Option<winit::window::CursorIcon> {
    match cursor_icon {
        egui::CursorIcon::None => None,

        egui::CursorIcon::Alias => Some(winit::window::CursorIcon::Alias),
        egui::CursorIcon::AllScroll => Some(winit::window::CursorIcon::AllScroll),
        egui::CursorIcon::Cell => Some(winit::window::CursorIcon::Cell),
        egui::CursorIcon::ContextMenu => Some(winit::window::CursorIcon::ContextMenu),
        egui::CursorIcon::Copy => Some(winit::window::CursorIcon::Copy),
        egui::CursorIcon::Crosshair => Some(winit::window::CursorIcon::Crosshair),
        egui::CursorIcon::Default => Some(winit::window::CursorIcon::Default),
        egui::CursorIcon::Grab => Some(winit::window::CursorIcon::Grab),
        egui::CursorIcon::Grabbing => Some(winit::window::CursorIcon::Grabbing),
        egui::CursorIcon::Help => Some(winit::window::CursorIcon::Help),
        egui::CursorIcon::Move => Some(winit::window::CursorIcon::Move),
        egui::CursorIcon::NoDrop => Some(winit::window::CursorIcon::NoDrop),
        egui::CursorIcon::NotAllowed => Some(winit::window::CursorIcon::NotAllowed),
        egui::CursorIcon::PointingHand => Some(winit::window::CursorIcon::Hand),
        egui::CursorIcon::Progress => Some(winit::window::CursorIcon::Progress),

        egui::CursorIcon::ResizeHorizontal => Some(winit::window::CursorIcon::EwResize),
        egui::CursorIcon::ResizeNeSw => Some(winit::window::CursorIcon::NeswResize),
        egui::CursorIcon::ResizeNwSe => Some(winit::window::CursorIcon::NwseResize),
        egui::CursorIcon::ResizeVertical => Some(winit::window::CursorIcon::NsResize),

        egui::CursorIcon::ResizeEast => Some(winit::window::CursorIcon::EResize),
        egui::CursorIcon::ResizeSouthEast => Some(winit::window::CursorIcon::SeResize),
        egui::CursorIcon::ResizeSouth => Some(winit::window::CursorIcon::SResize),
        egui::CursorIcon::ResizeSouthWest => Some(winit::window::CursorIcon::SwResize),
        egui::CursorIcon::ResizeWest => Some(winit::window::CursorIcon::WResize),
        egui::CursorIcon::ResizeNorthWest => Some(winit::window::CursorIcon::NwResize),
        egui::CursorIcon::ResizeNorth => Some(winit::window::CursorIcon::NResize),
        egui::CursorIcon::ResizeNorthEast => Some(winit::window::CursorIcon::NeResize),
        egui::CursorIcon::ResizeColumn => Some(winit::window::CursorIcon::ColResize),
        egui::CursorIcon::ResizeRow => Some(winit::window::CursorIcon::RowResize),

        egui::CursorIcon::Text => Some(winit::window::CursorIcon::Text),
        egui::CursorIcon::VerticalText => Some(winit::window::CursorIcon::VerticalText),
        egui::CursorIcon::Wait => Some(winit::window::CursorIcon::Wait),
        egui::CursorIcon::ZoomIn => Some(winit::window::CursorIcon::ZoomIn),
        egui::CursorIcon::ZoomOut => Some(winit::window::CursorIcon::ZoomOut),
    }
}
