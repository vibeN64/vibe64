#include "interface.hpp"
#include "../retroachievements/retroachievements.h"
#include "rdp_device.hpp"
#include "spirv.hpp"
#include "spirv_crt.hpp"
#include "wsi.hpp"
#include "wsi_platform.hpp"
#include <SDL3/SDL_vulkan.h>
#include <SDL3_ttf/SDL_ttf.h>
#include <algorithm>
#include <cmath>
#include <map>

#ifdef _WIN32
extern "C" {
__declspec(dllexport) unsigned long NvOptimusEnablement = 0x00000001;
__declspec(dllexport) int AmdPowerXpressRequestHighPerformance = 1;
}
#endif

using namespace Vulkan;

#define DP_STATUS_XBUS_DMA 0x01
#define DP_STATUS_FREEZE 0x02
#define DP_STATUS_FLUSH 0x04
#define DP_STATUS_START_GCLK 0x008
#define DP_STATUS_TMEM_BUSY 0x010
#define DP_STATUS_PIPE_BUSY 0x020
#define DP_STATUS_CMD_BUSY 0x040
#define DP_STATUS_CBUF_READY 0x080
#define DP_STATUS_DMA_BUSY 0x100
#define DP_STATUS_END_VALID 0x200
#define DP_STATUS_START_VALID 0x400

enum dpc_registers {
  DPC_START_REG,
  DPC_END_REG,
  DPC_CURRENT_REG,
  DPC_STATUS_REG,
  DPC_CLOCK_REG,
  DPC_BUFBUSY_REG,
  DPC_PIPEBUSY_REG,
  DPC_TMEM_REG,
  DPC_REGS_COUNT
};

enum vi_registers {
  VI_STATUS_REG,
  VI_ORIGIN_REG,
  VI_WIDTH_REG,
  VI_V_INTR_REG,
  VI_CURRENT_REG,
  VI_BURST_REG,
  VI_V_SYNC_REG,
  VI_H_SYNC_REG,
  VI_LEAP_REG,
  VI_H_START_REG,
  VI_V_START_REG,
  VI_V_BURST_REG,
  VI_X_SCALE_REG,
  VI_Y_SCALE_REG,
  VI_REGS_COUNT
};

enum user_event_codes {
  USER_EVENT_SAVE_STATE = 1,
  USER_EVENT_LOAD_STATE = 2,
  USER_EVENT_EXIT_GAME = 3,
  USER_EVENT_FAST_FORWARD = 4,
  USER_EVENT_LOAD_REWIND = 5,
  USER_EVENT_OPEN_MENU = 6,
};

typedef struct {
  uint32_t depthbuffer_address;
  uint32_t framebuffer_address;
  uint32_t framebuffer_y_offset;
  uint32_t texture_address;
  uint32_t framebuffer_pixel_size;
  uint32_t framebuffer_width;
  uint32_t texture_pixel_size;
  uint32_t texture_width;
  uint32_t framebuffer_height;
  bool depth_buffer_enabled;
} FrameBufferInfo;

typedef struct {
  uint32_t cmd_data[0x00040000 >> 2];
  int cmd_cur;
  int cmd_ptr;
  uint32_t region;
  FrameBufferInfo frame_buffer_info;
} RDP_DEVICE;

static void *g_tmem = nullptr;
static void *g_hidden_rdram = nullptr;

typedef struct {
  uint32_t fps;
  uint32_t vis;
} FPS_DATA;

typedef struct {
  std::string message;
  uint64_t milliseconds;
  Vulkan::ImageHandle image;
} Message;

static SDL_Window *window;
static RDP::CommandProcessor *processor;
static SDL_WSIPlatform *wsi_platform;
static WSI *wsi;

static RDP_DEVICE rdp_device;
static bool crop_letterbox;
static CALL_BACK callback;
static GFX_INFO gfx_info;
static const uint32_t *fragment_spirv;
static size_t fragment_size;

static std::vector<bool> rdram_dirty;
static uint64_t sync_signal;

static TTF_Font *message_font;
static std::queue<Message> messages;

static float message_font_size = 25.0;
static float achievement_challenge_indicator_font_size = 12.0;

static TTF_Font *achievement_challenge_indicator_font;
static std::vector<const char *> achievement_challenge_indicators;
static Vulkan::ImageHandle achievement_challenge_indicator_image;
static Vulkan::ImageHandle achievement_progress_indicator_image;
static std::map<uint32_t, std::string> leaderboard_trackers;
static bool display_challenge_indicator;

static bool display_fps;
static Vulkan::ImageHandle fps_image;

static std::queue<JoystickEvent> joystick_events;

typedef struct {
  std::string title;
  std::vector<std::string> items;
  uint32_t selected;
  std::string hint;
} Menu;

static bool menu_visible;
static Menu menu;
static Vulkan::ImageHandle menu_image;
static TTF_Font *menu_font;

typedef struct {
  float SourceSize[4];
  float OutputSize[4];
} Push;

static const unsigned cmd_len_lut[64] = {
    1, 1, 1, 1, 1, 1, 1, 1, 4, 6, 12, 14, 12, 14, 20, 22, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,  1,  1,  1,  2,  2,  1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,  1,  1,  1,  1,  1,  1, 1, 1, 1,
};

static void add_joystick_event(void *userdata) {
  JoystickEvent *joystick_event = (JoystickEvent *)userdata;
  joystick_events.push(*joystick_event);
  delete joystick_event;
}

