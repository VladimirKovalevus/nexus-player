import FFT from "./fft";

export default class AudioProcessor {
  constructor(context) {
    this.numSamps = 512;
    this.fftSize = this.numSamps * 2;

    this.fft = new FFT(this.fftSize, 512, true);

    if (context) {
      this.audioContext = context;
      this.audible = context.createDelay();

      this.analyser = context.createAnalyser();
      this.analyser.smoothingTimeConstant = 0.0;
      this.analyser.fftSize = this.fftSize;

      this.audible.connect(this.analyser);

      // Split channels
      this.analyserL = context.createAnalyser();
      this.analyserL.smoothingTimeConstant = 0.0;
      this.analyserL.fftSize = this.fftSize;

      this.analyserR = context.createAnalyser();
      this.analyserR.smoothingTimeConstant = 0.0;
      this.analyserR.fftSize = this.fftSize;

      this.splitter = context.createChannelSplitter(2);

      this.audible.connect(this.splitter);
      this.splitter.connect(this.analyserL, 0);
      this.splitter.connect(this.analyserR, 1);
    }

    this.timeByteArray = new Uint8Array(this.fftSize);
    this.timeByteArrayL = new Uint8Array(this.fftSize);
    this.timeByteArrayR = new Uint8Array(this.fftSize);

    // Initialized once as typed arrays
    this.freqArray = new Float32Array(this.numSamps);
    this.freqArrayL = new Float32Array(this.numSamps);
    this.freqArrayR = new Float32Array(this.numSamps);

    // Signed raw samples shifted to -128 -> 127
    this.timeArray = new Int8Array(this.fftSize);
    this.timeByteArraySignedL = new Int8Array(this.fftSize);
    this.timeByteArraySignedR = new Int8Array(this.fftSize);

    // Temporary array for smoothing
    this.tempTimeArrayL = new Int8Array(this.fftSize);
    this.tempTimeArrayR = new Int8Array(this.fftSize);

    // Undersampled from this.fftSize to this.numSamps
    this.timeArrayL = new Int8Array(this.numSamps);
    this.timeArrayR = new Int8Array(this.numSamps);
  }

  sampleAudio() {
    if (this.analyser) {
      this.analyser.getByteTimeDomainData(this.timeByteArray);
    }
    if (this.analyserL) {
      this.analyserL.getByteTimeDomainData(this.timeByteArrayL);
    }
    if (this.analyserR) {
      this.analyserR.getByteTimeDomainData(this.timeByteArrayR);
    }
    this.processAudio();
  }
  updateAudio(timeByteArray, timeByteArrayL, timeByteArrayR, freqArray, freqArrayL, freqArrayR) {
    if (timeByteArray) {
      this.timeByteArray.set(timeByteArray);
    }
    if (timeByteArrayL) {
      this.timeByteArrayL.set(timeByteArrayL);
    }
    if (timeByteArrayR) {
      this.timeByteArrayR.set(timeByteArrayR);
    }

    if (freqArray) {
      this.freqArray = freqArray;
    }
    if (freqArrayL) {
      this.freqArrayL = freqArrayL;
    }
    if (freqArrayR) {
      this.freqArrayR = freqArrayR;
    }

    this.processAudio(!!freqArray);
  }
  /* eslint-disable no-bitwise */
  processAudio(bypassFFT = false) {
    for (let i = 0, j = 0, lastIdx = 0; i < this.fftSize; i++) {
      // Shift Unsigned to Signed about 0
      this.timeArray[i] = this.timeByteArray[i] - 128;
      this.timeByteArraySignedL[i] = this.timeByteArrayL[i] - 128;
      this.timeByteArraySignedR[i] = this.timeByteArrayR[i] - 128;

      this.tempTimeArrayL[i] =
        0.5 *
        (this.timeByteArraySignedL[i] + this.timeByteArraySignedL[lastIdx]);
      this.tempTimeArrayR[i] =
        0.5 *
        (this.timeByteArraySignedR[i] + this.timeByteArraySignedR[lastIdx]);

      // Undersampled
      if (i % 2 === 0) {
        this.timeArrayL[j] = this.tempTimeArrayL[i];
        this.timeArrayR[j] = this.tempTimeArrayR[i];
        j += 1;
      }

      lastIdx = i;
    }

    if (!bypassFFT) {
      // Use full width samples for the FFT
      this.freqArray = this.fft.timeToFrequencyDomain(this.timeArray);
      this.freqArrayL = this.fft.timeToFrequencyDomain(this.timeByteArraySignedL);
      this.freqArrayR = this.fft.timeToFrequencyDomain(this.timeByteArraySignedR);
    }
  }

  connectAudio(audionode) {
    if (this.audible) {
      audionode.connect(this.audible);
    }
  }

  disconnectAudio(audionode) {
    if (this.audible) {
      audionode.disconnect(this.audible);
    }
  }
  /* eslint-enable no-bitwise */
}
