"""Run the import-free audio ABI in Wasmtime without JavaScript.
Install the optional wasmtime Python package, then run from the repository root.
"""
import json
import math
import struct
from importlib.metadata import version
from pathlib import Path
import wasmtime

runtime = wasmtime.Engine()
module = wasmtime.Module.from_file(runtime, "packages/web/dist/oxitone.wasm")
assert len(module.imports) == 0
store = wasmtime.Store(runtime)
abi = wasmtime.Instance(store, module, []).exports(store)
memory = abi["memory"]

def command(value):
    data = json.dumps({"protocolVersion": "1.0", **value}).encode()
    ptr = abi["oxi_alloc"](store, len(data))
    memory.write(store, data, ptr)
    abi["oxi_command"](store, ptr, len(data), 0, 0)
    start = abi["oxi_response_ptr"](store)
    result = json.loads(bytes(memory.read(store, start, start + abi["oxi_response_len"](store))))
    abi["oxi_free"](store, ptr, len(data))
    assert result["ok"], result
    return result["value"]

snapshot = {
    "protocolVersion": "1.0", "id": "prj_wasmtime", "revision": "0",
    "sampleRate": 48000, "blockSize": 128, "seed": 17,
    "tempoMap": [{"startBeat": {"numerator": 0, "denominator": 1}, "bpm": 120}],
    "timeSignatureMap": [{"startBar": 1, "numerator": 4, "denominator": 4}],
    "markers": [], "samples": [], "sampleClips": [], "automation": [], "mixerChannels": [],
    "channels": [{"id": "chn_drums", "instrument": {
        "pluginId": "example.drums", "pluginVersion": "1.0.0", "parameters": {}},
        "effectChain": [], "level": 0.8, "pan": 0, "mixerChannelId": "mix_master"}],
    "tracks": [{"id": "trk_drums", "channelIds": ["chn_drums"], "patternClipIds": ["pcl_drums"], "sampleClipIds": []}],
    "patterns": [{"id": "pat_drums", "lengthBeats": {"numerator": 4, "denominator": 1},
        "notes": [{"pitch": 36, "start": {"numerator": 0, "denominator": 1},
                   "duration": {"numerator": 1, "denominator": 4}, "velocity": 1}]}],
    "patternClips": [{"id": "pcl_drums", "trackId": "trk_drums", "patternId": "pat_drums",
        "startBeat": {"numerator": 0, "denominator": 1}}],
}
state = command({"type": "compile", "snapshot": snapshot})
command({"type": "transport", "command": "play"})
before = [memory.data_len(store), abi["oxi_allocations"](store), abi["oxi_deallocations"](store)]
left = abi["oxi_left_ptr"](store)
peak = 0
for _ in range(1000):
    assert abi["oxi_process"](store) == 0
    for value in struct.unpack("<128f", memory.read(store, left, left + 512)):
        assert math.isfinite(value)
        peak = max(peak, abs(value))
after = [memory.data_len(store), abi["oxi_allocations"](store), abi["oxi_deallocations"](store)]
assert peak > 0.01
assert before == after
command({"type": "renderWav", "frames": 48000, "bitDepth": 24})
start = abi["oxi_binary_ptr"](store)
wav = memory.read(store, start, start + abi["oxi_binary_len"](store))
assert wav[:4] == b"RIFF"
output = Path("target/examples/wasm")
output.mkdir(parents=True, exist_ok=True)
(output / "wasmtime-drums.wav").write_bytes(wav)
report = {"runtime": "wasmtime", "runtimeVersion": version("wasmtime"), "imports": 0, "peak": peak, "blocks": 1000,
          "memoryBefore": before, "memoryAfter": after, "wavBytes": len(wav), "state": state}
(output / "wasmtime-report.json").write_text(json.dumps(report, indent=2) + "\n")
print(json.dumps(report))
