#pragma once

#ifdef __cplusplus
#include <cstdint>
#include <stddef.h>

extern "C" {
#endif

typedef struct {
  uint8_t *RDRAM;
  uint8_t *DMEM;
  uint32_t RDRAM_SIZE;
  uint32_t *DPC_CURRENT_REG;
  uint32_t *DPC_START_REG;
  uint32_t *DPC_END_REG;
  uint32_t *DPC_STATUS_REG;
  bool PAL;
  bool widescreen;
  bool fullscreen;
  bool vsync;
  bool integer_scaling;
  uint32_t upscale;
  bool ssaa;
  uint32_t shader; // index into the display shaders, see rdp_shader_count
} GFX_INFO;

typedef struct {
  bool emu_running;
  bool save_state;
  bool load_state;
  bool load_rewind;
  bool reset_game;
  bool enable_speedlimiter;
  bool lower_volume;
  bool raise_volume;
  bool decrease_input_delay;
  bool increase_input_delay;
  bool paused;
  bool frame_advance;
  bool open_menu;
  uint32_t save_state_slot;
} CALL_BACK;

typedef struct {
  uint32_t joystick_id;
  bool connected;
} JoystickEvent;

typedef enum {
  MESSAGE_VERY_SHORT = 500,
  MESSAGE_SHORT = 3000,
  MESSAGE_LONG = 6000,
} MESSAGE_LENGTH;

void rdp_init(void *_window, GFX_INFO _gfx_info, const void *font,
              size_t font_size, uint32_t save_state_slot);
void rdp_close();
void rdp_set_vi_register(uint32_t reg, uint32_t value);
void rdp_update_screen();
void rdp_render_frame();
CALL_BACK rdp_check_callback();
uint64_t rdp_process_commands();
void rdp_idle();
void rdp_onscreen_message(const char *message, MESSAGE_LENGTH milliseconds);
void rdp_check_framebuffers(uint32_t address, uint32_t length);
size_t rdp_state_size();
void rdp_save_state(uint8_t *state);
void rdp_load_state(GFX_INFO _gfx_info, const uint8_t *state);
void rdp_set_fps(uint32_t fps, uint32_t vis);

JoystickEvent get_joystick_event();

// The display shaders: how the finished picture is drawn to the window
uint32_t rdp_shader_count();
const char *rdp_shader_id(uint32_t index);   // what the config file stores
const char *rdp_shader_name(uint32_t index); // what the user sees
void rdp_set_shader(uint32_t index);

// The in-game menu (see src/ui/menu.rs, which owns what it shows and does)
void rdp_menu_show(const char *title, const char *const *items, uint32_t count,
                   uint32_t selected, const char *hint);
void rdp_menu_hide();
void rdp_toggle_fullscreen();
bool rdp_is_fullscreen();
void rdp_set_save_state_slot(uint32_t slot);

void achievement_challenge_indicator_add(const char *achievement_title);
void achievement_challenge_indicator_remove(const char *achievement_title);
void achievement_progress_add(const char *achievement_title,
                              const char *progress);
void achievement_progress_remove();
void leaderboard_tracker_add(uint32_t id, const char *title,
                             const char *display);
void leaderboard_tracker_remove(uint32_t id);

#ifdef __cplusplus
}
#endif
