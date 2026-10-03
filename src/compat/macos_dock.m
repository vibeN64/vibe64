// VibeN64 runs a game in a process of its own (see run_rom in src/ui/gui.rs). To macOS
// the launcher and the game are two separate applications, each with a Dock icon and a
// place in the app switcher. While a game runs the launcher steps out of the way, so that
// there is one VibeN64 on screen, and comes back when the game ends.

#import <AppKit/AppKit.h>
#include <stdbool.h>

void viben64_launcher_away(bool away)
{
	dispatch_async(dispatch_get_main_queue(), ^{
		if (away) {
			[NSApp hide:nil];
			// no Dock icon and no entry in the app switcher
			[NSApp setActivationPolicy:NSApplicationActivationPolicyAccessory];
		} else {
			[NSApp setActivationPolicy:NSApplicationActivationPolicyRegular];
			[NSApp unhide:nil];
			[NSApp activateIgnoringOtherApps:YES];
		}
	});
}
