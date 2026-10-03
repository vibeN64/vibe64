//! The in-game menu. It pauses the game and shows a short list of actions over the
//! picture, so that a game in fullscreen can be saved, reset or left without knowing the
//! keyboard shortcuts. Esc opens and closes it, and so does hotkey + Start on a controller.
//!
//! This file decides what the menu offers and does; the panel itself is drawn by
//! rdp_menu_show in parallel-rdp/interface.cpp.

use crate::{cheats, device, retroachievements, ui};

#[derive(Clone, Copy, PartialEq)]
enum Item {
    // the main screen
    Resume,
    SaveState,
    LoadState,
    StateSlot,
    Controllers,
    Cheats,
    FastForward,
    Fullscreen,
    Shader,
    Reset,
    Quit,
    // the controllers screen; the number is the N64 port, starting at 0
    Pad(usize),
    Profile(usize),
    Pak(usize),
    Rumble,
    Back,
    // the cheats screen; the number is the place of the cheat in the game's list
    CheatsMaster,
    Cheat(usize),
    NoCheats,
}

#[derive(Clone, Copy, PartialEq)]
enum Screen {
    Main,
    Controllers,
    Cheats,
}

/// The cheats the built-in database knows for the game that is running, and which of them are
/// switched on. Loaded when the Cheats screen is first opened: the database is large.
#[derive(Default)]
struct CheatList {
    loaded: bool,
    /// the game, as the cheat settings file knows it
    game: String,
    available: std::collections::BTreeMap<String, cheats::CheatData>,
    /// the names in `available`, in the order they are shown
    names: Vec<String>,
    /// the chosen cheats, each with its option if it has options
    chosen: rustc_hash::FxHashMap<String, Option<String>>,
}

impl CheatList {
    fn load(device: &device::Device) -> CheatList {
        let game = ui::storage::get_game_crc(&device.cart.rom);
        let available = cheats::game_cheats(&device.cart.rom);
        let names = available.keys().cloned().collect();
        let chosen = ui::config::Cheats::new()
            .cheats
            .remove(&game)
            .unwrap_or_default();
        CheatList {
            loaded: true,
            game,
            available,
            names,
            chosen,
        }
    }

    fn options(&self, index: usize) -> Vec<String> {
        self.available[&self.names[index]]
            .options
            .as_ref()
            .map(|options| options.keys().cloned().collect())
            .unwrap_or_default()
    }

    fn label(&self, index: usize) -> String {
        let name = &self.names[index];
        let chosen = self.chosen.get(name);
        let mark = if chosen.is_some() { "[x]" } else { "[ ]" };
        let options = self.options(index);
        let label = if options.is_empty() {
            format!("{mark} {name}")
        } else {
            // the option in force, or the one that switching the cheat on would use
            let option = chosen
                .and_then(|option| option.clone())
                .unwrap_or_else(|| options[0].clone());
            format!("{mark} {name}: {option}")
        };
        shorten(&label, 64)
    }

    /// A press on a cheat: A, or Left/Right on one without options, switches it on or off.
    /// Left/Right on one with options picks the previous or next option, and switches it on.
    fn change(&mut self, index: usize, pick_option: Option<bool>) {
        let name = self.names[index].clone();
        let options = self.options(index);
        match pick_option {
            Some(forward) if !options.is_empty() => {
                let current = self
                    .chosen
                    .get(&name)
                    .and_then(|option| option.as_ref())
                    .and_then(|option| options.iter().position(|o| o == option));
                let next = options[step(options.len(), current, forward)].clone();
                self.chosen.insert(name, Some(next));
            }
            _ => {
                if self.chosen.remove(&name).is_none() {
                    self.chosen
                        .insert(name, options.first().map(|option| option.to_string()));
                }
            }
        }
    }

    /// Saves the choice with the other settings (the launcher's Cheats page reads the same
    /// file) and makes it the running cheats.
    fn save_and_apply(&self, device: &mut device::Device) {
        let mut saved = ui::config::Cheats::new();
        saved.cheats.insert(self.game.clone(), self.chosen.clone());
        cheats::apply(device, &self.available, &self.chosen);
    }
}

/// `text` cut to at most `max` characters, with ... where it was cut
fn shorten(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        let cut: String = text.chars().take(max - 3).collect();
        format!("{}...", cut.trim_end())
    }
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

const CONTROLS_HINT: &str =
    "Up/Down: move    A or Enter: choose    Left/Right: change    B: back    Esc: close";

/// How many rows of a long list show at once; the panel scrolls to keep the selected one
const VISIBLE_ROWS: usize = 11;

/// The rows of a list of `len` that show, with `selected` in the middle where it can be
fn visible_window(len: usize, selected: usize) -> (usize, usize) {
    if len <= VISIBLE_ROWS {
        return (0, len);
    }
    let start = selected
        .saturating_sub(VISIBLE_ROWS / 2)
        .min(len - VISIBLE_ROWS);
    (start, start + VISIBLE_ROWS)
}