bool sdl_event_filter(void *userdata, SDL_Event *event) {
  // SDL_EVENT_QUIT is Cmd+Q, or a request to terminate: end the game the same way as
  // closing its window, so that saves are written and controllers are released.
  if (event->type == SDL_EVENT_WINDOW_CLOSE_REQUESTED ||
      event->type == SDL_EVENT_QUIT) {
    callback.paused = false;
    callback.emu_running = false;
  } else if (event->type == SDL_EVENT_WINDOW_PIXEL_SIZE_CHANGED &&
             callback.emu_running) {
    wsi_platform->do_resize();

    if (message_font) {
      TTF_SetFontSize(message_font,
                      message_font_size * SDL_GetWindowDisplayScale(window));
    }
    menu_image = Vulkan::ImageHandle(); // redrawn to suit the new size
    if (achievement_challenge_indicator_font) {
      TTF_SetFontSize(achievement_challenge_indicator_font,
                      achievement_challenge_indicator_font_size *
                          SDL_GetWindowDisplayScale(window));
    }
  } else if (event->type == SDL_EVENT_WINDOW_MINIMIZED) {
    callback.paused = true;
  } else if (event->type == SDL_EVENT_WINDOW_RESTORED) {
    callback.paused = false;
  } else if (event->type == SDL_EVENT_KEY_DOWN && !event->key.repeat) {
    SDL_Event user_event;
    switch (event->key.scancode) {
    case SDL_SCANCODE_RETURN:
      if (event->key.mod & SDL_KMOD_ALT) {
        rdp_toggle_fullscreen();
      }
      break;
    case SDL_SCANCODE_F:
      if (event->key.mod & SDL_KMOD_ALT) {
        SDL_zero(user_event);
        user_event.type = SDL_EVENT_USER;
        user_event.user.code = USER_EVENT_FAST_FORWARD;
        SDL_PushEvent(&user_event);
      }
      break;
    case SDL_SCANCODE_P:
      if (event->key.mod & SDL_KMOD_ALT) {
        callback.paused = !callback.paused;
      }
      break;
    case SDL_SCANCODE_AC_BACK:
    case SDL_SCANCODE_ESCAPE:
      // Opens the in-game menu, or closes it. Where the menu cannot be used,
      // this still leaves a fullscreen game (see update_screen in vi.rs).
      callback.open_menu = true;
      break;
    case SDL_SCANCODE_F1:
      display_fps = !display_fps;
      break;
    case SDL_SCANCODE_F4:
      crop_letterbox = !crop_letterbox;
      break;
    case SDL_SCANCODE_F5:
      SDL_zero(user_event);
      user_event.type = SDL_EVENT_USER;
      user_event.user.code = USER_EVENT_SAVE_STATE;
      SDL_PushEvent(&user_event);
      break;
    case SDL_SCANCODE_F6:
      SDL_zero(user_event);
      user_event.type = SDL_EVENT_USER;
      user_event.user.code = USER_EVENT_LOAD_REWIND;
      SDL_PushEvent(&user_event);
      break;
    case SDL_SCANCODE_F7:
      SDL_zero(user_event);
      user_event.type = SDL_EVENT_USER;
      user_event.user.code = USER_EVENT_LOAD_STATE;
      SDL_PushEvent(&user_event);
      break;
    case SDL_SCANCODE_F8:
      if (messages.empty())
        SDL_RunOnMainThread(ra_display_inprogress_achievements, nullptr, false);
      break;
    case SDL_SCANCODE_F9:
      display_challenge_indicator = !display_challenge_indicator;
      rdp_onscreen_message(
          std::format("Challenge indicators: {}",
                      display_challenge_indicator ? "ON" : "OFF")
              .c_str(),
          MESSAGE_VERY_SHORT);
      break;
    case SDL_SCANCODE_F12:
      callback.reset_game = true;
      break;
    case SDL_SCANCODE_LEFTBRACKET:
      if (event->key.mod & SDL_KMOD_ALT) {
        callback.decrease_input_delay = true;
      } else {
        callback.lower_volume = true;
      }
      break;
    case SDL_SCANCODE_RIGHTBRACKET:
      if (event->key.mod & SDL_KMOD_ALT) {
        callback.increase_input_delay = true;
      } else {
        callback.raise_volume = true;
      }
      break;
    case SDL_SCANCODE_SLASH:
      callback.frame_advance = true;
      break;
    case SDL_SCANCODE_0:
    case SDL_SCANCODE_1:
    case SDL_SCANCODE_2:
    case SDL_SCANCODE_3:
    case SDL_SCANCODE_4:
    case SDL_SCANCODE_5:
    case SDL_SCANCODE_6:
    case SDL_SCANCODE_7:
    case SDL_SCANCODE_8:
    case SDL_SCANCODE_9:
      if (event->key.mod & SDL_KMOD_ALT) {
        if (event->key.scancode == SDL_SCANCODE_0)
          callback.save_state_slot = 0;
        else
          callback.save_state_slot = event->key.scancode - SDL_SCANCODE_1 + 1;
      }
      break;
    default:
      break;
    }
  } else if (event->type == SDL_EVENT_USER) {
    switch (event->user.code) {
    case USER_EVENT_SAVE_STATE:
      callback.save_state = true;
      break;
    case USER_EVENT_LOAD_REWIND:
      callback.load_rewind = true;
      break;
    case USER_EVENT_LOAD_STATE:
      callback.load_state = true;
      break;
    case USER_EVENT_EXIT_GAME:
      callback.emu_running = false;
      break;
    case USER_EVENT_FAST_FORWARD:
      callback.enable_speedlimiter = !callback.enable_speedlimiter;
      break;
    case USER_EVENT_OPEN_MENU:
      callback.open_menu = true;
      break;
    default:
      break;
    }
  } else if (event->type == SDL_EVENT_JOYSTICK_ADDED) {
    JoystickEvent *joystick_event = new JoystickEvent;
    joystick_event->joystick_id = event->jdevice.which;
    joystick_event->connected = true;
    SDL_RunOnMainThread(add_joystick_event, joystick_event, false);
  } else if (event->type == SDL_EVENT_JOYSTICK_REMOVED) {
    JoystickEvent *joystick_event = new JoystickEvent;
    joystick_event->joystick_id = event->jdevice.which;
    joystick_event->connected = false;
    SDL_RunOnMainThread(add_joystick_event, joystick_event, false);
  } else if (event->type == SDL_EVENT_WILL_ENTER_BACKGROUND) {
    wsi->end_frame();
    wsi->deinit_surface_and_swapchain();
  } else if (event->type == SDL_EVENT_RENDER_DEVICE_RESET) {
    wsi->init_surface_swapchain();
    wsi->begin_frame();
  }

  return 0;
}

