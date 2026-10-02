use std::ops::Neg;

use crate::ui;

pub const X_AXIS_SHIFT: usize = 16;
pub const Y_AXIS_SHIFT: usize = 24;

// Codes of the SDL user events that hotkeys send to the video code (see sdl_event_filter)
const USER_EVENT_SAVE_STATE: i32 = 1;
const USER_EVENT_LOAD_STATE: i32 = 2;
pub const USER_EVENT_EXIT_GAME: i32 = 3;
pub const USER_EVENT_FAST_FORWARD: i32 = 4;
const USER_EVENT_LOAD_REWIND: i32 = 5;
const USER_EVENT_OPEN_MENU: i32 = 6;

const MAX_AXIS_VALUE: f64 = 85.0;

pub const UNKNOWN_CONTROLLER_NAME: &str = "Unknown controller";

#[derive(Default)]
pub struct Controllers {
    pub game_controller: *mut sdl3_sys::gamepad::SDL_Gamepad,
    pub joystick: *mut sdl3_sys::joystick::SDL_Joystick,
    pub guid: sdl3_sys::guid::SDL_GUID,
    pub last_key_state: u32,
    /// A Nintendo Switch Online N64 controller is plugged into this port
    pub nso_n64: bool,
}

#[derive(Default, PartialEq, Copy, Clone, serde::Serialize, serde::Deserialize)]
pub struct InputData {
    pub data: u32,
    pub pak_change_pressed: bool,
}

fn bound_axis(x: &mut f64, y: &mut f64) {
    let radius = f64::sqrt(70.0 * 70.0 + 70.0 * 70.0); // this is roughly the maximum diagonal distance of the controller

    // Calculate the distance from the origin (0, 0)
    let distance = f64::sqrt((*x) * (*x) + (*y) * (*y));

    // If the distance is greater than the radius, scale the coordinates
    if distance > radius {
        let scale_factor = radius / distance;
        *x *= scale_factor;
        *y *= scale_factor;
    }
}

fn apply_deadzone(x: &mut f64, y: &mut f64, deadzone: i32) {
    let axis_deadzone = MAX_AXIS_VALUE * (deadzone as f64 / 100.0);

    // Calculate the distance from the origin (0, 0)
    let distance = f64::sqrt((*x) * (*x) + (*y) * (*y));

    if distance <= axis_deadzone {
        *x = 0.0;
        *y = 0.0;
        return;
    }

    let new_distance =
        (distance - axis_deadzone) * MAX_AXIS_VALUE / (MAX_AXIS_VALUE - axis_deadzone);
    *x = *x / distance * new_distance;
    *y = *y / distance * new_distance;
}

fn normalize_axis_position(axis_position: i16) -> f64 {
    axis_position as f64 * MAX_AXIS_VALUE / i16::MAX as f64
}

fn set_axis(
    profile: &ui::config::InputProfile,
    joystick: *mut sdl3_sys::joystick::SDL_Joystick,
    controller: *mut sdl3_sys::gamepad::SDL_Gamepad,
    keyboard_state: *const bool,
) -> (f64, f64) {
    let mut x = 0.0;
    let mut y = 0.0;
    let axes = [
        ui::input_profile::AXIS_LEFT,
        ui::input_profile::AXIS_RIGHT,
        ui::input_profile::AXIS_DOWN,
        ui::input_profile::AXIS_UP,
    ];
    let mut has_deadzone = false;
    for axis in axes {
        for input in profile.inputs[axis].iter() {
            if !controller.is_null()
                && let Some(ui::config::InputItem::ControllerAxis(controller_axis)) = input
            {
                let axis_position = unsafe {
                    sdl3_sys::gamepad::SDL_GetGamepadAxis(
                        controller,
                        sdl3_sys::gamepad::SDL_GamepadAxis(controller_axis.id),
                    )
                };
                if axis_position as isize * controller_axis.axis as isize > 0 {
                    let axis_value = if axis == ui::input_profile::AXIS_LEFT
                        || axis == ui::input_profile::AXIS_RIGHT
                    {
                        &mut x
                    } else {
                        &mut y
                    };
                    *axis_value = normalize_axis_position(axis_position);
                    has_deadzone = true;
                }
            } else if !joystick.is_null()
                && let Some(ui::config::InputItem::JoystickAxis(joystick_axis)) = input
            {
                let axis_position =
                    unsafe { sdl3_sys::joystick::SDL_GetJoystickAxis(joystick, joystick_axis.id) };
                if axis_position as isize * joystick_axis.axis as isize > 0 {
                    let axis_value = if axis == ui::input_profile::AXIS_LEFT
                        || axis == ui::input_profile::AXIS_RIGHT
                    {
                        &mut x
                    } else {
                        &mut y
                    };
                    *axis_value = normalize_axis_position(axis_position);
                    has_deadzone = true;
                }
            } else if let Some(ui::config::InputItem::Key(key)) = input
                && unsafe { *keyboard_state.offset(key.id as isize) }
            {
                match axis {
                    ui::input_profile::AXIS_LEFT => x = -MAX_AXIS_VALUE,
                    ui::input_profile::AXIS_RIGHT => x = MAX_AXIS_VALUE,
                    ui::input_profile::AXIS_DOWN => y = MAX_AXIS_VALUE,
                    ui::input_profile::AXIS_UP => y = -MAX_AXIS_VALUE,
                    _ => unreachable!(),
                }
            }
        }
    }

    y = y.neg();
    if has_deadzone {
        apply_deadzone(&mut x, &mut y, profile.deadzone);
    }
    (x, y)
}