fn items(device: &device::Device, screen: Screen, cheat_list: &CheatList) -> Vec<Item> {
    match screen {
        Screen::Main => vec![
            Item::Resume,
            Item::SaveState,
            Item::LoadState,
            Item::StateSlot,
            Item::Controllers,
            Item::Cheats,
            Item::FastForward,
            Item::Fullscreen,
            Item::Shader,
            Item::Reset,
            Item::Quit,
        ],
        Screen::Cheats => {
            if cheat_list.names.is_empty() {
                vec![Item::NoCheats, Item::Back]
            } else {
                let mut items = vec![Item::CheatsMaster];
                items.extend((0..cheat_list.names.len()).map(Item::Cheat));
                items.push(Item::Back);
                items
            }
        }
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

fn label(
    device: &device::Device,
    item: Item,
    fast_forward: bool,
    cheat_list: &CheatList,
) -> String {
    let slot = device.ui.storage.save_state_slot;
    match item {
        Item::Resume => "Resume".to_string(),
        Item::SaveState => format!("Save state (slot {slot})"),
        Item::LoadState => format!("Load state (slot {slot})"),
        Item::StateSlot => format!("State slot: {slot}"),
        Item::Controllers => "Controllers...".to_string(),
        Item::Cheats => "Cheats...".to_string(),
        Item::FastForward => format!("Fast forward: {}", on_off(fast_forward)),
        Item::Fullscreen => format!("Fullscreen: {}", on_off(ui::video::is_fullscreen())),
        Item::Shader => format!("Shader: {}", shader_name(device)),
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
        Item::CheatsMaster => {
            if device.cheats.cheats.is_empty() {
                "Cheats: none switched on".to_string()
            } else {
                format!("Cheats: {}", on_off(device.cheats.enabled))
            }
        }
        Item::Cheat(index) => cheat_list.label(index),
        Item::NoCheats => "No cheats known for this game".to_string(),
    }
}

fn shader_name(device: &device::Device) -> String {
    let shaders = ui::video::shaders();
    shaders
        .get(ui::video::shader_index(&device.ui.config.video.shader) as usize)
        .map(|(_, name)| name.clone())
        .unwrap_or_default()
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
    let mut selected = [0usize; 3]; // per screen, so that coming back finds its place
    let mut fast_forward = !device.speed_limiter.enabled;
    let mut cheat_list = CheatList::default();
    let mut shown: Option<(String, Vec<String>, usize, String)> = None;
    // Whatever is held as the menu opens does nothing until it has been let go
    let mut previous = Buttons::ALL;

    loop {
        let items = items(device, screen, &cheat_list);
        let current = &mut selected[screen as usize];
        *current = (*current).min(items.len() - 1);

        let labels: Vec<String> = items
            .iter()
            .map(|item| label(device, *item, fast_forward, &cheat_list))
            .collect();
        // a long list shows a window of itself, with the place in it in the title
        let (first, last) = visible_window(items.len(), *current);
        let title = match screen {
            Screen::Main => "Paused",
            Screen::Controllers => "Controllers",
            Screen::Cheats => "Cheats",
        };
        let place = match (screen, items[*current]) {
            // on the cheats screen only the cheats count, not the rows around them
            (Screen::Cheats, Item::Cheat(index)) => Some((index + 1, cheat_list.names.len())),
            (Screen::Cheats, _) => None,
            _ => Some((*current + 1, items.len())),
        };
        let title = match place {
            Some((number, total)) if items.len() > VISIBLE_ROWS => {
                format!("{title}   {number}/{total}")
            }
            _ => title.to_string(),
        };
        // under a cheat goes what the database says about it
        let hint = match items[*current] {
            Item::Cheat(index) => {
                let note = &cheat_list.available[&cheat_list.names[index]].note;
                let note = note.split_whitespace().collect::<Vec<_>>().join(" ");
                if note.is_empty() {
                    CONTROLS_HINT.to_string()
                } else {
                    shorten(&note, 72)
                }
            }
            _ => CONTROLS_HINT.to_string(),
        };
        let view = (title, labels[first..last].to_vec(), *current - first, hint);
        if shown.as_ref() != Some(&view) {
            ui::video::show_menu(&view.0, &view.1, view.2, &view.3);
            shown = Some(view);
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
            Item::Shader if changed => {
                // takes effect at once, so the menu shows over the new look
                let count = ui::video::shaders().len();
                let current = ui::video::shader_index(&device.ui.config.video.shader) as usize;
                ui::video::set_shader(device, step(count, Some(current), forward));
            }
            Item::Pad(port) if changed => change_pad(device, port, forward),
            Item::Profile(port) if changed => change_profile(device, port, forward),
            Item::Rumble if changed => {
                device.ui.config.input.rumble = !device.ui.config.input.rumble;
            }
            Item::CheatsMaster if changed => {
                // with nothing switched on there is nothing to turn on
                if !device.cheats.cheats.is_empty() {
                    device.cheats.enabled = !device.cheats.enabled;
                }
            }
            Item::Cheat(index) if changed => {
                let pick_option = if pressed.left {
                    Some(false)
                } else if pressed.right {
                    Some(true)
                } else {
                    None
                };
                cheat_list.change(index, pick_option);
                cheat_list.save_and_apply(device);
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
                Item::Cheats => {
                    if !cheat_list.loaded {
                        cheat_list = CheatList::load(device);
                    }
                    screen = Screen::Cheats;
                }
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
                // changed above, or nothing to do
                Item::StateSlot
                | Item::Shader
                | Item::Pad(_)
                | Item::Profile(_)
                | Item::Rumble
                | Item::CheatsMaster
                | Item::Cheat(_)
                | Item::NoCheats => {}
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