JoystickEvent get_joystick_event() {
  if (joystick_events.empty())
    return JoystickEvent{0, false};
  JoystickEvent joystick_event = joystick_events.front();
  joystick_events.pop();
  return joystick_event;
}

void rdp_idle() {
  processor->idle();
  sync_signal = 0;
  rdram_dirty.assign(gfx_info.RDRAM_SIZE >> 3, false);
}

static void rdp_new_processor() {
  RDP::CommandProcessorFlags flags =
      RDP::COMMAND_PROCESSOR_FLAG_HOST_VISIBLE_HIDDEN_RDRAM_BIT |
      RDP::COMMAND_PROCESSOR_FLAG_HOST_VISIBLE_TMEM_BIT;

  if (gfx_info.upscale == 2) {
    flags |= RDP::COMMAND_PROCESSOR_FLAG_SUPER_SAMPLED_DITHER_BIT;
    flags |= RDP::COMMAND_PROCESSOR_FLAG_UPSCALING_2X_BIT;
  } else if (gfx_info.upscale == 4) {
    flags |= RDP::COMMAND_PROCESSOR_FLAG_SUPER_SAMPLED_DITHER_BIT;
    flags |= RDP::COMMAND_PROCESSOR_FLAG_UPSCALING_4X_BIT;
  } else if (gfx_info.upscale == 8) {
    flags |= RDP::COMMAND_PROCESSOR_FLAG_SUPER_SAMPLED_DITHER_BIT;
    flags |= RDP::COMMAND_PROCESSOR_FLAG_UPSCALING_8X_BIT;
  } else {
    gfx_info.upscale = 1;
  }

  processor = new RDP::CommandProcessor(wsi->get_device(), gfx_info.RDRAM, 0,
                                        gfx_info.RDRAM_SIZE,
                                        gfx_info.RDRAM_SIZE / 2, flags);
  g_tmem = processor->get_tmem();
  if (!g_tmem) {
    LOGE("Failed to get tmem\n");
  }
  g_hidden_rdram = processor->begin_read_hidden_rdram();
  if (!g_hidden_rdram) {
    LOGE("Failed to get hidden_rdram\n");
  }

  sync_signal = 0;
  rdram_dirty.assign(gfx_info.RDRAM_SIZE >> 3, false);
}

static ImageHandle create_message_image(Vulkan::Device &device, int width,
                                        TTF_Font *font, const char *message) {
  if (strstr(message, "\n"))
    width = 0;

  SDL_Color fg = {255, 255, 255, 255};
  SDL_Color bg = {0, 0, 0, 0};
  SDL_Surface *surface =
      TTF_RenderText_LCD_Wrapped(font, message, 0, fg, bg, width);
  ImageCreateInfo info = ImageCreateInfo::immutable_2d_image(
      surface->w, surface->h, VK_FORMAT_B8G8R8A8_UNORM, false);
  ImageInitialData initial_data = {};
  initial_data.data = surface->pixels;
  initial_data.row_length = surface->pitch / 4;
  initial_data.image_height = surface->h;

  ImageHandle handle = device.create_image(info, &initial_data);
  SDL_DestroySurface(surface);
  return handle;
}

// The whole menu is drawn into one image: a dark panel with a title, one row per item
// (the selected one highlighted) and a line of help underneath.
static ImageHandle create_menu_image(Vulkan::Device &device) {
  // The text follows the size of the window, within limits, so that the menu is
  // neither cramped in a small window nor tiny in fullscreen.
  const float scale = SDL_GetWindowDisplayScale(window);
  int window_width, window_height;
  SDL_GetWindowSizeInPixels(window, &window_width, &window_height);
  const float text_size =
      std::clamp(window_height / 30.0f, 13.0f * scale, 22.0f * scale);
  const int pad_x = int(text_size * 1.1f);
  const int pad_y = int(text_size * 0.4f);
  const int margin = int(text_size * 0.7f);
  const int border = std::max(1, int(2 * scale));
  const SDL_Color white = {255, 255, 255, 255};
  const SDL_Color muted = {168, 168, 186, 255};

  TTF_SetFontSize(menu_font, text_size * 0.62f);
  SDL_Surface *hint =
      TTF_RenderText_Blended(menu_font, menu.hint.c_str(), 0, muted);
  TTF_SetFontSize(menu_font, text_size);
  SDL_Surface *title =
      TTF_RenderText_Blended(menu_font, menu.title.c_str(), 0, muted);
  std::vector<SDL_Surface *> rows;
  for (const std::string &item : menu.items) {
    rows.push_back(TTF_RenderText_Blended(menu_font, item.c_str(), 0, white));
  }

  int text_width = std::max(title ? title->w : 0, hint ? hint->w : 0);
  for (SDL_Surface *row : rows) {
    if (row)
      text_width = std::max(text_width, row->w);
  }
  const int row_height = TTF_GetFontHeight(menu_font) + 2 * pad_y;
  const int width = text_width + 2 * pad_x + 2 * margin;
  const int height = margin + (title ? title->h : 0) + margin +
                     row_height * int(rows.size()) + margin +
                     (hint ? hint->h : 0) + margin;

  SDL_Surface *surface =
      SDL_CreateSurface(width, height, SDL_PIXELFORMAT_ARGB8888);
  const SDL_PixelFormatDetails *format =
      SDL_GetPixelFormatDetails(surface->format);
  SDL_FillSurfaceRect(surface, nullptr,
                      SDL_MapRGBA(format, nullptr, 96, 72, 160, 255));
  SDL_Rect panel = {border, border, width - 2 * border, height - 2 * border};
  SDL_FillSurfaceRect(surface, &panel,
                      SDL_MapRGBA(format, nullptr, 22, 22, 30, 255));

  int y = margin;
  if (title) {
    SDL_Rect dst = {margin + pad_x, y, title->w, title->h};
    SDL_BlitSurface(title, nullptr, surface, &dst);
    y += title->h;
    SDL_DestroySurface(title);
  }
  y += margin;
  for (size_t i = 0; i < rows.size(); ++i) {
    if (i == menu.selected) {
      SDL_Rect highlight = {margin, y, width - 2 * margin, row_height};
      SDL_FillSurfaceRect(surface, &highlight,
                          SDL_MapRGBA(format, nullptr, 124, 58, 237, 255));
    }
    if (rows[i]) {
      SDL_Rect dst = {margin + pad_x, y + pad_y, rows[i]->w, rows[i]->h};
      SDL_BlitSurface(rows[i], nullptr, surface, &dst);
      SDL_DestroySurface(rows[i]);
    }
    y += row_height;
  }
  y += margin;
  if (hint) {
    SDL_Rect dst = {margin + pad_x, y, hint->w, hint->h};
    SDL_BlitSurface(hint, nullptr, surface, &dst);
    SDL_DestroySurface(hint);
  }

  ImageCreateInfo info = ImageCreateInfo::immutable_2d_image(
      surface->w, surface->h, VK_FORMAT_B8G8R8A8_UNORM, false);
  ImageInitialData initial_data = {};
  initial_data.data = surface->pixels;
  initial_data.row_length = surface->pitch / 4;
  initial_data.image_height = surface->h;

  ImageHandle handle = device.create_image(info, &initial_data);
  SDL_DestroySurface(surface);
  return handle;
}

