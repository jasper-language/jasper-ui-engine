"""Extract exact current production types; never maintain surrogate definitions."""
from pathlib import Path
root = Path(__file__).resolve().parents[2]
node = (root/'src/scene.rs').read_text().split('// BEGIN VERIFIED NODE ID TYPE')[1].split('// END VERIFIED NODE ID TYPE')[0]
capture = (root/'src/pointer.rs').read_text().split('// BEGIN VERIFIED POINTER ROUTER TYPE')[1].split('/// Routes pointer events')[0]
(root/'verification/capture-slots/src/types.rs').write_text('// Generated from production markers by generate-types.py.\n'+node+capture)
