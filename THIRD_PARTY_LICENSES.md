# Third-party licenses

This project is Apache-2.0. The following dependency is distributed with the
engine and keeps its own license:

## tiny-skia 0.12.0

Copyright (c) 2011 Google Inc. All rights reserved.
Copyright (c) 2020 Yevhenii Reizner All rights reserved.

BSD 3-Clause License. Full text:

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are
met:

  * Redistributions of source code must retain the above copyright
    notice, this list of conditions and the following disclaimer.

  * Redistributions in binary form must reproduce the above copyright
    notice, this list of conditions and the following disclaimer in
    the documentation and/or other materials provided with the
    distribution.

  * Neither the name of the copyright holder nor the names of its
    contributors may be used to endorse or promote products derived
    from this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
"AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT
OWNER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
(INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

## cosmic-text 0.19.0

Copyright the cosmic-text contributors. Licensed under MIT OR Apache-2.0.
Source: https://github.com/pop-os/cosmic-text

This project takes cosmic-text under the Apache-2.0 option; the MIT option is
also available. When distributing binaries, keep this notice with the
third-party license list.

## Optional native-backend and test dependencies

The following direct dependencies are optional (off by default) or
development-only. Each is dual-licensed; this project uses each under its
Apache-2.0 option, which is the same license as this project. Versions are
pinned exactly (`=x.y.z`, or a fixed git revision for Blitz).

| Crate | Version / revision | License | Source |
|---|---|---|---|
| blitz-dom | 0.3.0-beta.4 @ git rev 52739472460824ec7f1624ab135c09a4bbc99219 | MIT OR Apache-2.0 | https://github.com/DioxusLabs/blitz |
| blitz-traits | 0.3.0-beta.4 @ same rev | MIT OR Apache-2.0 | https://github.com/DioxusLabs/blitz |
| blitz-paint | 0.3.0-beta.4 @ same rev | MIT OR Apache-2.0 | https://github.com/DioxusLabs/blitz |
| anyrender | =0.14.0 | MIT OR Apache-2.0 | https://github.com/dioxuslabs/anyrender |
| anyrender_vello_cpu | =0.18.0 | MIT OR Apache-2.0 | https://github.com/dioxuslabs/anyrender |
| vello_cpu | =0.3.0 | Apache-2.0 OR MIT | https://github.com/linebender/vello |
| vello_gpu | =0.3.0 | Apache-2.0 OR MIT | https://github.com/linebender/vello |
| vello_common | =0.3.0 | Apache-2.0 OR MIT | https://github.com/linebender/vello |
| anyrender_vello_hybrid | =0.11.0 | MIT OR Apache-2.0 | https://github.com/dioxuslabs/anyrender |
| wgpu_context | =0.10.0 | MIT OR Apache-2.0 | https://github.com/dioxuslabs/anyrender |
| wgpu | =30.0.1 | MIT OR Apache-2.0 | https://github.com/gfx-rs/wgpu |
| pollster | =0.4.0 | Apache-2.0 OR MIT | https://github.com/zesterer/pollster |
| rustc-hash | =2.1.3 | Apache-2.0 OR MIT | https://github.com/rust-lang/rustc-hash |
| parley | =0.12.0 | Apache-2.0 OR MIT | https://github.com/linebender/parley |
| fontique | =0.12.0 | Apache-2.0 OR MIT | https://github.com/linebender/parley |
| keyboard-types | =0.8.3 | MIT OR Apache-2.0 | https://github.com/rust-windowing/keyboard-types |
| proptest (dev only) | =1.12.0 | MIT OR Apache-2.0 | https://github.com/proptest-rs/proptest |

The full Apache License 2.0 text is in `LICENSE`.
