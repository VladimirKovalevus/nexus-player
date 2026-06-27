# Butterchurn Raw Audio Interface

This interface lets you use Butterchurn in environments with no direct access to an `AudioContext` (for example, when audio data comes from a Rust, C++ or Node.js backend).

## 1. Initialization

When creating the visualizer, pass `null` as the first argument. This stops Butterchurn from trying to connect to the Web Audio API.

```javascript
import butterchurn from 'butterchurn';

const canvas = document.getElementById('canvas');
const visualizer = butterchurn.createVisualizer(null, canvas, {
  width: 800,
  height: 600,
  pixelRatio: window.devicePixelRatio,
  textureRatio: 1,
});
```

## 2. Data format (audioData)

The `renderRaw` method expects an object with three `Uint8Array` arrays, each **1024** bytes long.

*   **Values**: from `0` to `255`.
*   **Silence**: `128`.
*   **Length**: exactly `1024` (matches the internal `fftSize` / 2).

### If your backend (Rust) provides Float32 (-1.0 ... 1.0):
Before sending the samples to JS, convert them like this:
`uint8_sample = (float_sample * 127.0 + 128.0) as u8`

## 3. Calling the renderer

Call `renderRaw` in your main loop (requestAnimationFrame or a callback from Rust).

```javascript
function onAudioFrame(dataFromRust, deltaTime) {
  const audioData = {
    timeByteArray:  dataFromRust.mixed,  // Uint8Array(1024)
    timeByteArrayL: dataFromRust.left,   // Uint8Array(1024)
    timeByteArrayR: dataFromRust.right   // Uint8Array(1024)
  };

  // deltaTime is the time in seconds since the previous frame (for example, 0.016)
  visualizer.renderRaw(audioData, deltaTime);
}
```

## 4. Why does deltaTime (elapsedTime) matter?

Butterchurn uses time to compute motion (rotation, zoom, pulsing).
- If you do not pass `deltaTime`, it uses `performance.now()`.
- If you render video or your backend does not run in real time, you **must** pass an exact `deltaTime` so the animation stays smooth and in sync with the music.

## 5. Example with simulated data

```javascript
const mockData = new Uint8Array(1024).fill(128);

function loop() {
  // Fill with random data for testing
  for(let i=0; i<1024; i++) mockData[i] = Math.floor(Math.random() * 256);

  visualizer.renderRaw({
    timeByteArray:  mockData,
    timeByteArrayL: mockData,
    timeByteArrayR: mockData
  }, 1/60);

  requestAnimationFrame(loop);
}
```