fn is_controller_button_pressed(
    input: &Option<ui::config::InputItem>,
    joystick: *mut sdl3_sys::joystick::SDL_Joystick,
    controller: *mut sdl3_sys::gamepad::SDL_Gamepad,
) -> bool {
    if !controller.is_null() {
        if let Some(ui::config::InputItem::ControllerButton(controller_button)) = input
            && unsafe {
                sdl3_sys::gamepad::SDL_GetGamepadButton(
                    controller,
                    sdl3_sys::gamepad::SDL_GamepadButton(controller_button.id),
                )
            }
        {
            return true;
        }
        if let Some(ui::config::InputItem::ControllerAxis(controller_axis)) = input {
            let axis_position = unsafe {
                sdl3_sys::gamepad::SDL_GetGamepadAxis(
                    controller,
                    sdl3_sys::gamepad::SDL_GamepadAxis(controller_axis.id),
                )
            };
            if axis_position as isize * controller_axis.axis as isize > 0
                && axis_position.saturating_abs() > i16::MAX / 2
            {
                return true;
            }
        }
    } else if !joystick.is_null() {
        if let Some(ui::config::InputItem::JoystickButton(joystick_button)) = input
            && unsafe { sdl3_sys::joystick::SDL_GetJoystickButton(joystick, joystick_button.id) }
        {
            return true;
        }
        if let Some(ui::config::InputItem::JoystickHat(joystick_hat)) = input
            && (unsafe { sdl3_sys::joystick::SDL_GetJoystickHat(joystick, joystick_hat.id) }
                & joystick_hat.direction)
                != 0
        {
            return true;
        }
        if let Some(ui::config::InputItem::JoystickAxis(joystick_axis)) = input {
            let axis_position =
                unsafe { sdl3_sys::joystick::SDL_GetJoystickAxis(joystick, joystick_axis.id) };
            if (axis_position as isize * joystick_axis.axis as isize > 0
                || joystick_axis.initial_state != 0)
                && axis_position.abs_diff(joystick_axis.initial_state) > (u16::MAX / 4)
            {
                return true;
            }
        }
    }
    false
}

fn set_buttons(
    profile: &ui::config::InputProfile,
    joystick: *mut sdl3_sys::joystick::SDL_Joystick,
    controller: *mut sdl3_sys::gamepad::SDL_Gamepad,
    keyboard_state: *const bool,
    alt_pressed: bool,
) -> u32 {
    let mut keys = 0;
    for i in 0..14 {
        for input in profile.inputs[i].iter() {
            if let Some(ui::config::InputItem::Key(key)) = input
                && !alt_pressed
                && unsafe { *keyboard_state.offset(key.id as isize) }
            {
                keys |= 1 << i;
            }
            if is_controller_button_pressed(input, joystick, controller) {
                keys |= 1 << i;
            }
        }
    }
    keys
}

