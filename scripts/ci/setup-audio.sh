#!/usr/bin/env bash
# Hosted runners have no physical output. Keep the real CoreAudio test assertions
# by supplying a virtual output; this does not count as physical-device acceptance.
set -euo pipefail
if [[ "${CI:-}" != "true" || "${RUNNER_OS:-}" != "macOS" ]]; then
  echo "This script configures audio only on a macOS CI runner." >&2
  exit 1
fi
brew install switchaudio-osx
brew install --cask blackhole-2ch
sudo launchctl kickstart -kp system/com.apple.audio.coreaudiod
for attempt in {1..20}; do
  if SwitchAudioSource -a -t output | grep -Fxq 'BlackHole 2ch'; then
    SwitchAudioSource -s 'BlackHole 2ch' -t output
    SwitchAudioSource -s 'BlackHole 2ch' -t system
    SwitchAudioSource -c -t output > target/ci/audio-device.txt
    echo "Virtual CoreAudio output ready after attempt $attempt."
    exit 0
  fi
  sleep 1
done
echo "BlackHole did not become available as a CoreAudio output." >&2
exit 1
