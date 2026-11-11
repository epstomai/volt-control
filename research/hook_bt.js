// Frida: when UAMixerHelper writes a Volt HID frame (starts with F0), log the frame and the call stack
// as module+offset, so the frame builder can be located statically. Read-only.
const wf = Module.findExportByName('kernel32.dll', 'WriteFile');
Interceptor.attach(wf, {
  onEnter(args) {
    const buf = args[1], len = args[2].toInt32();
    if (len < 3 || buf.readU8() !== 0xf0) return;
    const bytes = Array.from(new Uint8Array(buf.readByteArray(Math.min(len, 12))), (x) => x.toString(16).padStart(2, '0')).join(' ');
    const bt = Thread.backtrace(this.context, Backtracer.ACCURATE).map((a) => {
      const m = Process.findModuleByAddress(a);
      return m ? `${m.name}+0x${a.sub(m.base).toString(16)}` : a.toString();
    });
    send(`HID ${bytes}\n    ${bt.join('\n    ')}`);
  },
});
send('hooks installed');