pub fn set_rumble(ui: &ui::Ui, channel: usize, rumble: u8) {
    let controller = ui.input.controllers[channel].game_controller;
    let joystick = ui.input.controllers[channel].joystick;
    if !controller.is_null() {
        unsafe {
            sdl3_sys::gamepad::SDL_RumbleGamepad(
                controller,
                (rumble & 1) as u16 * u16::MAX,
                (rumble & 1) as u16 * u16::MAX,
                (rumble & 1) as u32 * sdl3_sys::haptic::SDL_HAPTIC_INFINITY,
            )
        };
    } else if !joystick.is_null() {
        unsafe {
            sdl3_sys::joystick::SDL_RumbleJoystick(
                joystick,
                (rumble & 1) as u16 * u16::MAX,
                (rumble & 1) as u16 * u16::MAX,
                (rumble & 1) as u32 * sdl3_sys::haptic::SDL_HAPTIC_INFINITY,
            )
        };
    }
}

fn hotkey_pressed(
    profile: &ui::config::InputProfile,
    joystick: *mut sdl3_sys::joystick::SDL_Joystick,
    controller: *mut sdl3_sys::gamepad::SDL_Gamepad,
) -> bool {
    for input in profile.inputs[ui::input_profile::HOTKEY].iter() {
        if is_controller_button_pressed(input, joystick, controller) {
            return true;
        }
    }
    false
}

pub fn get_controller_names() -> Vec<String> {
    #[cfg(target_os = "android")]
    return ui::android::get_controller_names();

    #[cfg(not(target_os = "android"))]
    {
        let mut controllers: Vec<String> = get_joysticks()
            .iter()
            .map(|joystick| joystick_name(*joystick))
            .collect();
        // A port without a chosen controller takes the first free one (see init)
        controllers.insert(0, "Automatic".into());

        controllers
    }
}

#[cfg(feature = "gui")]
pub fn get_controller_paths() -> Vec<String> {
    #[cfg(target_os = "android")]
    return ui::android::get_controller_paths();

    #[cfg(not(target_os = "android"))]
    {
        let mut controller_paths: Vec<String> = vec![];

        for joystick in get_joysticks().iter() {
            let path = unsafe { sdl3_sys::joystick::SDL_GetJoystickPathForID(*joystick) };
            controller_paths.push(if path.is_null() {
                String::new()
            } else {
                unsafe { std::ffi::CStr::from_ptr(path).to_str().unwrap() }.to_string()
            });
        }
        controller_paths.insert(0, String::new());

        controller_paths
    }
}

fn joystick_name(joystick_id: sdl3_sys::joystick::SDL_JoystickID) -> String {
    let name = unsafe { sdl3_sys::joystick::SDL_GetJoystickNameForID(joystick_id) };
    if name.is_null() {
        UNKNOWN_CONTROLLER_NAME.to_string()
    } else {
        unsafe { std::ffi::CStr::from_ptr(name).to_str().unwrap() }.to_string()
    }
}

/// What a port's saved controller assignment is compared against
fn joystick_path(joystick_id: sdl3_sys::joystick::SDL_JoystickID) -> Option<String> {
    if cfg!(target_os = "android") {
        let vendor_id = unsafe { sdl3_sys::joystick::SDL_GetJoystickVendorForID(joystick_id) };
        let product_id = unsafe { sdl3_sys::joystick::SDL_GetJoystickProductForID(joystick_id) };
        Some(format!(
            "{}:{}:{}",
            joystick_name(joystick_id),
            vendor_id,
            product_id
        ))
    } else {
        let path = unsafe { sdl3_sys::joystick::SDL_GetJoystickPathForID(joystick_id) };
        if path.is_null() {
            None
        } else {
            Some(unsafe { std::ffi::CStr::from_ptr(path).to_str().unwrap() }.to_string())
        }
    }
}

/// Whether a port's input profile reads raw joysticks instead of gamepads
fn port_uses_dinput(ui: &ui::Ui, port: usize) -> bool {
    ui.config
        .input
        .input_profiles
        .get(&ui.config.input.input_profile_binding[port])
        .is_some_and(|profile| profile.dinput)
}

