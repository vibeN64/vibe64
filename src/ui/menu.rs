//! The in-game menu. It pauses the game and shows a short list of actions over the
//! picture, so that a game in fullscreen can be saved, reset or left without knowing the
//! keyboard shortcuts. Esc opens and closes it, and so does hotkey + Start on a controller.
//!
//! This file decides what the menu offers and does; the panel itself is drawn by
//! rdp_menu_show in parallel-rdp/interface.cpp.

use crate::{device, retroachievements, ui};

#[derive(Clone, Copy, PartialEq)]
enum Item {
    Resume,
    SaveState,
    LoadState,
    StateSlot,
    SwitchPak,
    FastForward,
    Fullscreen,
    Reset,
    Quit,
}

/// What is held down, on player 1's controller or the keyboard
#[derive(Clone, Copy, PartialEq)]
struct Buttons {
    up: bool,
    down: bool,
    left: bool,
    right: bool,
    accept: bool,
    back: bool,
}

impl Buttons {
    const ALL: Buttons = Buttons {
        up: true,
        down: true,
        left: true,
        right: true,
        accept: true,
        back: true,
    };

    fn any(&self) -> bool {
        self.up || self.down || self.left || self.right || self.accept || self.back
    }
}

fn read_buttons(ui: &mut ui::Ui) -> Buttons {
    const STICK_THRESHOLD: i8 = 40;

    let keys = ui::input::get(ui, 0).data;
    let button = |button: usize| keys & (1 << button) != 0;
    let stick_x = (keys >> ui::input::X_AXIS_SHIFT) as u8 as i8;
    let stick_y = (keys >> ui::input::Y_AXIS_SHIFT) as u8 as i8;
    let key = |scancode: sdl3_sys::scancode::SDL_Scancode| unsafe {
        *ui.input.keyboard_state.offset(i32::from(scancode) as isize)
    };

    // Start is left out on purpose: it is half of the combination that opens the menu,
    // and would choose the first item as soon as the hotkey was let go.
    Buttons {
        up: button(ui::input_profile::U_DPAD)
            || stick_y > STICK_THRESHOLD
            || key(sdl3_sys::scancode::SDL_SCANCODE_UP),
        down: button(ui::input_profile::D_DPAD)
            || stick_y < -STICK_THRESHOLD
            || key(sdl3_sys::scancode::SDL_SCANCODE_DOWN),
        left: button(ui::input_profile::L_DPAD)
            || stick_x < -STICK_THRESHOLD
            || key(sdl3_sys::scancode::SDL_SCANCODE_LEFT),
        right: button(ui::input_profile::R_DPAD)
            || stick_x > STICK_THRESHOLD
            || key(sdl3_sys::scancode::SDL_SCANCODE_RIGHT),
        accept: button(ui::input_profile::A_BUTTON) || key(sdl3_sys::scancode::SDL_SCANCODE_RETURN),
        back: button(ui::input_profile::B_BUTTON),
    }
}

fn pak_name(device: &device::Device) -> Option<&'static str> {
    device.pif.channels[0]
        .pak_handler
        .map(|handler| match handler.pak_type {
            device::controller::PakType::RumblePak => "Rumble Pak",
            device::controller::PakType::TransferPak => "Transfer Pak",
            _ => "Memory Pak",
        })
}

fn on_off(on: bool) -> &'static str {
    if on { "On" } else { "Off" }
}

fn label(device: &device::Device, item: Item, fast_forward: bool) -> String {
    let slot = device.ui.storage.save_state_slot;
    match item {
        Item::Resume => "Resume".to_string(),
        Item::SaveState => format!("Save state (slot {slot})"),
        Item::LoadState => format!("Load state (slot {slot})"),
        Item::StateSlot => format!("State slot: {slot}"),
        Item::SwitchPak => format!(
            "Switch controller pak (now: {})",
            pak_name(device).unwrap_or("none")
        ),
        Item::FastForward => format!("Fast forward: {}", on_off(fast_forward)),
        Item::Fullscreen => format!("Fullscreen: {}", on_off(ui::video::is_fullscreen())),
        Item::Reset => "Reset game".to_string(),
        Item::Quit => "Quit game".to_string(),
    }
}

