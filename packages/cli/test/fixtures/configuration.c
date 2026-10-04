/* Source-writeback fixture: dotted IDs and a plugin parameter named like host Mix. */
#include "oxitone_plugin.h"
#include <stdlib.h>

typedef struct { double values[2]; } Configuration;
static void *create(const OxiHostContextV1 *host) {
    (void)host;
    Configuration *instance = calloc(1, sizeof(Configuration));
    if (instance) instance->values[0] = instance->values[1] = 1;
    return instance;
}
static void dispose(void *instance) { free(instance); }
static uint32_t prepare(void *instance, double rate, uint32_t block) {
    (void)instance;
    return rate > 0 && block > 0 ? 0 : 1;
}
static void reset(void *instance) { (void)instance; }
static uint64_t zero(const void *instance) { (void)instance; return 0; }
static uint32_t process(void *instance, const OxiProcessContextV1 *ctx) {
    Configuration *configuration = instance;
    uint32_t parameter = 0;
    for (uint32_t frame = 0; frame < ctx->frames; frame++) {
        while (parameter < ctx->parameter_count && ctx->parameters[parameter].frame_offset == frame) {
            const OxiParameterEventV1 *event = &ctx->parameters[parameter++];
            if (event->parameter_index >= 2) return 1;
            configuration->values[event->parameter_index] = event->value;
        }
        for (uint32_t channel = 0; channel < ctx->output_count; channel++)
            ctx->outputs[channel][frame] = ctx->inputs[channel][frame]
                * (1 + configuration->values[1] * (configuration->values[0] - 1));
    }
    return 0;
}
static const OxiParamSpecV1 parameters[] = {
    {"band.2.gain", "Band gain", OXI_NORMALIZED, 0, 2, 1, OXI_SMOOTH_NONE, OXI_AUDIO_RATE, 1, OXI_MAP_LINEAR},
    {"mix", "Plugin mix", OXI_NORMALIZED, 0, 1, 1, OXI_SMOOTH_NONE, OXI_AUDIO_RATE, 1, OXI_MAP_LINEAR}
};
static const OxiPluginDescriptorV1 descriptor = {
    1, 0, "fixture.configuration", "1.0.0", OXI_EFFECT, OXI_STEREO, OXI_STEREO,
    0, parameters, 2, 0
};
static const OxiPluginEntryV1 entry = {
    1, 0, sizeof(OxiPluginEntryV1), &descriptor, "0.1.0",
    create, dispose, prepare, process, reset, zero, zero
};
const OxiPluginEntryV1 *oxitone_plugin_entry_v1(void) { return &entry; }