/// Whether a connected pad can be given to a port: no port has it yet, and it is a
/// gamepad unless the port reads raw joysticks.
fn is_free_for_port(
    ui: &ui::Ui,
    port: usize,
    joystick_id: sdl3_sys::joystick::SDL_JoystickID,
) -> bool {
    unsafe { sdl3_sys::joystick::SDL_GetJoystickFromID(joystick_id) }.is_null()
        && (port_uses_dinput(ui, port) || unsafe { sdl3_sys::gamepad::SDL_IsGamepad(joystick_id) })
}

fn port_is_empty(controller: &Controllers) -> bool {
    controller.game_controller.is_null() && controller.joystick.is_null()
}

/// Plugs a connected pad into an N64 port
fn open_controller(
    ui: &mut ui::Ui,
    port: usize,
    joystick_id: sdl3_sys::joystick::SDL_JoystickID,
) -> bool {
    if port_uses_dinput(ui, port) {
        let joystick = unsafe { sdl3_sys::joystick::SDL_OpenJoystick(joystick_id) };
        if joystick.is_null() {
            eprintln!("could not connect joystick: {}", u32::from(joystick_id));
            return false;
        }
        ui.input.controllers[port].joystick = joystick;
    } else {
        let gamepad = unsafe { sdl3_sys::gamepad::SDL_OpenGamepad(joystick_id) };
        if gamepad.is_null() {
            eprintln!("could not connect gamepad: {}", u32::from(joystick_id));
            return false;
        }
        ui.input.controllers[port].game_controller = gamepad;
        ui.input.controllers[port].nso_n64 = ui::input_profile::is_nso_n64_controller(joystick_id);
        // Player lights follow the N64 port the pad is plugged into,
        // not the order SDL happened to find the devices in.
        unsafe { sdl3_sys::gamepad::SDL_SetGamepadPlayerIndex(gamepad, port as i32) };
    }
    ui.input.controllers[port].guid =
        unsafe { sdl3_sys::joystick::SDL_GetJoystickGUIDForID(joystick_id) };
    println!("Player {} uses {}", port + 1, joystick_name(joystick_id));
    true
}

/// A pad was switched on or plugged in while a game is running
fn joystick_connected(ui: &mut ui::Ui, joystick_id: sdl3_sys::joystick::SDL_JoystickID) {
    // SDL also announces the pads that init() has already opened
    if !unsafe { sdl3_sys::joystick::SDL_GetJoystickFromID(joystick_id) }.is_null()
        || !list_joysticks().contains(&joystick_id)
    {
        return;
    }

    // The system's copy of an NSO N64 controller can show up before the real one.
    // A port that took the copy lets go of it, so that it gets the real one below.
    if ui::input_profile::is_nso_n64_controller(joystick_id) {
        for controller in ui.input.controllers.iter_mut() {
            if !controller.game_controller.is_null()
                && ui::input_profile::is_nso_n64_system_duplicate(unsafe {
                    sdl3_sys::gamepad::SDL_GetGamepadID(controller.game_controller)
                })
            {
                unsafe { sdl3_sys::gamepad::SDL_CloseGamepad(controller.game_controller) };
                controller.game_controller = std::ptr::null_mut();
            }
        }
    }

    // It goes back to the port it was in before, or else to the first enabled port
    // that has no pad.
    let guid = unsafe { sdl3_sys::joystick::SDL_GetJoystickGUIDForID(joystick_id) };
    let open_ports: Vec<usize> = (0..4)
        .filter(|&port| {
            ui.config.input.controller_enabled[port]
                && port_is_empty(&ui.input.controllers[port])
                && is_free_for_port(ui, port, joystick_id)
        })
        .collect();
    let port = open_ports
        .iter()
        .find(|&&port| ui.input.controllers[port].guid == guid)
        .or(open_ports.first());
    if let Some(&port) = port
        && open_controller(ui, port, joystick_id)
    {
        ui::video::onscreen_message(
            &format!("P{}: {} connected", port + 1, joystick_name(joystick_id)),
            ui::video::MESSAGE_LENGTH_MESSAGE_SHORT,
        );
    }
}

