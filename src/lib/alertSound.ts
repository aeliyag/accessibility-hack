let audioContext: AudioContext | null = null;
let alarmTimer = 0;

function context(): AudioContext | null {
  const AudioCtor =
    window.AudioContext ??
    (window as Window & { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
  if (!AudioCtor) {
    return null;
  }
  if (!audioContext) {
    audioContext = new AudioCtor();
  }
  return audioContext;
}

export function unlockAlertSound() {
  const audio = context();
  if (audio?.state === "suspended") {
    void audio.resume();
  }
}

function beep(frequency: number, start: number, duration: number, level: number) {
  const audio = context();
  if (!audio) {
    return;
  }

  const oscillator = audio.createOscillator();
  const gain = audio.createGain();
  const when = audio.currentTime + start;
  oscillator.type = "sine";
  oscillator.frequency.value = frequency;
  gain.gain.setValueAtTime(0.0001, when);
  gain.gain.exponentialRampToValueAtTime(level, when + 0.02);
  gain.gain.exponentialRampToValueAtTime(0.0001, when + duration);
  oscillator.connect(gain);
  gain.connect(audio.destination);
  oscillator.start(when);
  oscillator.stop(when + duration + 0.02);
}

function alarmPulse() {
  beep(880, 0, 0.16, 0.18);
  beep(880, 0.26, 0.16, 0.18);
  beep(660, 0.52, 0.28, 0.2);
}

export function startCloseAlarm() {
  stopCloseAlarm();
  unlockAlertSound();
  alarmPulse();
  alarmTimer = window.setInterval(alarmPulse, 2800);
}

export function stopCloseAlarm() {
  window.clearInterval(alarmTimer);
  alarmTimer = 0;
}

export function playLookAwayChime() {
  unlockAlertSound();
  beep(523, 0, 0.14, 0.1);
  beep(659, 0.16, 0.14, 0.1);
  beep(784, 0.32, 0.24, 0.12);
}
