# Butterchurn Raw Audio Interface

Этот интерфейс позволяет использовать Butterchurn в средах, где нет прямого доступа к `AudioContext` (например, при получении аудиоданных из Rust, C++ или Node.js бэкенда).

## 1. Инициализация

При создании визуализатора первым аргументом передайте `null`. Это отключит попытки Butterchurn подключиться к Web Audio API.

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

## 2. Формат данных (audioData)

Метод `renderRaw` ожидает объект с тремя массивами `Uint8Array` длиной **1024** байта.

*   **Значения**: от `0` до `255`.
*   **Тишина**: `128`.
*   **Длина**: Строго `1024` (соответствует внутреннему `fftSize` / 2).

### Если ваш бэкенд (Rust) отдает Float32 (-1.0 ... 1.0):
Перед отправкой в JS преобразуйте их следующим образом:
`uint8_sample = (float_sample * 127.0 + 128.0) as u8`

## 3. Вызов рендеринга

Вызывайте `renderRaw` в вашем основном цикле (requestAnimationFrame или callback из Rust).

```javascript
function onAudioFrame(dataFromRust, deltaTime) {
  const audioData = {
    timeByteArray:  dataFromRust.mixed,  // Uint8Array(1024)
    timeByteArrayL: dataFromRust.left,   // Uint8Array(1024)
    timeByteArrayR: dataFromRust.right   // Uint8Array(1024)
  };

  // deltaTime — время в секундах, прошедшее с прошлого кадра (например, 0.016)
  visualizer.renderRaw(audioData, deltaTime);
}
```

## 4. Почему важен deltaTime (elapsedTime)?

Butterchurn использует время для расчета движения (вращение, зум, пульсация). 
- Если не передавать `deltaTime`, он будет использовать `performance.now()`.
- Если вы рендерите видео или ваш бэкенд идет не в реальном времени, **обязательно** передавайте точный `deltaTime`, чтобы анимация была плавной и синхронизированной с музыкой.

## 5. Пример с эмуляцией данных

```javascript
const mockData = new Uint8Array(1024).fill(128);

function loop() {
  // Заполняем случайными данными для теста
  for(let i=0; i<1024; i++) mockData[i] = Math.floor(Math.random() * 256);

  visualizer.renderRaw({
    timeByteArray:  mockData,
    timeByteArrayL: mockData,
    timeByteArrayR: mockData
  }, 1/60);

  requestAnimationFrame(loop);
}
```