void rdp_menu_show(const char *title, const char *const *items, uint32_t count,
                   uint32_t selected, const char *hint) {
  menu.title = title;
  menu.items.assign(items, items + count);
  menu.selected = selected;
  menu.hint = hint;
  menu_visible = true;
  menu_image = Vulkan::ImageHandle();
}

void rdp_menu_hide() {
  menu_visible = false;
  menu_image = Vulkan::ImageHandle();
}

void rdp_toggle_fullscreen() {
  gfx_info.fullscreen = !gfx_info.fullscreen;
  SDL_SetWindowFullscreen(window, gfx_info.fullscreen);
}

bool rdp_is_fullscreen() { return gfx_info.fullscreen; }

void rdp_set_save_state_slot(uint32_t slot) { callback.save_state_slot = slot; }

void rdp_init(void *_window, GFX_INFO _gfx_info, const void *font,
              size_t font_size, uint32_t save_state_slot) {
  memset(&rdp_device, 0, sizeof(RDP_DEVICE));
  memset(&callback, 0, sizeof(CALL_BACK));

  window = (SDL_Window *)_window;
  SDL_SyncWindow(window);
  bool result = SDL_AddEventWatch(sdl_event_filter, nullptr);
  if (!result) {
    LOGE("Could not add event watch.\n");
    return;
  }

  gfx_info = _gfx_info;

  if (gfx_info.crt) {
    fragment_spirv = crt_fragment_spirv;
    fragment_size = sizeof(crt_fragment_spirv);
  } else {
    fragment_spirv = plain_fragment_spirv;
    fragment_size = sizeof(plain_fragment_spirv);
  }

  wsi = new WSI;
  wsi_platform = new SDL_WSIPlatform;
  wsi_platform->set_window(window);
  wsi->set_platform(wsi_platform);
#ifdef __APPLE__
  // MoltenVK has no MAILBOX mode, so asking for "no tearing" gives FIFO, which blocks
  // until the display takes the frame. In fullscreen on a ProMotion display that
  // fights the emulator's own frame pacing: the display slows down to match, the
  // emulator falls further behind, and it settles at about half speed with the audio
  // starving. IMMEDIATE never blocks, so it is used whatever the VSync setting says.
  wsi->set_present_mode(PresentMode::UnlockedMaybeTear);
#else
  if (gfx_info.vsync) {
    // VK_PRESENT_MODE_MAILBOX_KHR, fallback to VK_PRESENT_MODE_FIFO_KHR
    wsi->set_present_mode(PresentMode::UnlockedNoTearing);
  } else {
    // VK_PRESENT_MODE_MAILBOX_KHR, fallback to VK_PRESENT_MODE_IMMEDIATE_KHR
    wsi->set_present_mode(PresentMode::UnlockedMaybeTear);
  }
#endif
  wsi->set_backbuffer_srgb(false);
  Context::SystemHandles handles = {};
  if (!::Vulkan::Context::init_loader(
          (PFN_vkGetInstanceProcAddr)SDL_Vulkan_GetVkGetInstanceProcAddr())) {
    rdp_close();
    return;
  }
  if (!wsi->init_simple(1, handles)) {
    rdp_close();
    return;
  }

  rdp_new_processor();

  if (!processor->device_is_supported()) {
    rdp_close();
    return;
  }

  message_font =
      TTF_OpenFontIO(SDL_IOFromConstMem(font, font_size), true,
                     message_font_size * SDL_GetWindowDisplayScale(window));
  achievement_challenge_indicator_font =
      TTF_OpenFontIO(SDL_IOFromConstMem(font, font_size), true,
                     achievement_challenge_indicator_font_size *
                         SDL_GetWindowDisplayScale(window));
  menu_font = TTF_OpenFontIO(SDL_IOFromConstMem(font, font_size), true,
                             message_font_size);
  if (!message_font || !achievement_challenge_indicator_font || !menu_font) {
    rdp_close();
    return;
  }

  wsi->begin_frame();

  callback.emu_running = true;
  callback.enable_speedlimiter = true;
  callback.paused = false;
  callback.save_state_slot = save_state_slot;
  crop_letterbox = false;

  messages = std::queue<Message>();
  menu_visible = false;
  menu_image = Vulkan::ImageHandle();

  display_challenge_indicator = true;
  achievement_challenge_indicators.clear();
  leaderboard_trackers.clear();
  achievement_challenge_indicator_image = Vulkan::ImageHandle();
  achievement_progress_indicator_image = Vulkan::ImageHandle();
  fps_image = Vulkan::ImageHandle();
  display_fps = false;
}

