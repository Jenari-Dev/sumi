/* Pointer input for the virtual-KWin GUI checks, through KWin's fake-input protocol.
 *
 *   kwin-input <step>...      steps: move X Y | press | release | click | dblclick | wait MS
 *                             (X, Y: logical pixels of the output; `move` jumps, it does not glide)
 *
 * KWin only serves it to executables named in a desktop file with
 * X-KDE-Wayland-Interfaces=org_kde_kwin_fake_input (tests/kwin-overlay.sh writes it).
 * Built on the fly by tests/kwin-overlay.sh:  wayland-scanner + cc, no other dependency. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <wayland-client.h>
#include "fake-input-client-protocol.h"

static struct org_kde_kwin_fake_input *fake;

static void global(void *d, struct wl_registry *r, uint32_t name, const char *iface, uint32_t v)
{
    if (!strcmp(iface, "org_kde_kwin_fake_input"))
        fake = wl_registry_bind(r, name, &org_kde_kwin_fake_input_interface, v < 4 ? v : 4);
}
static void global_remove(void *d, struct wl_registry *r, uint32_t n) {}
static const struct wl_registry_listener rl = { global, global_remove };



int main(int argc, char **argv)
{
    struct wl_display *dpy = wl_display_connect(NULL);
    if (!dpy) { fprintf(stderr, "no Wayland display\n"); return 2; }
    struct wl_registry *reg = wl_display_get_registry(dpy);
    wl_registry_add_listener(reg, &rl, NULL);
    wl_display_roundtrip(dpy);
    if (!fake) { fprintf(stderr, "no org_kde_kwin_fake_input\n"); return 3; }
    org_kde_kwin_fake_input_authenticate(fake, "sumi-tests", "pointer input for GUI checks");
    wl_display_roundtrip(dpy);
    for (int i = 1; i < argc; i++) {
        if (!strcmp(argv[i], "move") && i + 2 < argc) {
            org_kde_kwin_fake_input_pointer_motion_absolute(fake, wl_fixed_from_double(atof(argv[i + 1])), wl_fixed_from_double(atof(argv[i + 2])));
            i += 2;
        } else if (!strcmp(argv[i], "press")) {
            org_kde_kwin_fake_input_button(fake, 272, 1);
        } else if (!strcmp(argv[i], "release")) {
            org_kde_kwin_fake_input_button(fake, 272, 0);
        } else if (!strcmp(argv[i], "click") || !strcmp(argv[i], "dblclick")) {
            int n = argv[i][0] == 'd' ? 2 : 1;
            for (int k = 0; k < n; k++) {
                org_kde_kwin_fake_input_button(fake, 272, 1);
                wl_display_flush(dpy);
                usleep(40000);
                org_kde_kwin_fake_input_button(fake, 272, 0);
                wl_display_flush(dpy);
                usleep(80000);
            }
        } else if (!strcmp(argv[i], "wait") && i + 1 < argc) {
            wl_display_flush(dpy);
            usleep(atoi(argv[++i]) * 1000);
        }
        wl_display_flush(dpy);
        usleep(15000);
    }
    wl_display_roundtrip(dpy);
    wl_display_disconnect(dpy);
    return 0;
}
