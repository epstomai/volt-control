// Frida: observe how UAMixerHelper talks to the Volt (read-only logging).
// Logs Thesycon TUSBAUDIO API calls that send data to the device, plus Volt HID writes.
const MOD = 'UniversalAudioUsbAudioapi_x64.dll';
const FNS = [
  'TUSBAUDIO_ClassVendorRequestOut',
  'TUSBAUDIO_AudioControlRequestSet',
  'TUSBAUDIO_SetVolume',
  'TUSBAUDIO_SetMute',
  'TUSBAUDIO_SetDspProperty',
];

function hex(p, n) {
  try {
    return Array.from(new Uint8Array(p.readByteArray(n)), (x) => x.toString(16).padStart(2, '0')).join(' ');
  } catch (_) {
    return null;
  }
}

function describe(args, count) {
  const parts = [];
  for (let i = 0; i < count; i++) {
    const a = args[i];
    let s = `a${i}=${a}`;
    const r = Process.findRangeByAddress(a);
    if (r && r.protection.indexOf('r') === 0 && !a.isNull()) {
      const h = hex(a, 32);
      if (h) s += ` [${h}]`;
    }
    parts.push(s);
  }
  return parts.join(' ');
}

for (const name of FNS) {
  const f = Module.findExportByName(MOD, name);
  if (!f) { send(`missing ${name}`); continue; }
  Interceptor.attach(f, {
    onEnter(args) { send(`${name} ${describe(args, 9)}`); },
  });
}

const wf = Module.findExportByName('kernel32.dll', 'WriteFile');
Interceptor.attach(wf, {
  onEnter(args) {
    const buf = args[1], len = args[2].toInt32();
    if (len >= 3 && buf.readU8() === 0xf0) send(`HID WriteFile len=${len} | ${hex(buf, Math.min(len, 16))}`);
  },
});
send('hooks installed');
