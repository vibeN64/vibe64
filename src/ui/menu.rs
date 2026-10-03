//! The in-game menu. It pauses the game and shows a short list of actions over the
//! picture, so that a game in fullscreen can be saved, reset or left without knowing the
//! keyboard shortcuts. Esc opens and closes it, and so does hotkey + Start on a controller.
//!
//! This file decides what the menu offers and does; the panel itself is drawn by
//! rdp_menu_show in parallel-rdp/interface.cpp.

use crate::{device, retroachievements, ui};

#[derive(Clone, Copy, PartialEq)]
enum Item {
    // the main screen
    Resume,
    SaveState,
    LoadState,
    StateSlot,
    Controllers,
    FastForward,
    Fullscreen,
    Reset,
    Quit,
    // the controllers screen; the number is the N64 port, starting at 0
    Pad(usize),
    Profile(usize),
    Pak(usize),
    Rumble,
    Back,
}

#[derive(Clone, Copy, PartialEq)]
enum Screen {
    Main,
    Controllers,
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

fn pak_name(device: &device::Device, port: usize) -> Option<&'static str> {
    device.pif.channels[port]
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

/// The N64 ports with a controller plugged in (the ones the settings enable)
fn active_ports(device: &device::Device) -> Vec<usize> {
    (0..4)
        .filter(|&port| {
            device.pif.channels[port].process.is_some()
                && !(port == 3 && device.ui.config.input.emulate_vru)
        })
        .collect()
}

/// The input profiles in the order the launcher lists them: "default" first
fn profile_names(device: &device::Device) -> Vec<String> {
    let mut names: Vec<String> = device
        .ui
        .config
        .input
        .input_profiles
        .keys()
        .cloned()
        .collect();
    if let Some(position) = names.iter().position(|name| name == "default") {
        let default = names.remove(position);
        names.insert(0, default);
    }
    names
}

fn profile_label(device: &device::Device, port: usize) -> String {
    let name = &device.ui.config.input.input_profile_binding[port];
    if name == "default" && device.ui.input.controllers[port].nso_n64 {
        // see get_nso_n64_profile: the default layout cannot work on this controller
        "default (NSO N64 layout)".to_string()
    } else {
        name.clone()
    }
}

fn items(device: &device::Device, screen: Screen) -> Vec<Item> {
    match screen {
        Screen::Main => vec![
            Item::Resume,
            Item::SaveState,
            Item::LoadState,
            Item::StateSlot,
            Item::Controllers,
            Item::FastForward,
            Item::Fullscreen,
            Item::Reset,
            Item::Quit,
        ],
        Screen::Controllers => {
            let mut items = vec![];
            for port in active_ports(device) {
                items.extend([Item::Pad(port), Item::Profile(port), Item::Pak(port)]);
            }
            items.extend([Item::Rumble, Item::Back]);
            items
        }
    }
}

fn label(device: &device::Device, item: Item, fast_forward: bool) -> String {
    let slot = device.ui.storage.save_state_slot;
    match item {
        Item::Resume => "Resume".to_string(),
        Item::SaveState => format!("Save state (slot {slot})"),
        Item::LoadState => format!("Load state (slot {slot})"),
        Item::StateSlot => format!("State slot: {slot}"),
        Item::Controllers => "Controllers...".to_string(),
        Item::FastForward => format!("Fast forward: {}", on_off(fast_forward)),
        Item::Fullscreen => format!("Fullscreen: {}", on_off(ui::video::is_fullscreen())),
        Item::Reset => "Reset game".to_string(),
        Item::Quit => "Quit game".to_string(),
        Item::Pad(port) => format!(
            "Player {} pad: {}",
            port + 1,
            ui::input::port_pad_name(&device.ui, port)
                .unwrap_or_else(|| "none connected".to_string())
        ),
        Item::Profile(port) => {
            format!(
                "Player {} profile: {}",
                port + 1,
                profile_label(device, port)
            )
        }
        Item::Pak(port) => format!(
            "Player {} pak: {}",
            port + 1,
            pak_name(device, port).unwrap_or("changing...")
        ),
        Item::Rumble => format!("Rumble: {}", on_off(device.ui.config.input.rumble)),
        Item::Back => "Back".to_string(),
    }
}

/// The next (or previous) entry of a list that wraps around
fn step(len: usize, current: Option<usize>, forward: bool) -> usize {
    match (current, forward) {
        (Some(i), true) => (i + 1) % len,
        (Some(i), false) => (i + len - 1) % len,
        (None, true) => 0,
        (None, false) => len - 1,
    }
}

fn change_pad(device: &mut device::Device, port: usize, forward: bool) {
    let pads = ui::input::pads_for_port(&device.ui, port);
    if pads.is_empty() {
        return;
    }
    let current = ui::input::port_pad_id(&device.ui, port)
        .and_then(|id| pads.iter().position(|(pad, _)| *pad == id));
    let (joystick_id, _) = pads[step(pads.len(), current, forward)];
    ui::input::assign_pad(&mut device.ui, port, joystick_id);
}

fn change_profile(device: &mut device::Device, port: usize, forward: bool) {
    let names = profile_names(device);
    if names.is_empty() {
        return;
    }
    let current = names
        .iter()
        .position(|name| *name == device.ui.config.input.input_profile_binding[port]);
    let name = names[step(names.len(), current, forward)].clone();
    ui::input::set_profile(&mut device.ui, port, &name);
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

    let mut screen = Screen::Main;
    let mut selected = [0usize; 2]; // per screen, so that coming back finds its place
    let mut fast_forward = !device.speed_limiter.enabled;
    let mut shown: Option<(Screen, Vec<String>, usize)> = None;
    // Whatever is held as the menu opens does nothing until it has been let go
    let mut previous = Buttons::ALL;

    loop {
        let items = items(device, screen);
        let current = &mut selected[screen as usize];
        *current = (*current).min(items.len() - 1);

        let labels: Vec<String> = items
            .iter()
            .map(|item| label(device, *item, fast_forward))
            .collect();
        if shown.as_ref() != Some(&(screen, labels.clone(), *current)) {
            ui::video::show_menu(
                match screen {
                    Screen::Main => "Paused",
                    Screen::Controllers => "Controllers",
                },
                &labels,
                *current,
                "Up/Down: move    A or Enter: choose    Left/Right: change    B: back    Esc: close",
            );
            shown = Some((screen, labels, *current));
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
            if screen == Screen::Main {
                break;
            }
            screen = Screen::Main;
            continue;
        }
        let current = &mut selected[screen as usize];
        if pressed.up {
            *current = (*current + items.len() - 1) % items.len();
        }
        if pressed.down {
            *current = (*current + 1) % items.len();
        }

        let item = items[*current];
        let forward = !pressed.left;
        let changed = pressed.left || pressed.right || pressed.accept;
        match item {
            Item::StateSlot if changed => {
                let slot = device.ui.storage.save_state_slot;
                let slot = if forward {
                    (slot + 1) % 10
                } else {
                    (slot + 9) % 10
                };
                ui::video::set_save_state_slot(device, slot);
            }
            Item::Pad(port) if changed => change_pad(device, port, forward),
            Item::Profile(port) if changed => change_profile(device, port, forward),
            Item::Rumble if changed => {
                device.ui.config.input.rumble = !device.ui.config.input.rumble;
            }
            _ if pressed.accept => match item {
                Item::Resume => break,
                Item::SaveState => {
                    device.savestate.save_state = true;
                    break;
                }
                Item::LoadState => {
                    device.savestate.load_state = true;
                    break;
                }
                Item::Controllers => screen = Screen::Controllers,
                Item::Back => screen = Screen::Main,
                Item::Pak(port) => {
                    // the new pak goes in half a second into the game, with a message
                    device::controller::request_pak_change(device, port);
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
                // changed above
                Item::StateSlot | Item::Pad(_) | Item::Profile(_) | Item::Rumble => {}
            },
            _ => {}
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