fn joystick_disconnected(ui: &mut ui::Ui, joystick_id: sdl3_sys::joystick::SDL_JoystickID) {
    for (port, controller) in ui.input.controllers.iter_mut().enumerate() {
        if !controller.joystick.is_null()
            && controller.joystick
                == unsafe { sdl3_sys::joystick::SDL_GetJoystickFromID(joystick_id) }
        {
            unsafe { sdl3_sys::joystick::SDL_CloseJoystick(controller.joystick) };
            controller.joystick = std::ptr::null_mut();
        } else if !controller.game_controller.is_null()
            && controller.game_controller
                == unsafe { sdl3_sys::gamepad::SDL_GetGamepadFromID(joystick_id) }
        {
            unsafe { sdl3_sys::gamepad::SDL_CloseGamepad(controller.game_controller) };
            controller.game_controller = std::ptr::null_mut();
            controller.nso_n64 = false;
        } else {
            continue;
        }
        ui::video::onscreen_message(
            &format!("P{}: controller disconnected", port + 1),
            ui::video::MESSAGE_LENGTH_MESSAGE_SHORT,
        );
    }
}

fn handle_joystick_events(ui: &mut ui::Ui) {
    let joystick_event = unsafe { ui::video::get_joystick_event() };
    if joystick_event.joystick_id != 0 {
        let joystick_id = sdl3_sys::joystick::SDL_JoystickID(joystick_event.joystick_id);
        if joystick_event.connected {
            joystick_connected(ui, joystick_id);
        } else {
            joystick_disconnected(ui, joystick_id);
        }
    }
}

pub fn push_user_event(code: i32) {
    let mut event: sdl3_sys::events::SDL_Event = Default::default();
    event.user.r#type = u32::from(sdl3_sys::events::SDL_EVENT_USER);
    event.user.code = code;
    unsafe { sdl3_sys::events::SDL_PushEvent(&mut event) };
}

fn handle_hotkeys(keys: u32, last_key_state: u32) {
    let pressed = |button: usize| keys & (1 << button) != 0 && last_key_state & (1 << button) == 0;
    if pressed(ui::input_profile::L_TRIG) {
        push_user_event(USER_EVENT_SAVE_STATE);
    }
    if pressed(ui::input_profile::R_TRIG) {
        push_user_event(USER_EVENT_LOAD_STATE);
    }
    if pressed(ui::input_profile::START_BUTTON) {
        push_user_event(USER_EVENT_OPEN_MENU);
    }
    if pressed(ui::input_profile::Z_TRIG) {
        push_user_event(USER_EVENT_FAST_FORWARD);
    }
    if pressed(ui::input_profile::L_CBUTTON) {
        push_user_event(USER_EVENT_LOAD_REWIND);
    }
}

pub fn get(ui: &mut ui::Ui, channel: usize) -> InputData {
    handle_joystick_events(ui);

    let mut profile_name = ui.config.input.input_profile_binding[channel].as_str();
    // The default profile cannot work with an NSO N64 controller (see get_nso_n64_profile),
    // so a port left on "default" gets the built-in N64 layout while one is attached.
    if ui.input.controllers[channel].nso_n64 && profile_name == "default" {
        profile_name = ui::input_profile::NSO_N64_PROFILE;
    }
    let Some(profile) = ui.config.input.input_profiles.get(profile_name) else {
        eprintln!("Invalid profile name: {profile_name}");
        return InputData {
            data: 0,
            pak_change_pressed: false,
        };
    };
    let controller = ui.input.controllers[channel].game_controller;
    let joystick = ui.input.controllers[channel].joystick;

    let alt_pressed = unsafe {
        // ignore key presses if ALT is pressed
        *ui.input
            .keyboard_state
            .offset(i32::from(sdl3_sys::scancode::SDL_SCANCODE_LALT) as isize)
            || *ui
                .input
                .keyboard_state
                .offset(i32::from(sdl3_sys::scancode::SDL_SCANCODE_RALT) as isize)
    };

    let mut keys = set_buttons(
        profile,
        joystick,
        controller,
        ui.input.keyboard_state,
        alt_pressed,
    );

    let (mut x, mut y) = set_axis(profile, joystick, controller, ui.input.keyboard_state);
    bound_axis(&mut x, &mut y);

    keys |= (x.round() as i8 as u8 as u32) << X_AXIS_SHIFT;
    keys |= (y.round() as i8 as u8 as u32) << Y_AXIS_SHIFT;

    let last_key_state = ui.input.controllers[channel].last_key_state;
    ui.input.controllers[channel].last_key_state = keys;

    if hotkey_pressed(profile, joystick, controller) {
        handle_hotkeys(keys, last_key_state);
        InputData {
            data: 0,
            pak_change_pressed: keys & (1 << ui::input_profile::B_BUTTON) != 0,
        }
    } else {
        let mut pak_change_pressed = false;
        for input in profile.inputs[ui::input_profile::HOTKEY].iter() {
            if let Some(ui::config::InputItem::Key(key)) = input
                && unsafe { *ui.input.keyboard_state.offset(key.id as isize) }
            {
                pak_change_pressed = true;
            }
        }
        InputData {
            data: keys,
            pak_change_pressed,
        }
    }
}