void rdp_close() {
  display_fps = false;

  g_tmem = nullptr;
  g_hidden_rdram = nullptr;

  messages = std::queue<Message>();
  menu_visible = false;
  menu_image = Vulkan::ImageHandle();
  achievement_challenge_indicator_image = Vulkan::ImageHandle();
  achievement_progress_indicator_image = Vulkan::ImageHandle();
  fps_image = Vulkan::ImageHandle();

  if (wsi)
    wsi->end_frame();

  if (message_font) {
    TTF_CloseFont(message_font);
    message_font = nullptr;
  }
  if (achievement_challenge_indicator_font) {
    TTF_CloseFont(achievement_challenge_indicator_font);
    achievement_challenge_indicator_font = nullptr;
  }
  if (menu_font) {
    TTF_CloseFont(menu_font);
    menu_font = nullptr;
  }
  if (processor) {
    delete processor;
    processor = nullptr;
  }
  if (wsi) {
    delete wsi;
    wsi = nullptr;
  }
  if (wsi_platform) {
    delete wsi_platform;
    wsi_platform = nullptr;
  }

  SDL_RemoveEventWatch(sdl_event_filter, nullptr);
}

static void calculate_viewport(float *x, float *y, float *width, float *height,
                               uint32_t display_height) {
  uint32_t display_width =
      gfx_info.widescreen ? display_height * 16 / 9 : display_height * 4 / 3;

  int w, h;
  SDL_GetWindowSizeInPixels(window, &w, &h);

  if (gfx_info.integer_scaling) {
    // Integer scaling path
    int scale_x = w / display_width;
    int scale_y = h / display_height;
    int scale = (scale_x < scale_y) ? scale_x : scale_y;
    if (scale < 1)
      scale = 1;

    // Calculate scaled dimensions
    int scaled_width = display_width * scale;
    int scaled_height = display_height * scale;

    *width = scaled_width;
    *height = scaled_height;

    // Center the viewport
    int integer_x = (w - *width) / 2.0f;
    int integer_y = (h - *height) / 2.0f;

    *x = integer_x;
    *y = integer_y;
  } else {
    // Regular scaling path - maintain aspect ratio
    float scale_x = w / (float)display_width;
    float scale_y = h / (float)display_height;
    float scale = (scale_x < scale_y) ? scale_x : scale_y;

    *width = display_width * scale;
    *height = display_height * scale;

    // Center the viewport
    *x = (w - *width) / 2.0f;
    *y = (h - *height) / 2.0f;
  }
}

static void draw_indicator(CommandBufferHandle cmd,
                           Vulkan::ImageHandle indicator_image, VkViewport vp) {
  cmd->set_texture(0, 0, indicator_image->get_view(),
                   Vulkan::StockSampler::NearestClamp);
  vp.x = vp.x + vp.width - indicator_image->get_width();
  vp.y = vp.y + vp.height - indicator_image->get_height();
  vp.height = indicator_image->get_height();
  vp.width = indicator_image->get_width();
  cmd->set_viewport(vp);

  cmd->draw(3);
}

static void draw_fps(CommandBufferHandle cmd, VkViewport vp) {
  cmd->set_texture(0, 0, fps_image->get_view(),
                   Vulkan::StockSampler::NearestClamp);
  vp.y = vp.y + vp.height - fps_image->get_height();
  vp.height = fps_image->get_height();
  vp.width = fps_image->get_width();
  cmd->set_viewport(vp);

  cmd->draw(3);
}

static void pop_message(void *userdata) { messages.pop(); }
static uint32_t pop_message_callback(void *userdata, SDL_TimerID timerID,
                                     uint32_t interval) {
  SDL_RunOnMainThread(pop_message, NULL, false);
  return 0;
}

static void render_frame(Vulkan::Device &device) {
  RDP::ScanoutOptions options = {};
  options.persist_frame_on_invalid_input = true;
  options.blend_previous_frame = !gfx_info.ssaa;
  options.upscale_deinterlacing = gfx_info.ssaa;
  if (gfx_info.ssaa) {
    switch (gfx_info.upscale) {
    case 2:
      options.downscale_steps = 1;
      break;
    case 4:
      options.downscale_steps = 2;
      break;
    case 8:
      options.downscale_steps = 3;
      break;
    default:
      options.downscale_steps = 0;
      break;
    }
  }

  if (crop_letterbox && gfx_info.widescreen) {
    options.crop_rect.enable = true;
    if (gfx_info.PAL) {
      options.crop_rect.top = 36;
      options.crop_rect.bottom = 36;
    } else {
      options.crop_rect.top = 30;
      options.crop_rect.bottom = 30;
    }
  }

  Vulkan::ImageHandle image = processor->scanout(options);

  Vulkan::ResourceLayout vertex_layout = {};
  Vulkan::ResourceLayout fragment_layout = {};
  fragment_layout.output_mask = 1 << 0;
  fragment_layout.sets[0].sampled_image_mask = 1 << 0;
  if (gfx_info.crt)
    fragment_layout.push_constant_size = sizeof(Push);

  // This request is cached.
  auto *program =
      device.request_program(vertex_spirv, sizeof(vertex_spirv), fragment_spirv,
                             fragment_size, &vertex_layout, &fragment_layout);

  // Blit image on screen.
  auto cmd = device.request_command_buffer();
  {
    auto rp = device.get_swapchain_render_pass(
        Vulkan::SwapchainRenderPass::ColorOnly);
    cmd->begin_render_pass(rp);

    cmd->set_program(program);

    // Basic default render state.
    cmd->set_opaque_state();
    cmd->set_depth_test(false, false);
    cmd->set_cull_mode(VK_CULL_MODE_NONE);

    VkViewport vp = cmd->get_viewport();
    const VkViewport screen = vp;
    // If we don't have an image, we just get a cleared screen in the render
    // pass.
    if (image) {
      calculate_viewport(&vp.x, &vp.y, &vp.width, &vp.height,
                         image->get_height() / gfx_info.upscale);

      if (gfx_info.crt) {
        // Set shader parameters
        Push push = {
            {float(image->get_width()), float(image->get_height()),
             1.0f / float(image->get_width()),
             1.0f / float(image->get_height())},
            {vp.width, vp.height, 1.0f / vp.width, 1.0f / vp.height},
        };
        cmd->push_constants(&push, 0, sizeof(push));
      }

      cmd->set_texture(0, 0, image->get_view(),
                       Vulkan::StockSampler::NearestClamp);
      cmd->set_viewport(vp);
      // The vertices are constants in the shader.
      // Draws fullscreen quad using oversized triangle.
      cmd->draw(3);
    }
    if (!messages.empty()) {
      Message *message = &messages.front();
      if (!message->image) {
        message->image = create_message_image(device, vp.width, message_font,
                                              message->message.c_str());
        SDL_AddTimer(message->milliseconds, pop_message_callback, NULL);
      }
      cmd->set_texture(0, 0, message->image->get_view(),
                       Vulkan::StockSampler::NearestClamp);
      vp.x = floor(vp.x + (vp.width - message->image->get_width()) / 2);
      vp.y = vp.y + vp.height - message->image->get_height();
      vp.height = message->image->get_height();
      vp.width = message->image->get_width();
      cmd->set_viewport(vp);

      cmd->draw(3);
    } else if (achievement_progress_indicator_image) {
      draw_indicator(cmd, achievement_progress_indicator_image, vp);
    } else if (achievement_challenge_indicator_image &&
               display_challenge_indicator) {
      draw_indicator(cmd, achievement_challenge_indicator_image, vp);
    }

    if (messages.empty() && display_fps && fps_image) {
      draw_fps(cmd, vp);
    }

    if (menu_visible) {
      if (!menu_image)
        menu_image = create_menu_image(device);
      // centred, and shrunk if the window is smaller than the menu
      float fit = std::min({1.0f, screen.width / menu_image->get_width(),
                            screen.height / menu_image->get_height()});
      VkViewport menu_vp = screen;
      menu_vp.width = floor(menu_image->get_width() * fit);
      menu_vp.height = floor(menu_image->get_height() * fit);
      menu_vp.x = floor(screen.x + (screen.width - menu_vp.width) / 2);
      menu_vp.y = floor(screen.y + (screen.height - menu_vp.height) / 2);
      cmd->set_texture(0, 0, menu_image->get_view(),
                       fit < 1.0f ? Vulkan::StockSampler::LinearClamp
                                  : Vulkan::StockSampler::NearestClamp);
      cmd->set_viewport(menu_vp);
      cmd->draw(3);
    }

    cmd->end_render_pass();
  }
  device.submit(cmd);
}

