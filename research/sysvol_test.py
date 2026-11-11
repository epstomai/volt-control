import subprocess, sys, time, warnings
warnings.filterwarnings("ignore")
from comtypes import CoInitialize
from pycaw.pycaw import AudioUtilities
CoInitialize()
ep = AudioUtilities.GetSpeakers().EndpointVolume
def st(): return round(ep.GetMasterVolumeLevelScalar() * 100), bool(ep.GetMute())
def run(*a): subprocess.run([sys.executable, *a], capture_output=True)
v0, m0 = st(); print("start", v0, m0)
ep.SetMasterVolumeLevelScalar(0.9, None); time.sleep(0.6); run("shot.py", "sv_ext.png"); print("external -> 90:", st())
ep.SetMasterVolumeLevelScalar(v0 / 100, None); time.sleep(0.4)
run("click.py", "393", "273"); time.sleep(0.6); run("shot.py", "sv_mute.png"); print("UI mute click:", st())
run("click.py", "393", "273"); time.sleep(0.6); print("UI mute click again:", st())
from PIL import Image
a, b = Image.open("sv_ext.png").crop((340, 95, 450, 290)), Image.open("sv_mute.png").crop((340, 95, 450, 290))
c = Image.new("RGB", (230, 195), (255, 0, 0)); c.paste(a, (0, 0)); c.paste(b, (120, 0)); c.save("sv_cmp.png")