pub fn assign_controller(config: &mut ui::config::Config, controller: i32, port: usize) {
    let joysticks = get_joysticks();
    if controller < 0 {
        config.input.controller_assignment[port - 1] = None;
    } else if controller < joysticks.len() as i32 {
        let path =
            unsafe { sdl3_sys::joystick::SDL_GetJoystickPathForID(joysticks[controller as usize]) };
        if !path.is_null() {
            config.input.controller_assignment[port - 1] =
                Some(unsafe { std::ffi::CStr::from_ptr(path).to_str().unwrap().to_string() });
        } else {
            eprintln!("Invalid controller path for controller {controller}");
        }
    } else {
        eprintln!("Invalid controller number")
    }
}

pub fn bind_input_profile(config: &mut ui::config::Config, profile: String, port: usize) {
    if config.input.input_profiles.contains_key(&profile) {
        config.input.input_profile_binding[port - 1] = profile;
    } else {
        eprintln!("Invalid profile name")
    }
}

pub fn clear_bindings(config: &mut ui::config::Config) {
    for i in 0..4 {
        config.input.controller_assignment[i] = None;
        config.input.input_profile_binding[i] = "default".to_string();
    }
}

/// The hidden system copy of an NSO N64 controller is found first and takes player slot
/// one, which leaves the real pad showing two lights. Give the real pads the first slots.
/// Once a game starts, the lights follow the port the pad is assigned to instead.
fn claim_player_slots(joysticks: &[sdl3_sys::joystick::SDL_JoystickID]) {
    for (slot, joystick_id) in joysticks
        .iter()
        .filter(|joystick| ui::input_profile::is_nso_n64_controller(**joystick))
        .enumerate()
    {
        let slot = slot as i32;
        let in_use = !unsafe { sdl3_sys::joystick::SDL_GetJoystickFromID(*joystick_id) }.is_null();
        if !in_use
            && unsafe { sdl3_sys::joystick::SDL_GetJoystickPlayerIndexForID(*joystick_id) } != slot
        {
            let joystick = unsafe { sdl3_sys::joystick::SDL_OpenJoystick(*joystick_id) };
            if !joystick.is_null() {
                unsafe {
                    sdl3_sys::joystick::SDL_SetJoystickPlayerIndex(joystick, slot);
                    sdl3_sys::joystick::SDL_CloseJoystick(joystick);
                }
            }
        }
    }
}

/// The connected pads, without the hidden system copies of NSO N64 controllers
fn list_joysticks() -> Vec<sdl3_sys::joystick::SDL_JoystickID> {
    let mut num_joysticks = 0;
    let sdl_joysticks = unsafe { sdl3_sys::joystick::SDL_GetJoysticks(&mut num_joysticks) };
    if !sdl_joysticks.is_null() {
        let mut parts =
            unsafe { std::slice::from_raw_parts(sdl_joysticks, num_joysticks as usize) }.to_vec();
        unsafe { sdl3_sys::stdinc::SDL_free(sdl_joysticks as *mut std::ffi::c_void) };
        // macOS also offers the NSO N64 controller through its own GameController
        // framework. SDL does not fold that copy into the real one, and it has no C
        // buttons, so it is left out whenever the real one is there.
        if parts
            .iter()
            .any(|joystick| ui::input_profile::is_nso_n64_controller(*joystick))
        {
            parts.retain(|joystick| !ui::input_profile::is_nso_n64_system_duplicate(*joystick));
        }
        parts
    } else {
        eprintln!("Could not get joysticks");
        vec![]
    }
}

pub fn get_joysticks() -> Vec<sdl3_sys::joystick::SDL_JoystickID> {
    unsafe { sdl3_sys::events::SDL_PumpEvents() };
    let joysticks = list_joysticks();
    claim_player_slots(&joysticks);
    joysticks
}