void rdp_set_vi_register(uint32_t reg, uint32_t value) {
  processor->set_vi_register(RDP::VIRegister(reg), value);
}

void rdp_render_frame() {
  auto &device = wsi->get_device();
  render_frame(device);
}

void rdp_update_screen() {
  if (SDL_GetWindowFlags(window) & SDL_WINDOW_MINIMIZED) {
    return;
  }
  if (!wsi->end_frame()) {
    LOGE("End frame failed\n");
    SDL_PumpEvents(); // For Android to trigger pause event
  }
  wsi->begin_frame();
}

CALL_BACK rdp_check_callback() {
  CALL_BACK return_value = callback;
  callback.save_state = false;
  callback.load_rewind = false;
  callback.load_state = false;
  callback.reset_game = false;
  callback.lower_volume = false;
  callback.raise_volume = false;
  callback.decrease_input_delay = false;
  callback.increase_input_delay = false;
  callback.frame_advance = false;
  callback.open_menu = false;
  return return_value;
}

void rdp_check_framebuffers(uint32_t address, uint32_t length) {
  if (sync_signal) {
    address >>= 3;
    length = (length + 7) >> 3;

    if (address >= rdram_dirty.size())
      return;

    uint32_t end_addr =
        std::min(address + length, static_cast<uint32_t>(rdram_dirty.size()));

    auto it = std::find(rdram_dirty.begin() + address,
                        rdram_dirty.begin() + end_addr, true);
    if (it != rdram_dirty.begin() + end_addr) {
      processor->wait_for_timeline(sync_signal);
      rdram_dirty.assign(gfx_info.RDRAM_SIZE >> 3, false);
      sync_signal = 0;
    }
  }
}

size_t rdp_state_size() {
  return sizeof(RDP_DEVICE) + 0x1000 + processor->get_hidden_rdram_size();
}

void rdp_save_state(uint8_t *state) {
  memcpy(state, &rdp_device, sizeof(RDP_DEVICE));

  if (g_tmem) {
    memcpy(state + sizeof(RDP_DEVICE), g_tmem, 0x1000);
  }

  if (g_hidden_rdram) {
    memcpy(state + sizeof(RDP_DEVICE) + 0x1000, g_hidden_rdram,
           processor->get_hidden_rdram_size());
  }
}

void rdp_load_state(GFX_INFO _gfx_info, const uint8_t *state) {
  gfx_info = _gfx_info;
  memcpy(&rdp_device, state, sizeof(RDP_DEVICE));

  if (g_tmem) {
    memcpy(g_tmem, state + sizeof(RDP_DEVICE), 0x1000);
  }

  if (g_hidden_rdram) {
    memcpy(g_hidden_rdram, state + sizeof(RDP_DEVICE) + 0x1000,
           processor->get_hidden_rdram_size());
  }
}

static void push_onscreen_message(void *data) {
  Message *message = (Message *)data;
  messages.push(*message);
  delete message;
}

void rdp_onscreen_message(const char *message, MESSAGE_LENGTH milliseconds) {
  Message *data = new Message;
  data->message = message;
  data->milliseconds = static_cast<uint64_t>(milliseconds);
  data->image = Vulkan::ImageHandle();
  SDL_RunOnMainThread(push_onscreen_message, data, false);
}

uint32_t pixel_size(uint32_t pixel_type, uint32_t area) {
  switch (pixel_type) {
  case 0:
    return area / 2;
  case 1:
    return area;
  case 2:
    return area * 2;
  case 3:
    return area * 4;
  default:
    LOGE("Invalid pixel size: %u\n", pixel_type);
    return 0;
  }
}

