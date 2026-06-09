#include <stddef.h>

extern void *rust_stbtt_malloc(size_t size, void *userdata);
extern void rust_stbtt_free(void *ptr, void *userdata);

#define STBTT_malloc(x,u)  rust_stbtt_malloc(x,u)
#define STBTT_free(x,u)    rust_stbtt_free(x,u)

#define STB_TRUETYPE_IMPLEMENTATION
#include "../third-party/stb/stb_truetype.h"