pub fn init(ui: &mut ui::Ui) {
    ui::sdl_init(sdl3_sys::init::SDL_INIT_GAMEPAD);

    ui.input.keyboard_state =
        unsafe { sdl3_sys::keyboard::SDL_GetKeyboardState(std::ptr::null_mut()) };
    if ui.input.keyboard_state.is_null() {
        panic!("Could not get keyboard state");
    }

    unsafe { sdl3_sys::events::SDL_PumpEvents() };
    let joysticks = list_joysticks();

    // Ports with a chosen controller get it first...
    for port in 0..4 {
        if ui.config.input.controller_enabled[port]
            && let Some(assignment) = ui.config.input.controller_assignment[port].clone()
            && let Some(joystick_id) = joysticks.iter().copied().find(|joystick| {
                joystick_path(*joystick).as_ref() == Some(&assignment)
                    && is_free_for_port(ui, port, *joystick)
            })
        {
            open_controller(ui, port, joystick_id);
        }
    }

    // ...then every enabled port still without one takes the next free controller.
    // This also covers a chosen controller that cannot be found, which is the normal
    // case for Bluetooth pads: they come back under a new path after each reconnect.
    for port in 0..4 {
        if ui.config.input.controller_enabled[port]
            && port_is_empty(&ui.input.controllers[port])
            && let Some(joystick_id) = joysticks
                .iter()
                .copied()
                .find(|joystick| is_free_for_port(ui, port, *joystick))
        {
            open_controller(ui, port, joystick_id);
        }
    }
}

/// Switches off the wireless Nintendo Switch controllers (the NSO N64 controller is one),
/// the way a console does when it shuts down. Left alone they stay connected, lights on,
/// until they give up by themselves.
pub fn power_off_controllers() {
    // The "set HCI state" subcommand with "disconnect". SDL's Switch driver sends a short
    // effect to the controller as a subcommand, unchanged.
    const POWER_OFF: [u8; 2] = [0x06, 0x00];

    ui::sdl_init(sdl3_sys::init::SDL_INIT_GAMEPAD);
    unsafe { sdl3_sys::events::SDL_PumpEvents() };
    for joystick_id in list_joysticks() {
        if !ui::input_profile::is_switch_controller(joystick_id) {
            continue;
        }
        let gamepad = unsafe { sdl3_sys::gamepad::SDL_OpenGamepad(joystick_id) };
        if gamepad.is_null() {
            continue;
        }
        println!("Switching off {}", joystick_name(joystick_id));
        unsafe {
            // over USB the same command would only put the controller to sleep
            if sdl3_sys::gamepad::SDL_GetGamepadConnectionState(gamepad)
                == sdl3_sys::joystick::SDL_JOYSTICK_CONNECTION_WIRELESS
            {
                sdl3_sys::gamepad::SDL_SendGamepadEffect(
                    gamepad,
                    POWER_OFF.as_ptr() as *const std::ffi::c_void,
                    POWER_OFF.len() as i32,
                );
            }
            sdl3_sys::gamepad::SDL_CloseGamepad(gamepad);
        }
    }
}

pub fn close(ui: &mut ui::Ui) {
    // A game can end while a pad is shaking. SDL's Switch driver holds back a "stop" that
    // comes within 30 ms of its last rumble packet and sends it on a later update, so
    // without that update the pad is closed still shaking, and it does not stop by itself.
    if ui.input.controllers.iter().any(|c| !port_is_empty(c)) {
        for channel in 0..ui.input.controllers.len() {
            set_rumble(ui, channel, 0);
        }
        unsafe {
            sdl3_sys::timer::SDL_Delay(40);
            sdl3_sys::joystick::SDL_UpdateJoysticks();
        }
    }

    for controller in ui.input.controllers.iter_mut() {
        if !controller.joystick.is_null() {
            unsafe { sdl3_sys::joystick::SDL_CloseJoystick(controller.joystick) }
            controller.joystick = std::ptr::null_mut();
        }
        if !controller.game_controller.is_null() {
            unsafe { sdl3_sys::gamepad::SDL_CloseGamepad(controller.game_controller) }
            controller.game_controller = std::ptr::null_mut();
        }
    }
}