uint64_t rdp_process_commands() {
  uint64_t interrupt_timer = 0;
  const uint32_t DP_CURRENT = *gfx_info.DPC_CURRENT_REG & 0x00FFFFF8;
  const uint32_t DP_END = *gfx_info.DPC_END_REG & 0x00FFFFF8;

  int length = DP_END - DP_CURRENT;
  if (length <= 0)
    return interrupt_timer;

  length = unsigned(length) >> 3;
  if ((rdp_device.cmd_ptr + length) & ~(0x0003FFFF >> 3))
    return interrupt_timer;

  uint32_t offset = DP_CURRENT;
  if (*gfx_info.DPC_STATUS_REG & DP_STATUS_XBUS_DMA) {
    do {
      offset &= 0xFF8;
      rdp_device.cmd_data[2 * rdp_device.cmd_ptr + 0] = SDL_Swap32BE(
          *reinterpret_cast<const uint32_t *>(gfx_info.DMEM + offset));
      rdp_device.cmd_data[2 * rdp_device.cmd_ptr + 1] = SDL_Swap32BE(
          *reinterpret_cast<const uint32_t *>(gfx_info.DMEM + offset + 4));
      offset += sizeof(uint64_t);
      rdp_device.cmd_ptr++;
    } while (--length > 0);
  } else {
    if (DP_END > 0x7ffffff || DP_CURRENT > 0x7ffffff) {
      return interrupt_timer;
    } else {
      do {
        offset &= 0xFFFFF8;
        rdp_device.cmd_data[2 * rdp_device.cmd_ptr + 0] =
            *reinterpret_cast<const uint32_t *>(gfx_info.RDRAM + offset);
        rdp_device.cmd_data[2 * rdp_device.cmd_ptr + 1] =
            *reinterpret_cast<const uint32_t *>(gfx_info.RDRAM + offset + 4);
        offset += sizeof(uint64_t);
        rdp_device.cmd_ptr++;
      } while (--length > 0);
    }
  }

  while (rdp_device.cmd_cur - rdp_device.cmd_ptr < 0) {
    uint32_t w1 = rdp_device.cmd_data[2 * rdp_device.cmd_cur];
    uint32_t w2 = rdp_device.cmd_data[2 * rdp_device.cmd_cur + 1];
    uint32_t command = (w1 >> 24) & 63;
    int cmd_length = cmd_len_lut[command];

    if (rdp_device.cmd_ptr - rdp_device.cmd_cur - cmd_length < 0) {
      *gfx_info.DPC_START_REG = *gfx_info.DPC_CURRENT_REG =
          *gfx_info.DPC_END_REG;
      return interrupt_timer;
    }

    if (command >= 8)
      processor->enqueue_command(cmd_length * 2,
                                 &rdp_device.cmd_data[2 * rdp_device.cmd_cur]);

    switch (RDP::Op(command)) {
    case RDP::Op::FillTriangle:
    case RDP::Op::FillZBufferTriangle:
    case RDP::Op::TextureTriangle:
    case RDP::Op::TextureZBufferTriangle:
    case RDP::Op::ShadeTriangle:
    case RDP::Op::ShadeZBufferTriangle:
    case RDP::Op::ShadeTextureTriangle:
    case RDP::Op::ShadeTextureZBufferTriangle:
    case RDP::Op::TextureRectangle:
    case RDP::Op::TextureRectangleFlip:
    case RDP::Op::FillRectangle: {
      uint32_t offset_address =
          (rdp_device.frame_buffer_info.framebuffer_address +
           pixel_size(rdp_device.frame_buffer_info.framebuffer_pixel_size,
                      rdp_device.frame_buffer_info.framebuffer_y_offset *
                          rdp_device.frame_buffer_info.framebuffer_width)) >>
          3;
      if (offset_address < rdram_dirty.size() && !rdram_dirty[offset_address]) {
        uint32_t end_addr = std::min(
            offset_address +
                ((pixel_size(
                      rdp_device.frame_buffer_info.framebuffer_pixel_size,
                      rdp_device.frame_buffer_info.framebuffer_width *
                          rdp_device.frame_buffer_info.framebuffer_height) +
                  7) >>
                 3),
            static_cast<uint32_t>(rdram_dirty.size()));
        std::fill(rdram_dirty.begin() + offset_address,
                  rdram_dirty.begin() + end_addr, true);
      }

      if (rdp_device.frame_buffer_info.depth_buffer_enabled) {
        offset_address =
            (rdp_device.frame_buffer_info.depthbuffer_address +
             pixel_size(2,
                        rdp_device.frame_buffer_info.framebuffer_y_offset *
                            rdp_device.frame_buffer_info.framebuffer_width)) >>
            3;
        if (offset_address < rdram_dirty.size() &&
            !rdram_dirty[offset_address]) {
          uint32_t end_addr = std::min(
              offset_address +
                  ((pixel_size(
                        2,
                        rdp_device.frame_buffer_info.framebuffer_width *
                            rdp_device.frame_buffer_info.framebuffer_height) +
                    7) >>
                   3),
              static_cast<uint32_t>(rdram_dirty.size()));
          std::fill(rdram_dirty.begin() + offset_address,
                    rdram_dirty.begin() + end_addr, true);
        }
      }
    } break;
    case RDP::Op::LoadTLut:
    case RDP::Op::LoadTile: {
      uint32_t upper_left_t = (w1 & 0xFFF) >> 2;
      uint32_t offset_address =
          (rdp_device.frame_buffer_info.texture_address +
           pixel_size(rdp_device.frame_buffer_info.texture_pixel_size,
                      upper_left_t *
                          rdp_device.frame_buffer_info.texture_width)) >>
          3;
      if (offset_address < rdram_dirty.size() && !rdram_dirty[offset_address]) {
        uint32_t lower_right_t = (w2 & 0xFFF) >> 2;
        uint32_t end_addr = std::min(
            offset_address +
                ((pixel_size(rdp_device.frame_buffer_info.texture_pixel_size,
                             (lower_right_t - upper_left_t) *
                                 rdp_device.frame_buffer_info.texture_width) +
                  7) >>
                 3),
            static_cast<uint32_t>(rdram_dirty.size()));
        std::fill(rdram_dirty.begin() + offset_address,
                  rdram_dirty.begin() + end_addr, true);
      }
    } break;
    case RDP::Op::LoadBlock: {
      uint32_t upper_left_s = ((w1 >> 12) & 0xFFF);
      uint32_t upper_left_t = (w1 & 0xFFF);
      uint32_t offset_address =
          (rdp_device.frame_buffer_info.texture_address +
           pixel_size(rdp_device.frame_buffer_info.texture_pixel_size,
                      upper_left_s +
                          upper_left_t *
                              rdp_device.frame_buffer_info.texture_width)) >>
          3;
      if (offset_address < rdram_dirty.size() && !rdram_dirty[offset_address]) {
        uint32_t lower_right_s = ((w2 >> 12) & 0xFFF);
        uint32_t end_addr = std::min(
            offset_address +
                ((pixel_size(rdp_device.frame_buffer_info.texture_pixel_size,
                             lower_right_s - upper_left_s) +
                  7) >>
                 3),
            static_cast<uint32_t>(rdram_dirty.size()));
        std::fill(rdram_dirty.begin() + offset_address,
                  rdram_dirty.begin() + end_addr, true);
      }
    } break;
    case RDP::Op::SetColorImage:
      rdp_device.frame_buffer_info.framebuffer_address = (w2 & 0x00FFFFFF);
      rdp_device.frame_buffer_info.framebuffer_pixel_size = (w1 >> 19) & 0x3;
      rdp_device.frame_buffer_info.framebuffer_width = (w1 & 0x3FF) + 1;
      break;
    case RDP::Op::SetMaskImage:
      rdp_device.frame_buffer_info.depthbuffer_address = (w2 & 0x00FFFFFF);
      break;
    case RDP::Op::SetTextureImage:
      rdp_device.frame_buffer_info.texture_address = (w2 & 0x00FFFFFF);
      rdp_device.frame_buffer_info.texture_pixel_size = (w1 >> 19) & 0x3;
      rdp_device.frame_buffer_info.texture_width = (w1 & 0x3FF) + 1;
      break;
    case RDP::Op::SetScissor: {
      uint32_t upper_left_x = ((w1 >> 12) & 0xFFF) >> 2;
      uint32_t upper_left_y = (w1 & 0xFFF) >> 2;
      uint32_t lower_right_x = ((w2 >> 12) & 0xFFF) >> 2;
      uint32_t lower_right_y = (w2 & 0xFFF) >> 2;
      if (lower_right_x > upper_left_x && lower_right_y > upper_left_y) {
        rdp_device.region =
            (lower_right_x - upper_left_x) * (lower_right_y - upper_left_y);
      } else {
        rdp_device.region = 0;
      }

      rdp_device.frame_buffer_info.framebuffer_y_offset = upper_left_y;
      rdp_device.frame_buffer_info.framebuffer_height =
          lower_right_y - upper_left_y;
    } break;
    case RDP::Op::SetOtherModes: {
      uint8_t cycle_type = (w1 >> 20) & 3;
      uint8_t depth_read_write = (w2 >> 4) & 3;
      rdp_device.frame_buffer_info.depth_buffer_enabled =
          ((cycle_type & 2) == 0) && (depth_read_write != 0);
    } break;
    case RDP::Op::SyncFull:
      sync_signal = processor->signal_timeline();

      interrupt_timer = rdp_device.region;
      if (interrupt_timer == 0)
        interrupt_timer = 5000;
      break;
    default:
      break;
    }

    rdp_device.cmd_cur += cmd_length;
  }

  rdp_device.cmd_ptr = 0;
  rdp_device.cmd_cur = 0;
  *gfx_info.DPC_CURRENT_REG = *gfx_info.DPC_END_REG;

  return interrupt_timer;
}

