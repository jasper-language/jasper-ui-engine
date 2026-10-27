# Deferred Skia integration

Historical, unaccepted source and native tests retained for audit and rollback.
Not exported by the production crate; not part of automatic root test discovery.
The former skia-text-pending Cargo feature has been removed. These tests need
explicit experimental manifest restoration before use. Do not resume Skia hit,
UTF-16 or storage Kani diagnostics under the current schedule.

Existing evidence retains its historical source hashes and failures/timeouts.
The extraction verifier can still locate this archived source; this does not
make it current production evidence or authorize running the deferred suite.
Replacement: Vello CPU 0.3.0 + Vello GPU 0.3.0 + Parley 0.12.0.
Canvas is shelved. The old optional tiny-skia Canvas feature is rollback only.
