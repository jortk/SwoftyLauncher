/* SPDX-License-Identifier: MIT */
#pragma once
#include <stdint.h>

/* Image access through SwoftyCoreLive table v4 images(). All results are copied into caller-owned
   memory. No image files or pointers into the game's resources are returned.
   start enables bounded, asynchronous readback of named runtime badge textures.
   image accepts team_badge_texture references, including catalogue and runtime badges.
   Catalogue reads and decoding run on a worker only when requested, following active redirects.
   LATE means an archive read is pending; UNAVAILABLE means no usable image.
   sources returns the active game-relative archive redirects, not a cache search.
   image returns RGBA8 bytes with alpha, at most 64x64; caller provides 16,384 bytes.
   Check the table version before accessing its members. Older cores may omit it. */
typedef struct SwoftyLiveImages
{
    uint32_t version;
    int (*start)(void);
    int (*sources)(char *json, uint32_t cap, uint64_t *revision);
    int (*image)(const char *reference, uint8_t *rgba, uint32_t cap, uint32_t *width, uint32_t *height,
                 uint64_t *revision);
    void (*clear)(void); /* explicit cache reset: retire pixels; queued GPU work stays alive until its fence */
} SwoftyLiveImages;