static void rdp_set_fps_callback(void *userdata) {
  FPS_DATA *data = (FPS_DATA *)userdata;
  if (!wsi) {
    delete data;
    return;
  }
  fps_image = create_message_image(
      wsi->get_device(), 0, achievement_challenge_indicator_font,
      std::format("FPS: {} VI/S: {}", data->fps, data->vis).c_str());
  delete data;
}

void rdp_set_fps(uint32_t fps, uint32_t vis) {
  if (display_fps) {
    FPS_DATA *data = new FPS_DATA;
    data->fps = fps;
    data->vis = vis;
    SDL_RunOnMainThread(rdp_set_fps_callback, data, false);
  }
}

static void update_challenge_indicator() {
  std::string message;
  for (const auto &leaderboard_tracker : leaderboard_trackers) {
    message += leaderboard_tracker.second;
    message += '\n';
  }

  if (!leaderboard_trackers.empty() &&
      !achievement_challenge_indicators.empty()) {
    message += "---\n";
  }

  const auto &v = achievement_challenge_indicators;
  for (size_t i = 0; i < std::min<size_t>(v.size(), 5); ++i) {
    message += v[i];
    message += '\n';
  }

  if (message.empty()) {
    achievement_challenge_indicator_image = Vulkan::ImageHandle();
  } else {
    achievement_challenge_indicator_image = create_message_image(
        wsi->get_device(), 0, achievement_challenge_indicator_font,
        message.c_str());
  }
}

void achievement_challenge_indicator_add(const char *achievement_title) {
  achievement_challenge_indicators.push_back(achievement_title);
  update_challenge_indicator();
}

void achievement_challenge_indicator_remove(const char *achievement_title) {
  std::erase(achievement_challenge_indicators, achievement_title);
  update_challenge_indicator();
}

void achievement_progress_add(const char *achievement_title,
                              const char *progress) {
  std::string message = std::format("{}: {}", achievement_title, progress);
  achievement_progress_indicator_image =
      create_message_image(wsi->get_device(), 0, message_font, message.c_str());
}

void achievement_progress_remove() {
  achievement_progress_indicator_image = Vulkan::ImageHandle();
}

void leaderboard_tracker_add(uint32_t id, const char *title,
                             const char *display) {
  if (title) {
    leaderboard_trackers[id] = std::format("{}: {}", title, display);
    update_challenge_indicator();
  }
}

void leaderboard_tracker_remove(uint32_t id) {
  leaderboard_trackers.erase(id);
  update_challenge_indicator();
}