/// One step of the menu loop: draw, wait a frame, and let SDL and RetroAchievements tick
fn frame() {
    ui::video::render_frame();
    ui::video::update_screen();
    std::thread::sleep(std::time::Duration::from_millis(16));
    unsafe { sdl3_sys::events::SDL_PumpEvents() };
    retroachievements::do_idle();
}

pub fn run(device: &mut device::Device) {
    // a pad that was shaking would go on shaking for as long as the menu is open
    for channel in 0..device.ui.input.controllers.len() {
        ui::input::set_rumble(&device.ui, channel, 0);
    }

    let mut items = vec![
        Item::Resume,
        Item::SaveState,
        Item::LoadState,
        Item::StateSlot,
    ];
    if pak_name(device).is_some() {
        items.push(Item::SwitchPak);
    }
    items.extend([Item::FastForward, Item::Fullscreen, Item::Reset, Item::Quit]);

    let mut selected = 0;
    let mut fast_forward = !device.speed_limiter.enabled;
    let mut shown: Option<(Vec<String>, usize)> = None;
    // Whatever is held as the menu opens does nothing until it has been let go
    let mut previous = Buttons::ALL;

    loop {
        let labels: Vec<String> = items
            .iter()
            .map(|item| label(device, *item, fast_forward))
            .collect();
        if shown.as_ref() != Some(&(labels.clone(), selected)) {
            ui::video::show_menu(
                "Paused",
                &labels,
                selected,
                "Up/Down: move    A or Enter: choose    Left/Right: change    B or Esc: back",
            );
            shown = Some((labels, selected));
        }
        frame();

        let (game_wanted, menu_key) = ui::video::poll_menu();
        if !game_wanted || menu_key {
            break;
        }

        let buttons = read_buttons(&mut device.ui);
        let pressed = Buttons {
            up: buttons.up && !previous.up,
            down: buttons.down && !previous.down,
            left: buttons.left && !previous.left,
            right: buttons.right && !previous.right,
            accept: buttons.accept && !previous.accept,
            back: buttons.back && !previous.back,
        };
        previous = buttons;

        if pressed.back {
            break;
        }
        if pressed.up {
            selected = (selected + items.len() - 1) % items.len();
        }
        if pressed.down {
            selected = (selected + 1) % items.len();
        }

        let item = items[selected];
        if item == Item::StateSlot && (pressed.left || pressed.right || pressed.accept) {
            let slot = device.ui.storage.save_state_slot;
            let slot = if pressed.left {
                (slot + 9) % 10
            } else {
                (slot + 1) % 10
            };
            ui::video::set_save_state_slot(device, slot);
        } else if pressed.accept {
            match item {
                Item::Resume => break,
                Item::SaveState => {
                    device.savestate.save_state = true;
                    break;
                }
                Item::LoadState => {
                    device.savestate.load_state = true;
                    break;
                }
                Item::SwitchPak => {
                    // the new pak goes in half a second into the game, with a message
                    device::controller::request_pak_change(device, 0);
                    break;
                }
                Item::FastForward => {
                    ui::input::push_user_event(ui::input::USER_EVENT_FAST_FORWARD);
                    fast_forward = !fast_forward;
                }
                Item::Fullscreen => ui::video::toggle_fullscreen(),
                Item::Reset => {
                    ui::video::reset_game(device);
                    break;
                }
                Item::Quit => {
                    ui::input::push_user_event(ui::input::USER_EVENT_EXIT_GAME);
                    break;
                }
                Item::StateSlot => {}
            }
        }
    }

    ui::video::hide_menu();

    // Wait for the buttons to be let go (for a second at most), so that the game does not
    // also get the press that chose an item or closed the menu.
    for _ in 0..60 {
        frame();
        ui::video::poll_menu();
        if !read_buttons(&mut device.ui).any() {
            break;
        }
    }
}
