#include "oxitone_plugin.h"
#include <math.h>
#include <stdlib.h>

#ifndef FIXTURE_LAYOUT
#define FIXTURE_LAYOUT OXI_STEREO
#endif
#ifndef FIXTURE_KIND
#define FIXTURE_KIND OXI_EFFECT
#endif
#ifndef FIXTURE_ABI
#define FIXTURE_ABI 1
#endif
#ifndef FIXTURE_FAULT
#define FIXTURE_FAULT 0
#endif

typedef struct { float gain, voice; uint64_t latency; } Gain;

static void *create(const OxiHostContextV1 *host) {
    if (host->max_block_size == 13) return NULL;
    Gain *g = calloc(1, sizeof(Gain));
    if (g) g->gain = 1.0f;
    return g;
}
static void dispose(void *instance) { free(instance); }
static uint32_t prepare(void *instance, double rate, uint32_t block) {
    (void)instance;
    return rate > 0 && block != 13 ? 0 : 1;
}
static void reset(void *instance) {
    Gain *g = instance;
    g->voice = 0;
}
static uint64_t tail(const void *instance) {
    return ((const Gain *)instance)->voice > 0 ? 17 : 0;
}
static uint64_t latency(const void *instance) {
    return ((const Gain *)instance)->latency;
}
static uint32_t process(void *instance, const OxiProcessContextV1 *ctx) {
    Gain *g = instance;
    if (FIXTURE_FAULT == 1) return 1;
    if (FIXTURE_FAULT == 3) g->latency++;
    uint32_t p = 0, n = 0;
    for (uint32_t f = 0; f < ctx->frames; f++) {
        while (p < ctx->parameter_count && ctx->parameters[p].frame_offset == f) {
            if (ctx->parameters[p].parameter_index != 0) return 2;
            g->gain = (float)ctx->parameters[p++].value;
        }
        while (n < ctx->note_count && ctx->notes[n].frame_offset == f) {
            const OxiNoteEventV1 *note = &ctx->notes[n++];
            g->voice = note->kind == OXI_NOTE_ON ? note->velocity * (note->pitch / 64.0f) : 0;
        }
        for (uint32_t c = 0; c < ctx->output_count; c++) {
            float input = ctx->input_count ? ctx->inputs[c][f] : g->voice;
            float side = ctx->sidechain_count ? ctx->sidechain[c][f] : 0;
            ctx->outputs[c][f] = FIXTURE_FAULT == 2 ? NAN : (input + side) * g->gain;
        }
    }
    return 0;
}

static const OxiParamSpecV1 parameters[] = {
    {"gain", "Gain", OXI_NORMALIZED, 0, 2, 1, OXI_SMOOTH_NONE, OXI_AUDIO_RATE, 1, OXI_MAP_LINEAR}
};
static const OxiPluginDescriptorV1 descriptor = {
    1, 0, "fixture.gain", "1.0.0", FIXTURE_KIND,
    FIXTURE_KIND == OXI_INSTRUMENT ? OXI_NONE : FIXTURE_LAYOUT,
    FIXTURE_LAYOUT, OXI_SIDECHAIN | OXI_REPORTS_TAIL, parameters, 1,
    FIXTURE_KIND == OXI_INSTRUMENT ? 1 : 0
};
static const OxiPluginEntryV1 entry = {
    FIXTURE_ABI, 0, sizeof(OxiPluginEntryV1), &descriptor, "0.1.0",
    create, dispose, prepare, process, reset, tail, latency
};
const OxiPluginEntryV1 *oxitone_plugin_entry_v1(void) { return &entry; }

#ifdef FIXTURE_RUNNER
#include <stdio.h>
/* The same C implementation linked statically, for bitwise comparison. */
int main(void) {
    OxiHostContextV1 host = {48000, 8};
    void *g = create(&host);
    if (!g || prepare(g, 48000, 8)) return 1;
    const float l[] = {1,2,3,4,5,6,7,8}, r[] = {8,7,6,5,4,3,2,1};
    const float side[] = {0.25f,0.25f,0.25f,0.25f,0.25f,0.25f,0.25f,0.25f};
    const float *inputs[] = {l,r}, *sidechain[] = {side,side};
    float left[8], right[8];
    float *outputs[] = {left,right};
    OxiParameterEventV1 params[] = {{0,0,0.5}, {3,0,0.25}, {3,0,0.75}};
    OxiProcessContextV1 ctx = {8,48000,inputs,2,outputs,2,NULL,0,params,3,sidechain,2};
    if (process(g, &ctx)) return 2;
    fwrite(left, sizeof(float), 8, stdout);
    fwrite(right, sizeof(float), 8, stdout);
    dispose(g);
    return 0;
}
#endif
