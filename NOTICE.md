# Notices

Dispatch is free software under the MIT License (see [LICENSE](LICENSE)), with the exceptions listed below. It is a fork of [Handy](https://github.com/cjpais/Handy) by CJ Pais, also MIT-licensed; Handy's copyright notice is kept in LICENSE.

Full license texts are in the [licenses](licenses) folder, which the Windows installer also carries.

## Not covered by the MIT License: the railroad sounds

These three sound effects were generated with [ElevenLabs](https://elevenlabs.io) on a paid plan and are used under the [ElevenLabs Sound Effects Terms](https://elevenlabs.io/sound-effects-terms):

- `src-tauri/resources/railroad_start.wav`
- `src-tauri/resources/railroad_stop.wav`
- `src/assets/dispatch/sounds/achievement-whistle.wav`

They are included only as part of Dispatch. They are **not** licensed under the MIT License, and you may not extract them to sell, license or distribute on their own, as samples or as part of a sound library. If you fork Dispatch, you can keep them in your fork as part of the app, or replace them with your own.

## Third-party components Dispatch adds to Handy

Handy's own dependencies are listed in its repository and in `src-tauri/Cargo.lock` and `bun.lock`. Dispatch adds the following.

### Read aloud (George's voice)

- **loqui-kokoro and loqui-g2p** 0.4.0, by Awakened Labs Inc. MIT License (`licenses/loqui-MIT.txt`). Runs Kokoro and turns English text into phonemes. Their notice is reproduced in full in `licenses/loqui-NOTICE.txt`. The parts embedded in Dispatch through these crates:
  - misaki G2P lexicons and English G2P logic (version 0.9.4), https://github.com/hexgrad/misaki. Apache License 2.0 (`licenses/Apache-2.0.txt`).
  - English grapheme-to-phoneme models by PeterReid, https://huggingface.co/PeterReid/graphemes_to_phonemes_en_us and https://huggingface.co/PeterReid/graphemes_to_phonemes_en_gb. Apache License 2.0.
  - spaCy English tokenizer rules (spaCy 3.8.16), https://github.com/explosion/spaCy. Copyright (c) 2016-2026 ExplosionAI GmbH, 2016 spaCy GmbH, 2015 Matthew Honnibal. MIT License.
- **ONNX Runtime**, https://github.com/microsoft/onnxruntime. Copyright (c) Microsoft Corporation. MIT License (`licenses/onnxruntime-MIT.txt`). Statically linked through the `ort` crate.
- **Kokoro-82M** model weights and voices, https://huggingface.co/hexgrad/Kokoro-82M (ONNX export: https://huggingface.co/onnx-community/Kokoro-82M-v1.0-ONNX). Apache License 2.0. Not included in Dispatch: downloaded from Hugging Face when you set up read aloud, pinned to a fixed revision and checked against known SHA-256 hashes.

No GPL code is linked.

### Playback and recordings

- **nnnoiseless** 0.5.2, a Rust port of RNNoise, https://github.com/jneem/nnnoiseless. Copyright (c) 2020 Joe Neeman, (c) 2017 Mozilla, (c) 2007-2017 Jean-Marc Valin, (c) 2005-2017 Xiph.Org Foundation, (c) 2003-2004 Mark Borgerding. BSD 3-Clause License (`licenses/nnnoiseless-BSD-3-Clause.txt`). Quiets background noise when you play a dictation back.
- **trash** 5.4.0, https://github.com/ArturKovacs/trash. Copyright 2019 Artúr Barnabás Kovács. MIT License (`licenses/trash-MIT.txt`). Moves old recordings to the Recycle Bin instead of deleting them.

### Fonts

- **Inter**, https://github.com/rsms/inter. SIL Open Font License 1.1 (`licenses/Inter-OFL-1.1.txt`).
- **Space Grotesk**, https://github.com/floriankarsten/space-grotesk. SIL Open Font License 1.1 (`licenses/SpaceGrotesk-OFL-1.1.txt`).

### Models you download

Speech models (Whisper, Parakeet and others) and the optional AI cleanup model are not part of Dispatch. They are downloaded when you choose them, under their own licenses, as in Handy.
