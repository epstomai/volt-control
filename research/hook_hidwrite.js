// Frida: log what UAMixerHelper writes to HID devices (read-only observation).
// hidapi on Windows uses WriteFile for output reports and HidD_SetFeature /
// HidD_SetOutputReport for feature/output reports. We log only buffers that look
// like Volt frames (start with F0, optionally after a report-ID byte) to stay quiet.
function hex(ptr, len) {
  const b = new Uint8Array(ptr.readByteArray(Math.min(len, 96)));
  return Array.from(b, (x) => x.toString(16).padStart(2, '0')).join(' ');
}

function looksVolt(ptr, len) {
  if (len < 3) return false;
  const b0 = ptr.readU8(), b1 = ptr.add(1).readU8();
  return b0 === 0xf0 || b1 === 0xf0;
}

function hook(mod, name, bufArg, lenArg) {
  const f = Module.findExportByName(mod, name);
  if (!f) { send(`missing ${mod}!${name}`); return; }
  Interceptor.attach(f, {
    onEnter(args) {
      const buf = args[bufArg], len = args[lenArg].toInt32();
      if (looksVolt(buf, len)) send(`${name} len=${len} | ${hex(buf, len)}`);
    },
  });
  send(`hooked ${mod}!${name}`);
}

hook('kernel32.dll', 'WriteFile', 1, 2);
hook('hid.dll', 'HidD_SetOutputReport', 1, 2);
hook('hid.dll', 'HidD_SetFeature', 1, 2);
