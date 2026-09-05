#ifndef OXITONE_PLUGIN_H
#define OXITONE_PLUGIN_H
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ABI v1.0, native alignment; no packing pragmas. Strings are UTF-8/NUL.
 * Metadata and function pointers remain valid until library unload.
 * All enums use uint32_t. A zero lifecycle status means success.
 */
enum { OXI_INSTRUMENT = 0, OXI_EFFECT = 1 };
enum { OXI_NONE = 0, OXI_MONO = 1, OXI_STEREO = 2 };
enum { OXI_SIDECHAIN = 1, OXI_REPORTS_TAIL = 2 };
enum { OXI_NORMALIZED = 0, OXI_DB, OXI_HZ, OXI_SEMITONES,
       OXI_SECONDS, OXI_BEATS, OXI_ENUM };
enum { OXI_SMOOTH_NONE = 0, OXI_SMOOTH_LINEAR, OXI_SMOOTH_ONE_POLE };
enum { OXI_CONTROL_RATE = 0, OXI_AUDIO_RATE = 1 };
enum { OXI_MAP_DEFAULT = 0, OXI_MAP_LINEAR, OXI_MAP_LOG,
       OXI_MAP_BIPOLAR, OXI_MAP_ENUM };
enum { OXI_NOTE_ON = 0, OXI_NOTE_OFF = 1 };

typedef struct {
    const char *id, *label;
    uint32_t unit;
    double min, max, default_value;
    uint32_t smoothing, rate;
    uint8_t automation;
    uint32_t mapping;
} OxiParamSpecV1;

typedef struct {
    uint32_t abi_major, abi_minor;
    const char *plugin_id, *plugin_version;
    uint32_t kind, input_layout, output_layout, capabilities;
    const OxiParamSpecV1 *params;
    uint32_t param_count, max_polyphony;
} OxiPluginDescriptorV1;

typedef struct {
    double sample_rate;
    uint32_t max_block_size;
} OxiHostContextV1;

typedef struct {
    uint32_t frame_offset, kind;
    uint8_t pitch;
    float velocity;
} OxiNoteEventV1;

typedef struct {
    uint32_t frame_offset, parameter_index;
    double value;
} OxiParameterEventV1;

/* Pointers borrow host-owned buffers for this call only. Each channel has
 * frames samples; counts define valid table lengths, including zero.
 * Events are sorted by frame_offset (< frames), stable at equal offsets.
 * Parameter indices refer to descriptor order, values use physical units.
 * At a shared frame: note-off, parameter, note-on. No pointer may be kept.
 */
typedef struct {
    uint32_t frames;
    double sample_rate;
    const float *const *inputs;
    uint32_t input_count;
    float *const *outputs;
    uint32_t output_count;
    const OxiNoteEventV1 *notes;
    uint32_t note_count;
    const OxiParameterEventV1 *parameters;
    uint32_t parameter_count;
    const float *const *sidechain;
    uint32_t sidechain_count;
} OxiProcessContextV1;

typedef struct {
    uint32_t abi_major, abi_minor, struct_size;
    const OxiPluginDescriptorV1 *descriptor;
    const char *min_host_version;
    void *(*create)(const OxiHostContextV1 *);
    void (*dispose)(void *);
    uint32_t (*prepare)(void *, double, uint32_t);
    uint32_t (*process)(void *, const OxiProcessContextV1 *);
    void (*reset)(void *);
    uint64_t (*tail_frames)(const void *);
    uint64_t (*latency_frames)(const void *);
} OxiPluginEntryV1;

/* create/prepare/dispose: control thread, may allocate. create returns
 * NULL on failure. process/reset/tail_frames/latency_frames: realtime-safe,
 * no allocation/deallocation, locks, blocking, I/O or unwinding. Instances
 * may move between threads; host serializes access to each instance.
 * latency may change only in prepare. reset flushes voices/delay state.
 */
const OxiPluginEntryV1 *oxitone_plugin_entry_v1(void);

#ifdef __cplusplus
}
#endif
#endif
