//! Prints what SDL reports for a connected gamepad: its name, IDs, mapping string, and
//! every button press and large axis movement. Used to check input profiles against a
//! real controller. With "rumble" after the seconds, every button press also shakes the
//! pad for half a second, to check that SDL can drive its motor.
//!
//!     cargo run --release --example pad_probe -- [seconds] [rumble]

use std::ffi::CStr;

// SDL needs the clang runtime that the emulator's build script links in
use viben64 as _;

fn c_str(ptr: *const std::ffi::c_char) -> String {
    if ptr.is_null() {
        "(null)".to_string()
    } else {
        unsafe { CStr::from_ptr(ptr) }.to_string_lossy().to_string()
    }
}

fn main() {
    let seconds: u64 = std::env::args()
        .nth(1)
        .and_then(|arg| arg.parse().ok())
        .unwrap_or(45);
    let rumble = std::env::args().nth(2).is_some_and(|arg| arg == "rumble");

    unsafe {
        sdl3_sys::everything::SDL_SetHint(
            sdl3_sys::everything::SDL_HINT_JOYSTICK_ALLOW_BACKGROUND_EVENTS,
            c"1".as_ptr(),
        );
        if !sdl3_sys::init::SDL_Init(sdl3_sys::init::SDL_INIT_GAMEPAD) {
            println!(
                "SDL_Init failed: {}",
                c_str(sdl3_sys::error::SDL_GetError())
            );
            return;
        }
    }

    let mut open: Vec<*mut sdl3_sys::gamepad::SDL_Gamepad> = Vec::new();
    let mut axis_state = [[0i8; 8]; 8];
    let start = std::time::Instant::now();
    println!("listening for {seconds} s");

    while start.elapsed().as_secs() < seconds {
        let mut event = sdl3_sys::events::SDL_Event::default();
        while unsafe { sdl3_sys::events::SDL_PollEvent(&mut event) } {
            let event_type = sdl3_sys::events::SDL_EventType(unsafe { event.r#type });
            match event_type {
                sdl3_sys::events::SDL_EVENT_GAMEPAD_ADDED => {
                    let id = unsafe { event.gdevice.which };
                    let pad = unsafe { sdl3_sys::gamepad::SDL_OpenGamepad(id) };
                    if pad.is_null() {
                        continue;
                    }
                    open.push(pad);
                    unsafe {
                        println!(
                            "PAD {} name={:?} vendor={:04x} product={:04x}",
                            open.len() - 1,
                            c_str(sdl3_sys::gamepad::SDL_GetGamepadName(pad)),
                            sdl3_sys::gamepad::SDL_GetGamepadVendor(pad),
                            sdl3_sys::gamepad::SDL_GetGamepadProduct(pad),
                        );
                        let mapping = sdl3_sys::gamepad::SDL_GetGamepadMapping(pad);
                        println!("MAPPING {}", c_str(mapping));
                        sdl3_sys::stdinc::SDL_free(mapping as *mut std::ffi::c_void);
                    }
                }
                sdl3_sys::events::SDL_EVENT_GAMEPAD_BUTTON_DOWN => {
                    let button = unsafe { event.gbutton.button };
                    let name = unsafe {
                        sdl3_sys::gamepad::SDL_GetGamepadStringForButton(
                            sdl3_sys::gamepad::SDL_GamepadButton(button as i32),
                        )
                    };
                    println!(
                        "{:6.2} BUTTON {} ({})",
                        start.elapsed().as_secs_f32(),
                        c_str(name),
                        button
                    );
                    if rumble {
                        let pad =
                            unsafe { sdl3_sys::gamepad::SDL_GetGamepadFromID(event.gbutton.which) };
                        let sent = unsafe {
                            sdl3_sys::gamepad::SDL_RumbleGamepad(pad, u16::MAX, u16::MAX, 500)
                        };
                        println!(
                            "       RUMBLE {}",
                            if sent {
                                "sent".to_string()
                            } else {
                                c_str(sdl3_sys::error::SDL_GetError())
                            }
                        );
                    }
                }
                sdl3_sys::events::SDL_EVENT_GAMEPAD_AXIS_MOTION => {
                    let axis = unsafe { event.gaxis.axis } as usize;
                    let value = unsafe { event.gaxis.value };
                    let which = unsafe { event.gaxis.which }.0 as usize % 8;
                    // only report an axis when it crosses half way, once per direction
                    let state: i8 = if value > i16::MAX / 2 {
                        1
                    } else if value < i16::MIN / 2 {
                        -1
                    } else {
                        0
                    };
                    if axis < 8 && axis_state[which][axis] != state {
                        axis_state[which][axis] = state;
                        if state != 0 {
                            let name = unsafe {
                                sdl3_sys::gamepad::SDL_GetGamepadStringForAxis(
                                    sdl3_sys::gamepad::SDL_GamepadAxis(axis as i32),
                                )
                            };
                            println!(
                                "{:6.2} AXIS   {} {}",
                                start.elapsed().as_secs_f32(),
                                c_str(name),
                                if state > 0 { "+" } else { "-" }
                            );
                        }
                    }
                }
                _ => {}
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }

    // let a pending rumble stop reach the pads before they are closed
    for pad in &open {
        unsafe { sdl3_sys::gamepad::SDL_RumbleGamepad(*pad, 0, 0, 0) };
    }
    unsafe {
        sdl3_sys::timer::SDL_Delay(40);
        sdl3_sys::joystick::SDL_UpdateJoysticks();
    }
    for pad in open {
        unsafe { sdl3_sys::gamepad::SDL_CloseGamepad(pad) };
    }
    println!("done");
}
