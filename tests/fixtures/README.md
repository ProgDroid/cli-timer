# Sound fixtures

Each file is 0.5 s of a 440 Hz sine, mono, 22 050 Hz. They pin how `-s` decodes real files
(the `src/sound.rs` tests).

Made with `ffmpeg version 4.4.1-essentials_build-www.gyan.dev`, from the repository root:

```
C='-f lavfi -i sine=frequency=440:duration=0.5:sample_rate=22050 -ac 1 -map_metadata -1 -fflags +bitexact'
ffmpeg $C -c:a libmp3lame -b:a 32k tests/fixtures/tone.mp3
ffmpeg $C -c:a flac tests/fixtures/tone.flac
ffmpeg $C -c:a libvorbis -q:a 0 tests/fixtures/tone.ogg
ffmpeg $C -c:a adpcm_ms tests/fixtures/tone-adpcm.wav
```

`tone-adpcm.wav` is a WAV holding ADPCM, not PCM. rodio 0.16 (up to v0.2.1) decoded it; since
v0.2.2 (rodio 0.23 with the `symphonia-pcm` feature only) it is rejected, as
`an_adpcm_wav_is_rejected_as_undecodable` asserts.

`.gitattributes` marks `tests/fixtures/**` as `-text` so Windows checkouts never alter the bytes.
